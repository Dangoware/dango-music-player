// Crate things
//use crate::music_controller::config::Config;
use crate::music_storage::music_db::URI;
use std::error::Error;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::{self, Receiver, Sender};
use crossbeam_channel::bounded;

// GStreamer things
use glib::{FlagsClass, MainContext};
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

/// An instance of a music player with a GStreamer backend
pub struct Player {
    source: Option<URI>,
    //pub message_tx: Sender<PlayerCmd>,
    pub message_rx: crossbeam::channel::Receiver<PlayerCmd>,
    playbin: Arc<RwLock<Element>>,
    paused: bool,
    volume: f64,
    start: Arc<RwLock<Option<Duration>>>,
    end: Arc<RwLock<Option<Duration>>>,
    pub position: Arc<RwLock<Option<Duration>>>,
    gapless: bool,
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

        let playbin_arc = Arc::new(RwLock::new(gst::ElementFactory::make("playbin3").build().unwrap()));

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

        playbin.write().unwrap().set_property_from_value("flags", &flags);

        let position = Arc::new(RwLock::new(None));
        let start = Arc::new(RwLock::new(None));
        let end: Arc<RwLock<Option<Duration>>> = Arc::new(RwLock::new(None));

        let position_update = position.clone();
        let start_update = start.clone();
        let end_update = end.clone();
        let (message_tx, message_rx) = bounded(1);
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
                    if let Some(time) = *start_update.read().unwrap() { pos_temp = Some(pos_temp.unwrap() - time) }

                    let atf = end_update.read().unwrap().unwrap() - Duration::milliseconds(100);
                    if pos_temp.unwrap() >= end_update.read().unwrap().unwrap() {
                        message_tx.try_send(PlayerCmd::Eos).unwrap();
                        playbin_arc
                            .write()
                            .unwrap()
                            .set_state(gst::State::Ready)
                            .expect("Unable to set the pipeline state");
                        *start_update.write().unwrap() = None;
                        *end_update.write().unwrap() = None;
                    } else if pos_temp.unwrap() >= atf {
                        match message_tx.try_send(PlayerCmd::AboutToFinish) {
                            Ok(_) => println!("Sent ATF"),
                            Err(err) => println!("{}", err),
                        }
                    }
                }

                *position_update.write().unwrap() = pos_temp;

                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        });

        /*
        playbin.read().unwrap().connect("about-to-finish", false, move |_| {
            //message_tx.send(PlayerCmd::AboutToFinish).unwrap();
            None
        });
        */

        let source = None;
        Self {
            source,
            playbin,
            message_rx,
            paused: false,
            volume: 1.0,
            gapless: false,
            start,
            end,
            position,
        }
    }

    pub fn source(&self) -> &Option<URI> {
        &self.source
    }

    pub fn enqueue_next(&mut self, next_track: &URI) {
        self.ready();
        self.set_source(next_track);
        self.play();
    }

    /// Set the playback URI
    fn set_source(&mut self, source: &URI) {
        self.source = Some(source.clone());
        match source {
            URI::Cue {start, end, ..} => {
                self.playbin.write().unwrap().set_property("uri", source.as_uri());

                // Set the start and end positions of the CUE file
                *self.start.write().unwrap() = Some(Duration::from_std(*start).unwrap());
                *self.end.write().unwrap() = Some(Duration::from_std(*end).unwrap());

                self.pause();

                // Wait for it to be ready, and then move to the proper position
                while self.playbin.read().unwrap().query_duration::<ClockTime>().is_none() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                };

                self.seek_to(Duration::from_std(*start).unwrap()).unwrap();
            },
            _ => {
                self.playbin.write().unwrap().set_property("uri", source.as_uri());

                self.pause();

                while self.playbin.read().unwrap().query_duration::<ClockTime>().is_none() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                };

                *self.start.write().unwrap() = Some(Duration::seconds(0));
                *self.end.write().unwrap() = self.playbin
                    .read()
                    .unwrap()
                    .query_duration::<ClockTime>()
                    .map(|pos| Duration::nanoseconds(pos.nseconds() as i64));
            },
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

    fn set_state(&mut self, state: gst::State) {
        self.playbin
            .write()
            .unwrap()
            .set_state(state)
            .expect("Unable to set the pipeline state");
    }

    pub fn ready(&mut self) {
        self.set_state(gst::State::Ready)
    }

    /// If the player is paused or stopped, starts playback
    pub fn play(&mut self) {
        self.set_state(gst::State::Playing);
    }

    /// Pause, if playing
    pub fn pause(&mut self) {
        self.paused = true;
        self.set_state(gst::State::Paused);
    }

    /// Resume from being paused
    pub fn resume(&mut self) {
        self.paused = false;
        self.set_state(gst::State::Playing);
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
            self.playbin
                .read()
                .unwrap()
                .query_duration::<ClockTime>()
                .map(|pos| Duration::nanoseconds(pos.nseconds() as i64))
        }
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
        let seek_pos_clock = ClockTime::from_useconds(target_pos.num_microseconds().unwrap() as u64);
        self.set_gstreamer_volume(0.0);
        self.playbin
            .write()
            .unwrap()
            .seek_simple(gst::SeekFlags::FLUSH, seek_pos_clock)?;
        self.set_gstreamer_volume(self.volume);
        Ok(())
    }

    pub fn state(&mut self) -> gst::State {
        self.playbin.read().unwrap().current_state()
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
