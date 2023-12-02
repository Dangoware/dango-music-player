// Crate things
//use crate::music_controller::config::Config;
use crate::music_storage::music_db::URI;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, Sender};

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
    pub message_rx: Receiver<PlayerCmd>,
    playbin: Element,
    paused: bool,
    volume: f64,
    start: Option<Duration>,
    end: Option<Duration>,
    position: Option<Duration>,
    gapless: bool,
}

impl Player {
    pub fn new() -> Self {
        // Initialize GStreamer
        gst::init().unwrap();

        let playbin = gst::ElementFactory::make("playbin").build().unwrap();

        let flags = playbin.property_value("flags");
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
        playbin.set_property_from_value("flags", &flags);


        let (message_tx, message_rx) = std::sync::mpsc::channel();
        playbin.connect("about-to-finish", false, move |_| {
                println!("test");
                message_tx.send(PlayerCmd::AboutToFinish).unwrap();
                None
            });

        let source = None;
        Self {
            source,
            playbin,
            message_rx,
            paused: false,
            volume: 0.5,
            gapless: false,
            start: None,
            end: None,
            position: None,
        }
    }

    pub fn source(&self) -> &Option<URI> {
        &self.source
    }

    pub fn enqueue_next(&mut self, next_track: URI) {
        self.set_state(gst::State::Ready);

        self.set_source(next_track);

        self.play();
    }

    /// Set the playback URI
    pub fn set_source(&mut self, source: URI) {
        self.source = Some(source.clone());
        match source {
            URI::Cue {start, ..} => {
                self.playbin.set_property("uri", source.as_uri());
                self.play();
                while self.state() != gst::State::Playing {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                };
                self.seek_to(Duration::from_std(start).unwrap()).unwrap();
            }
            _ => self.playbin.set_property("uri", source.as_uri()),
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
        self.playbin.set_property("volume", volume)
    }

    /// Returns the current volume level, a float from 0 to 1
    pub fn volume(&mut self) -> f64 {
        self.volume
    }

    fn set_state(&mut self, state: gst::State) {
        self.playbin
            .set_state(state)
            .expect("Unable to set the pipeline state");
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
        self.playbin.current_state() == gst::State::Paused
    }

    /// Get the current playback position of the player
    pub fn position(&mut self) -> Option<Duration> {
        self.position = self
            .playbin
            .query_position::<ClockTime>()
            .map(|pos| Duration::nanoseconds(pos.nseconds() as i64));
        self.position
    }

    /// Get the duration of the currently playing track
    pub fn duration(&mut self) -> Option<Duration> {
        if self.end.is_some() && self.start.is_some() {
            Some(self.end.unwrap() - self.start.unwrap())
        } else {
            None
        }
    }

    /// Seek relative to the current position
    pub fn seek_by(&mut self, seek_amount: Duration) -> Result<(), Box<dyn Error>> {
        let time_pos = match self.position() {
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
            .seek_simple(gst::SeekFlags::FLUSH, seek_pos_clock)?;
        self.set_gstreamer_volume(self.volume);
        Ok(())
    }

    pub fn state(&mut self) -> gst::State {
        self.playbin.current_state()
    }
}

impl Drop for Player {
    /// Cleans up `GStreamer` pipeline when `Backend` is dropped.
    fn drop(&mut self) {
        self.playbin
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
    }
}
