use chrono::Duration;
use thiserror::Error;

use crate::music_storage::library::URI;

#[derive(Error, Debug)]
pub enum PlayerError {
    #[error("player initialization failed: {0}")]
    Init(String),
    #[error("could not change playback state")]
    StateChange(String),
    #[error("seeking failed: {0}")]
    Seek(String),
    #[error("the file or source is not found")]
    NotFound,
    #[error("failed to build gstreamer item")]
    Build,
    #[error("poison error")]
    Poison,
    #[error("general player error")]
    General(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum PlayerState {
    Playing,
    Paused,
    Ready,
    Buffering(u8),
    Null,
    VoidPending,
}

#[derive(Debug)]
pub enum PlayerCommand {
    Play,
    Pause,
    EndOfStream,
    AboutToFinish,
}

pub trait Player {
    /// Create a new player
    fn new() -> Result<Self, PlayerError> where Self: Sized;

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

    fn seek_by(&mut self, seek_amount: Duration) -> Result<(), PlayerError>;

    fn seek_to(&mut self, target_pos: Duration) -> Result<(), PlayerError>;

    fn message_channel(&self) -> &crossbeam::channel::Receiver<PlayerCommand>;
}
