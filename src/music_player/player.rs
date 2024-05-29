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

#[derive(Debug, PartialEq, Eq)]
pub enum PlayerCommand {
    Play,
    Pause,
    EndOfStream,
    AboutToFinish,
}

pub trait Player {
    /// Create a new player.
    fn new() -> Result<Self, PlayerError> where Self: Sized;

    /// Get the currently playing [URI] from the player.
    fn source(&self) -> &Option<URI>;

    /// Insert a new [`URI`] to be played. This method should be called at the
    /// beginning to start playback of something, and once the [`PlayerCommand`]
    /// indicates the track is about to finish to enqueue gaplessly.
    ///
    /// For backends which do not support gapless playback, `AboutToFinish`
    /// will not be called, and the next [`URI`] should be enqueued once `Eos`
    /// occurs.
    fn enqueue_next(&mut self, next_track: &URI) -> Result<(), PlayerError>;

    /// Set the playback volume, accepts a float from `0` to `1`.
    ///
    /// Values outside the range of `0` to `1` will be capped.
    fn set_volume(&mut self, volume: f64);

    /// Returns the current volume level, a float from `0` to `1`.
    fn volume(&self) -> f64;

    /// If the player is paused or stopped, starts playback.
    fn play(&mut self) -> Result<(), PlayerError>;

    /// If the player is playing, pause playback.
    fn pause(&mut self) -> Result<(), PlayerError>;

    /// Stop the playback entirely, removing the current [`URI`] from the player.
    fn stop(&mut self) -> Result<(), PlayerError>;

    /// Convenience function to check if playback is paused.
    fn is_paused(&self) -> bool;

    /// Get the current playback position of the player.
    fn position(&self) -> Option<Duration>;

    /// Get the duration of the currently playing track.
    fn duration(&self) -> Option<Duration>;

    /// Seek relative to the current position.
    ///
    /// The position is capped at the duration of the song, and zero.
    fn seek_by(&mut self, seek_amount: Duration) -> Result<(), PlayerError>;

    /// Seek absolutely within the song.
    ///
    /// The position is capped at the duration of the song, and zero.
    fn seek_to(&mut self, target_pos: Duration) -> Result<(), PlayerError>;

    /// Return a reference to the player message channel, which can be cloned
    /// in order to monitor messages from the player.
    fn message_channel(&self) -> &crossbeam::channel::Receiver<PlayerCommand>;
}
