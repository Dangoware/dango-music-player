// Crate things
//use crate::music_controller::config::Config;
use crate::music_storage::library::URI;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::error::Error;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

// GStreamer things
use glib::FlagsClass;
use gst::{ClockTime, Element};
use gstreamer as gst;
use gstreamer::prelude::*;

// Extra things
use chrono::Duration;

use super::player::{Player, PlayerCommand, PlayerError, PlayerState};

impl From<gst::State> for PlayerState {
    fn from(value: gst::State) -> Self {
        match value {
            gst::State::VoidPending => Self::VoidPending,
            gst::State::Playing => Self::Playing,
            gst::State::Paused => Self::Paused,
            gst::State::Ready => Self::Ready,
            gst::State::Null => Self::Null,
        }
    }
}

impl TryInto<gst::State> for PlayerState {
    fn try_into(self) -> Result<gst::State, Box<dyn Error>> {
        match self {
            Self::VoidPending => Ok(gst::State::VoidPending),
            Self::Playing => Ok(gst::State::Playing),
            Self::Paused => Ok(gst::State::Paused),
            Self::Ready => Ok(gst::State::Ready),
            Self::Null => Ok(gst::State::Null),
            state => Err(format!("Invalid gst::State: {:?}", state).into()),
        }
    }

    type Error = Box<dyn Error>;
}

#[derive(Debug, PartialEq, Eq)]
enum PlaybackInfo {
    Idle,
    Switching,
    Playing{
        start: Duration,
        end:   Duration,
    },
    Finished // When this is sent, the thread will die!
}

/// An instance of a music player with a GStreamer backend
#[derive(Debug)]
pub struct GStreamer {
    source:     Option<URI>,

    message_rx: crossbeam::channel::Receiver<PlayerCommand>,
    playback_tx: crossbeam::channel::Sender<PlaybackInfo>,

    playbin:    Arc<RwLock<Element>>,
    volume:     f64,
    start:      Option<Duration>,
    end:        Option<Duration>,
    paused:     bool,
    position:   Arc<RwLock<Option<Duration>>>,
}

impl From<gst::StateChangeError> for PlayerError {
    fn from(value: gst::StateChangeError) -> Self {
        PlayerError::StateChange(value.to_string())
    }
}

impl From<glib::BoolError> for PlayerError {
    fn from(value: glib::BoolError) -> Self {
        PlayerError::General(value.to_string())
    }
}

impl GStreamer {
    /// Set the playback URI
    fn set_source(&mut self, source: &URI) -> Result<(), PlayerError> {
        if !source.exists().is_ok_and(|x| x) {
            // If the source doesn't exist, gstreamer will crash!
            return Err(PlayerError::NotFound)
        }

        // Make sure the playback tracker knows the stuff is stopped
        self.playback_tx.send(PlaybackInfo::Switching).unwrap();

        let uri = self.playbin.read().unwrap().property_value("current-uri");
        self.source = Some(source.clone());
        match source {
            URI::Cue { start, end, .. } => {
                self.playbin
                    .write()
                    .unwrap()
                    .set_property("uri", source.as_uri());

                // Set the start and end positions of the CUE file
                self.start = Some(Duration::from_std(*start).unwrap());
                self.end = Some(Duration::from_std(*end).unwrap());

                // Send the updated position to the tracker
                self.playback_tx.send(PlaybackInfo::Playing{
                    start: self.start.unwrap(),
                    end: self.end.unwrap()
                }).unwrap();

                // Wait for it to be ready, and then move to the proper position
                self.play().unwrap();
                let now = std::time::Instant::now();
                while now.elapsed() < std::time::Duration::from_millis(20) {
                    if self.seek_to(Duration::from_std(*start).unwrap()).is_ok() {
                        return Ok(());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                //panic!("Couldn't seek to beginning of cue track in reasonable time (>20ms)");
                return Err(PlayerError::StateChange("Could not seek to beginning of CUE track".into()))
            }
            _ => {
                self.playbin
                    .write()
                    .unwrap()
                    .set_property("uri", source.as_uri());

                self.play().unwrap();

                while uri.get::<&str>().unwrap_or("")
                    == self.property("current-uri").get::<&str>().unwrap_or("")
                    || self.position().is_none()
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }

                self.start = Some(Duration::seconds(0));
                self.end = self.raw_duration();

                // Send the updated position to the tracker
                self.playback_tx.send(PlaybackInfo::Playing{
                    start: self.start.unwrap(),
                    end: self.end.unwrap()
                }).unwrap();
            }
        }

        Ok(())
    }

    /// Gets a mutable reference to the playbin element
    fn playbin_mut(
        &mut self,
    ) -> Result<RwLockWriteGuard<gst::Element>, std::sync::PoisonError<RwLockWriteGuard<'_, Element>>>
    {
        let element = match self.playbin.write() {
            Ok(element) => element,
            Err(err) => return Err(err),
        };
        Ok(element)
    }

    /// Gets a read-only reference to the playbin element
    fn playbin(
        &self,
    ) -> Result<RwLockReadGuard<gst::Element>, std::sync::PoisonError<RwLockReadGuard<'_, Element>>>
    {
        let element = match self.playbin.read() {
            Ok(element) => element,
            Err(err) => return Err(err),
        };
        Ok(element)
    }

    /// Set volume of the internal playbin player, can be
    /// used to bypass the main volume control for seeking
    fn set_gstreamer_volume(&mut self, volume: f64) {
        self.playbin_mut().unwrap().set_property("volume", volume)
    }

    fn set_state(&mut self, state: gst::State) -> Result<(), gst::StateChangeError> {
        self.playbin_mut().unwrap().set_state(state)?;

        Ok(())
    }

    fn raw_duration(&self) -> Option<Duration> {
        self.playbin()
            .unwrap()
            .query_duration::<ClockTime>()
            .map(|pos| Duration::nanoseconds(pos.nseconds() as i64))
    }

    /// Get the current state of the playback
    fn state(&mut self) -> PlayerState {
        self.playbin().unwrap().current_state().into()
        /*
        match *self.buffer.read().unwrap() {
            None => self.playbin().unwrap().current_state().into(),
            Some(value) => PlayerState::Buffering(value),
        }
        */
    }

    fn property(&self, property: &str) -> glib::Value {
        self.playbin().unwrap().property_value(property)
    }
}

impl Player for GStreamer {
    fn new() -> Result<Self, PlayerError> {
        // Initialize GStreamer, maybe figure out how to nicely fail here
        if let Err(err) = gst::init() {
            return Err(PlayerError::Init(err.to_string()))
        };
        let ctx = glib::MainContext::default();
        let _guard = ctx.acquire();
        let mainloop = glib::MainLoop::new(Some(&ctx), false);

        let playbin_arc = Arc::new(RwLock::new(
            match gst::ElementFactory::make("playbin3").build() {
                Ok(playbin) => playbin,
                Err(error) => return Err(PlayerError::Init(error.to_string())),
            }
        ));

        let playbin = playbin_arc.clone();

        let flags = playbin.read().unwrap().property_value("flags");
        let flags_class = FlagsClass::with_type(flags.type_()).unwrap();

        // Set up the Playbin flags to only play audio
        let flags = flags_class
            .builder_with_value(flags)
            .ok_or(PlayerError::Build)?
            .set_by_nick("audio")
            .set_by_nick("download")
            .unset_by_nick("video")
            .unset_by_nick("text")
            .build()
            .ok_or(PlayerError::Build)?;

        playbin.write().unwrap().set_property_from_value("flags", &flags);
        playbin.write().unwrap().set_property("instant-uri", true);

        let position = Arc::new(RwLock::new(None));

        // Set up the thread to monitor the position
        let (playback_tx, playback_rx) = unbounded();
        let (status_tx, status_rx) = unbounded::<PlaybackInfo>();
        let position_update = Arc::clone(&position);

        let _playback_monitor =
            std::thread::spawn(|| playback_monitor(playbin_arc, status_rx, playback_tx, position_update));

        // Set up the thread to monitor bus messages
        let playbin_bus_ctrl = Arc::clone(&playbin);
        let bus_watch = playbin
            .read()
            .unwrap()
            .bus()
            .expect("Failed to get GStreamer message bus")
            .add_watch(move |_bus, msg| {
                match msg.view() {
                    gst::MessageView::Eos(_) => {}
                    gst::MessageView::StreamStart(_) => println!("Stream start"),
                    gst::MessageView::Error(_) => {
                        playbin_bus_ctrl
                            .write()
                            .unwrap()
                            .set_state(gst::State::Ready)
                            .unwrap();

                        playbin_bus_ctrl
                            .write()
                            .unwrap()
                            .set_state(gst::State::Playing)
                            .unwrap();
                    }
                    /* TODO: Fix buffering!!
                    gst::MessageView::Buffering(buffering) => {
                        let percent = buffering.percent();
                        if percent < 100 {
                            playbin_bus_ctrl
                                .write()
                                .unwrap()
                                .set_state(gst::State::Paused)
                                .unwrap();
                        } else if !(buffering) {
                            playbin_bus_ctrl
                                .write()
                                .unwrap()
                                .set_state(gst::State::Playing)
                                .unwrap();
                        }
                    }
                    */
                    _ => (),
                }
                glib::ControlFlow::Continue
            })
            .expect("Failed to connect to GStreamer message bus");

        // Set up a thread to watch the messages
        std::thread::spawn(move || {
            let _watch = bus_watch;
            mainloop.run()
        });

        let source = None;
        Ok(Self {
            source,
            playbin,
            message_rx: playback_rx,
            playback_tx: status_tx,
            volume: 1.0,
            start: None,
            end: None,
            paused: false,
            position,
        })
    }

    fn source(&self) -> &Option<URI> {
        &self.source
    }

    /// Insert a new track to be played. This method should be called at the
    /// beginning to start playback of something, and once the [PlayerCommand]
    /// indicates the track is about to finish to enqueue gaplessly.
    fn enqueue_next(&mut self, next_track: &URI) -> Result<(), PlayerError> {
        self.set_source(next_track)
    }

    /// Set the playback volume, accepts a float from 0 to 1
    fn set_volume(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 1.0);
        self.set_gstreamer_volume(self.volume);
    }

    /// Returns the current volume level, a float from 0 to 1
    fn volume(&mut self) -> f64 {
        self.volume
    }

    fn ready(&mut self) -> Result<(), PlayerError> {
        self.set_state(gst::State::Ready)?;
        Ok(())
    }

    /// If the player is paused or stopped, starts playback
    fn play(&mut self) -> Result<(), PlayerError> {
        self.set_state(gst::State::Playing)?;
        Ok(())
    }

    /// Pause, if playing
    fn pause(&mut self) -> Result<(), PlayerError> {
        //self.paused = true;
        self.set_state(gst::State::Paused)?;
        Ok(())
    }

    /// Resume from being paused
    fn resume(&mut self) -> Result<(), PlayerError> {
        //self.paused = false;
        self.set_state(gst::State::Playing)?;
        Ok(())
    }

    /// Check if playback is paused
    fn is_paused(&mut self) -> bool {
        self.playbin().unwrap().current_state() == gst::State::Paused
    }

    /// Get the current playback position of the player
    fn position(&mut self) -> Option<Duration> {
        *self.position.read().unwrap()
    }

    /// Get the duration of the currently playing track
    fn duration(&mut self) -> Option<Duration> {
        if self.end.is_some() && self.start.is_some() {
            Some(self.end.unwrap() - self.start.unwrap())
        } else {
            self.raw_duration()
        }
    }

    /// Seek relative to the current position
    fn seek_by(&mut self, seek_amount: Duration) -> Result<(), PlayerError> {
        let time_pos = match *self.position.read().unwrap() {
            Some(pos) => pos,
            None => return Err(PlayerError::Seek("No position".into())),
        };
        let seek_pos = time_pos + seek_amount;

        self.seek_to(seek_pos)?;
        Ok(())
    }

    /// Seek absolutely
    fn seek_to(&mut self, target_pos: Duration) -> Result<(), PlayerError> {
        let start = if self.start.is_none() {
            return Err(PlayerError::Seek("No START time".into()));
        } else {
            self.start.unwrap()
        };

        let end = if self.end.is_none() {
            return Err(PlayerError::Seek("No END time".into()));
        } else {
            self.end.unwrap()
        };

        let adjusted_target = target_pos + start;
        let clamped_target = adjusted_target.clamp(start, end);

        let seek_pos_clock =
            ClockTime::from_useconds(clamped_target.num_microseconds().unwrap() as u64);

        self.set_gstreamer_volume(0.0);
        self.playbin_mut()
            .unwrap()
            .seek_simple(gst::SeekFlags::FLUSH, seek_pos_clock)?;
        self.set_gstreamer_volume(self.volume);
        Ok(())
    }

    /// Stop the playback entirely
    fn stop(&mut self) -> Result<(), PlayerError> {
        self.pause()?;
        self.ready()?;

        // Send the updated position to the tracker
        self.playback_tx.send(PlaybackInfo::Idle).unwrap();

        // Set all positions to none
        *self.position.write().unwrap() = None;
        self.start = None;
        self.end = None;
        Ok(())
    }

    /// Return a reference to the player message channel
    fn message_channel(&self) -> &crossbeam::channel::Receiver<PlayerCommand> {
        &self.message_rx
    }
}

impl Drop for GStreamer {
    /// Cleans up the `GStreamer` pipeline and the monitoring
    /// thread when [Player] is dropped.
    fn drop(&mut self) {
        self.playbin_mut()
            .unwrap()
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
        let _ = self.playback_tx.send(PlaybackInfo::Finished);
    }
}

fn playback_monitor(
    playbin: Arc<RwLock<Element>>,
    status_rx: Receiver<PlaybackInfo>,
    playback_tx: Sender<PlayerCommand>,
    position: Arc<RwLock<Option<Duration>>>,
) {
    let mut stats = PlaybackInfo::Idle;
    let mut pos_temp;
    loop {
        // Check for new messages to decide how to proceed
        if let Ok(result) = status_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            stats = result
        }

        pos_temp = playbin
            .read()
            .unwrap()
            .query_position::<ClockTime>()
            .map(|pos| Duration::nanoseconds(pos.nseconds() as i64));

        match stats {
            PlaybackInfo::Playing{start, end} if pos_temp.is_some() => {
                // Check if the current playback position is close to the end
                let finish_point = end - Duration::milliseconds(250);
                if pos_temp.unwrap() >= end {
                    let _ = playback_tx.try_send(PlayerCommand::EndOfStream);
                    playbin
                        .write()
                        .unwrap()
                        .set_state(gst::State::Ready)
                        .expect("Unable to set the pipeline state");
                } else if pos_temp.unwrap() >= finish_point {
                    let _ = playback_tx.try_send(PlayerCommand::AboutToFinish);
                }

                // This has to be done AFTER the current time in the file
                // is calculated, or everything else is wrong
                pos_temp = Some(pos_temp.unwrap() - start)
            },
            PlaybackInfo::Finished => {
                *position.write().unwrap() = None;
                break
            },
            PlaybackInfo::Idle | PlaybackInfo::Switching => {},
            _ => ()
        }

        *position.write().unwrap() = pos_temp;
    }
}
