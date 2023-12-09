// Crate things
//use crate::music_controller::config::Config;
use crate::music_storage::library::{Tag, URI};
use crossbeam_channel::bounded;
use std::error::Error;
use std::sync::{Arc, RwLock};

// GStreamer things
use glib::FlagsClass;
use gst::{ClockTime, Element};
use gstreamer as gst;
use gstreamer::prelude::*;

// Time things
use chrono::Duration;

#[derive(Debug)]
pub enum PlayerCmd {
    Play,
    Pause,
    Eos,
    AboutToFinish,
}

#[derive(Debug)]
pub enum PlayerState {
    Playing,
    Paused,
    Ready,
    Buffering(u8),
    Null,
    VoidPending,
}

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

/// An instance of a music player with a GStreamer backend
pub struct Player {
    source: Option<URI>,
    //pub message_tx: Sender<PlayerCmd>,
    pub message_rx: crossbeam::channel::Receiver<PlayerCmd>,
    playbin: Arc<RwLock<Element>>,
    volume: f64,
    start: Arc<RwLock<Option<Duration>>>,
    end: Arc<RwLock<Option<Duration>>>,
    position: Arc<RwLock<Option<Duration>>>,
    buffer: Arc<RwLock<Option<u8>>>,
    paused: Arc<RwLock<bool>>,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    pub fn new() -> Self {
        // Initialize GStreamer
        gst::init().unwrap();
        let ctx = glib::MainContext::default();
        let _guard = ctx.acquire();
        let mainloop = glib::MainLoop::new(Some(&ctx), false);

        let playbin_arc = Arc::new(RwLock::new(
            gst::ElementFactory::make("playbin3").build().unwrap(),
        ));

        let playbin = playbin_arc.clone();

        let flags = playbin.read().unwrap().property_value("flags");
        let flags_class = FlagsClass::with_type(flags.type_()).unwrap();

        // Set up the Playbin flags to only play audio
        let flags = flags_class
            .builder_with_value(flags)
            .unwrap()
            .set_by_nick("audio")
            .set_by_nick("download")
            .unset_by_nick("video")
            .unset_by_nick("text")
            .build()
            .unwrap();

        playbin
            .write()
            .unwrap()
            .set_property_from_value("flags", &flags);

        playbin.write().unwrap().set_property("instant-uri", true);

        let position = Arc::new(RwLock::new(None));
        let start = Arc::new(RwLock::new(None));
        let end: Arc<RwLock<Option<Duration>>> = Arc::new(RwLock::new(None));
        let buffer = Arc::new(RwLock::new(None));
        let paused = Arc::new(RwLock::new(false));

        // Set up the thread to monitor the position
        let position_update = position.clone();
        let start_update = Arc::clone(&start);
        let end_update = Arc::clone(&end);
        let (message_tx, message_rx) = bounded(1); //TODO: Maybe figure out a better method than making this bounded
        std::thread::spawn(move || {
            loop {
                let mut pos_temp = playbin_arc
                    .read()
                    .unwrap()
                    .query_position::<ClockTime>()
                    .map(|pos| Duration::nanoseconds(pos.nseconds() as i64));

                if pos_temp.is_some()
                    && start_update.read().unwrap().is_some()
                    && end_update.read().unwrap().is_some()
                {
                    let atf = end_update.read().unwrap().unwrap() - Duration::milliseconds(250);
                    if pos_temp.unwrap() >= end_update.read().unwrap().unwrap() {
                        let _ = message_tx.try_send(PlayerCmd::Eos);
                        playbin_arc
                            .write()
                            .unwrap()
                            .set_state(gst::State::Ready)
                            .expect("Unable to set the pipeline state");
                        *start_update.write().unwrap() = None;
                        *end_update.write().unwrap() = None;
                    } else if pos_temp.unwrap() >= atf {
                        let _ = message_tx.try_send(PlayerCmd::AboutToFinish);
                    }

                    // This has to be done AFTER the current time in the file
                    // is calculated, or everything else is wrong
                    if let Some(time) = *start_update.read().unwrap() {
                        pos_temp = Some(pos_temp.unwrap() - time)
                    }
                }

                //println!("{:?}", pos_temp);

                *position_update.write().unwrap() = pos_temp;

                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });

        // Set up the thread to monitor bus messages
        let playbin_bus_ctrl = Arc::clone(&playbin);
        let buffer_bus_ctrl = Arc::clone(&buffer);
        let paused_bus_ctrl = Arc::clone(&paused);
        let bus_watch = playbin
            .read()
            .unwrap()
            .bus()
            .expect("Failed to get GStreamer message bus")
            .add_watch(move |_bus, msg| {
                match msg.view() {
                    gst::MessageView::Eos(_) => {}
                    gst::MessageView::StreamStart(_) => println!("Stream start"),
                    gst::MessageView::Error(e) => {
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
                    },
                    gst::MessageView::Buffering(buffering) => {
                        let percent = buffering.percent();
                        if percent < 100 {
                            *buffer_bus_ctrl.write().unwrap() = Some(percent as u8);
                            playbin_bus_ctrl
                                .write()
                                .unwrap()
                                .set_state(gst::State::Paused)
                                .unwrap();
                        } else if *paused_bus_ctrl.read().unwrap() == false {
                            *buffer_bus_ctrl.write().unwrap() = None;
                            playbin_bus_ctrl
                                .write()
                                .unwrap()
                                .set_state(gst::State::Playing)
                                .unwrap();
                        }
                    }
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
        Self {
            source,
            playbin,
            message_rx,
            volume: 1.0,
            start,
            end,
            paused,
            position,
            buffer,
        }
    }

    pub fn source(&self) -> &Option<URI> {
        &self.source
    }

    pub fn enqueue_next(&mut self, next_track: &URI) {
        self.set_source(next_track);
    }

    /// Set the playback URI
    fn set_source(&mut self, source: &URI) {
        let uri = self.playbin.read().unwrap().property_value("current-uri");
        self.source = Some(source.clone());
        match source {
            URI::Cue { start, end, .. } => {
                self.playbin
                    .write()
                    .unwrap()
                    .set_property("uri", source.as_uri());

                // Set the start and end positions of the CUE file
                *self.start.write().unwrap() = Some(Duration::from_std(*start).unwrap());
                *self.end.write().unwrap() = Some(Duration::from_std(*end).unwrap());

                self.play().unwrap();

                // Wait for it to be ready, and then move to the proper position
                let now = std::time::Instant::now();
                while now.elapsed() < std::time::Duration::from_millis(20) {
                    if self.seek_to(Duration::from_std(*start).unwrap()).is_ok() {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                panic!("Couldn't seek to beginning of cue track in reasonable time (>20ms)");
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

                *self.start.write().unwrap() = Some(Duration::seconds(0));
                *self.end.write().unwrap() = self.raw_duration();
            }
        }
    }

    /// Set the playback volume, accepts a float from 0 to 1
    pub fn set_volume(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 1.0);
        self.set_gstreamer_volume(self.volume);
    }

    /// Set volume of the internal playbin player, can be
    /// used to bypass the main volume control for seeking
    fn set_gstreamer_volume(&mut self, volume: f64) {
        self.playbin.write().unwrap().set_property("volume", volume)
    }

    /// Returns the current volume level, a float from 0 to 1
    pub fn volume(&mut self) -> f64 {
        self.volume
    }

    fn set_state(&mut self, state: gst::State) -> Result<(), gst::StateChangeError> {
        self.playbin.write().unwrap().set_state(state)?;

        Ok(())
    }

    pub fn ready(&mut self) -> Result<(), gst::StateChangeError> {
        self.set_state(gst::State::Ready)
    }

    /// If the player is paused or stopped, starts playback
    pub fn play(&mut self) -> Result<(), gst::StateChangeError> {
        self.set_state(gst::State::Playing)
    }

    /// Pause, if playing
    pub fn pause(&mut self) -> Result<(), gst::StateChangeError> {
        *self.paused.write().unwrap() = true;
        self.set_state(gst::State::Paused)
    }

    /// Resume from being paused
    pub fn resume(&mut self) -> Result<(), gst::StateChangeError> {
        *self.paused.write().unwrap() = false;
        self.set_state(gst::State::Playing)
    }

    /// Check if playback is paused
    pub fn is_paused(&mut self) -> bool {
        self.playbin.read().unwrap().current_state() == gst::State::Paused
    }

    /// Get the current playback position of the player
    pub fn position(&mut self) -> Option<Duration> {
        *self.position.read().unwrap()
    }

    /// Get the duration of the currently playing track
    pub fn duration(&mut self) -> Option<Duration> {
        if self.end.read().unwrap().is_some() && self.start.read().unwrap().is_some() {
            Some(self.end.read().unwrap().unwrap() - self.start.read().unwrap().unwrap())
        } else {
            self.raw_duration()
        }
    }

    pub fn raw_duration(&self) -> Option<Duration> {
        self.playbin
            .read()
            .unwrap()
            .query_duration::<ClockTime>()
            .map(|pos| Duration::nanoseconds(pos.nseconds() as i64))
    }

    /// Seek relative to the current position
    pub fn seek_by(&mut self, seek_amount: Duration) -> Result<(), Box<dyn Error>> {
        let time_pos = match *self.position.read().unwrap() {
            Some(pos) => pos,
            None => return Err("No position".into()),
        };
        let seek_pos = time_pos + seek_amount;

        self.seek_to(seek_pos)?;
        Ok(())
    }

    /// Seek absolutely
    pub fn seek_to(&mut self, target_pos: Duration) -> Result<(), Box<dyn Error>> {
        let start;
        if self.start.read().unwrap().is_none() {
            return Err("Failed to seek: No START time".into());
        } else {
            start = self.start.read().unwrap().unwrap();
        }

        let end;
        if self.end.read().unwrap().is_none() {
            return Err("Failed to seek: No END time".into());
        } else {
            end = self.end.read().unwrap().unwrap();
        }

        let adjusted_target = target_pos + start;
        let clamped_target = adjusted_target.clamp(start, end);

        let seek_pos_clock =
            ClockTime::from_useconds(clamped_target.num_microseconds().unwrap() as u64);

        self.set_gstreamer_volume(0.0);
        self.playbin
            .write()
            .unwrap()
            .seek_simple(gst::SeekFlags::FLUSH, seek_pos_clock)?;
        self.set_gstreamer_volume(self.volume);
        Ok(())
    }

    /// Get the current state of the playback
    pub fn state(&mut self) -> PlayerState {
        match *self.buffer.read().unwrap() {
            None => self.playbin.read().unwrap().current_state().into(),
            Some(value) => PlayerState::Buffering(value),
        }
    }

    pub fn property(&self, property: &str) -> glib::Value {
        self.playbin.read().unwrap().property_value(property)
    }

    /// Stop the playback entirely
    pub fn stop(&mut self) -> Result<(), gst::StateChangeError> {
        self.pause()?;
        self.ready()?;

        // Set all positions to none
        *self.position.write().unwrap() = None;
        *self.start.write().unwrap() = None;
        *self.end.write().unwrap() = None;
        Ok(())
    }
}

impl Drop for Player {
    /// Cleans up `GStreamer` pipeline when `Backend` is dropped.
    fn drop(&mut self) {
        self.playbin
            .write()
            .unwrap()
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
    }
}
