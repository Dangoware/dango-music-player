use std::vec::IntoIter;

use crate::music_storage::library::{Album, AlbumTrack, Song};

use super::controller::PlayerLocation;

#[derive(Debug, Clone, PartialEq)]
pub struct QueueSong {
    pub song: Song,
    pub location: PlayerLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueueAlbum {
    pub album: Album,
    pub location: PlayerLocation,
}

impl IntoIterator for QueueAlbum {
    type Item = AlbumTrack;
    type IntoIter = IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.album.into_iter()
    }
}
