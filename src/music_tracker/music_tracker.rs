use async_trait::async_trait;

use crate::{music_storage::music_db::Song, music_controller::config::Config};

#[async_trait]
pub trait MusicTracker {
    async fn track_song(song: &Song);
    async fn track_now(song: &Song);
    async fn test_auth(config: &Config);
    async fn get_tracked(song: &Song);
}

