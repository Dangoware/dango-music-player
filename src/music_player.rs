// Crate things
use crate::music_controller::config::Config;
use crate::music_storage::music_db::URI;
use std::error::Error;
use std::sync::mpsc::{Sender, self, Receiver};

// GStreamer things
use gst::{ClockTime, Element};
use gstreamer as gst;
use gstreamer::prelude::*;
use glib::FlagsClass;

// Time things
use chrono::Duration;

enum PlayerCmd {
    Play,
}

/// An instance of a music player with a GStreamer backend
pub struct Player {
    source: Option<URI>,
    events: Sender<PlayerCmd>,
    playbin: Element,
    position: Duration,
    duration: Duration,
    paused: bool,
    volume: f64,
    gapless: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}


impl Player {
    pub fn new() -> Self {
        gst::init().unwrap();

        let playbin = gst::ElementFactory::make("playbin")
            .build()
            .unwrap();

        let flags = playbin.property_value("flags");
        let flags_class = FlagsClass::with_type(flags.type_()).unwrap();

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

        playbin
            .bus()
            .expect("Failed to get GStreamer message bus");

        let source = None;
        let (tx, _): (Sender<PlayerCmd>, Receiver<PlayerCmd>) = mpsc::channel();
        Self {
            source,
            events: tx,
            playbin,
            paused: false,
            volume: 0.5,
            gapless: false,
            position: Duration::seconds(0),
            duration: Duration::seconds(0),
        }
    }

    pub fn enqueue_next(&mut self, next_track: URI) {
        self.set_state(gst::State::Ready);

        self.playbin.set_property("uri", next_track.as_uri());

        self.play();
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

    /// Set the playback URI
    pub fn set_source(&mut self, source: URI) {
        self.source = Some(source.clone());
        self.playbin.set_property("uri", source.as_uri())
    }

    /// Get the current playback position of the player
    pub fn position(&mut self) -> Option<Duration> {
        self.playbin.query_position::<ClockTime>().map(|pos| Duration::nanoseconds(pos.nseconds() as i64))
    }

    /// Get the duration of the currently playing track
    pub fn duration(&mut self) -> Option<Duration> {
        self.playbin.query_duration::<ClockTime>().map(|pos| Duration::milliseconds(pos.mseconds() as i64))
    }

    /// Seek relative to the current position
    pub fn seek_by(&mut self, seek_amount: Duration) -> Result<(), Box<dyn Error>> {
        let time_pos = match self.position() {
            Some(pos) => pos,
            None => return Err("No position".into())
        };
        let seek_pos = time_pos + seek_amount;

        self.seek_to(seek_pos)?;
        Ok(())
    }

    /// Seek absolutely
    pub fn seek_to(&mut self, last_pos: Duration) -> Result<(), Box<dyn Error>> {
        let duration = match self.duration() {
            Some(dur) => dur,
            None => return Err("No duration".into())
        };
        let seek_pos = last_pos.clamp(Duration::seconds(0), duration);

        let seek_pos_clock = ClockTime::from_mseconds(seek_pos.num_milliseconds() as u64);
        self.set_gstreamer_volume(0.0);
        self
            .playbin
            .seek_simple(gst::SeekFlags::FLUSH, seek_pos_clock)?;
        self.set_gstreamer_volume(self.volume);
        Ok(())
    }
}
