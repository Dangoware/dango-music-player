use walkdir::Error;

use crate::music_controller::config::Config;
use std::{path::Path, default, thread::AccessError};

use super::{library::{AlbumArt, Song}, music_collection::MusicCollection};

#[derive(Debug, Default)]
pub struct Playlist<'a> {
    title: String,
    cover: Option<&'a AlbumArt>,
    tracks: Vec<&'a Song>,
}
impl MusicCollection for Playlist<'_> {
    fn title(&self) -> &String {
        &self.title
    }
    fn cover(&self) -> Option<&AlbumArt> {
        match self.cover {
            Some(e) => Some(e),
            None => None,
        }
    }
    fn tracks(&self) -> Vec<&Song> {
            self.tracks.clone()
    }
}
