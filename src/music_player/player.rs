use chrono::Duration;
use thiserror::Error;
use gstreamer as gst;

use crate::music_storage::library::URI;


#[derive(Error, Debug)]
pub enum PlayerError {
    #[error("player initialization failed")]
    Init(#[from] glib::Error),
    #[error("element factory failed to create playbin3")]
    Factory(#[from] glib::BoolError),
    #[error("could not change playback state")]
    StateChange(#[from] gst::StateChangeError),
    #[error("the file or source is not found")]
    NotFound,
    #[error("failed to build gstreamer item")]
    Build,
    #[error("poison error")]
    Poison,
    #[error("general player error")]
    General,
}

pub trait Player {
    fn source(&self) -> &Option<URI>;

    fn enqueue_next(&mut self, next_track: &URI) -> Result<(), PlayerError>;

    fn set_volume(&mut self, volume: f64);

    fn volume(&mut self) -> f64;

    fn ready(&mut self) -> Result<(), PlayerError>;

    fn play(&mut self) -> Result<(), PlayerError>;

    fn resume(&mut self) -> Result<(), PlayerError>;

    fn pause(&mut self) -> Result<(), PlayerError>;

    fn stop(&mut self) -> Result<(), PlayerError>;

    fn is_paused(&mut self) -> bool;

    fn position(&mut self) -> Option<Duration>;

    fn duration(&mut self) -> Option<Duration>;

    fn raw_duration(&self) -> Option<Duration>;

    fn seek_by(&mut self, seek_amount: Duration) -> Result<(), PlayerError>;

    fn seek_to(&mut self, target_pos: Duration) -> Result<(), PlayerError>;

}