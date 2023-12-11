use chrono::Duration;
use walkdir::Error;

use crate::music_controller::config::Config;
use std::{default, path::Path, thread::AccessError};

use super::{
    library::{self, AlbumArt, Song, Tag},
    music_collection::MusicCollection,
};

#[derive(Debug, Clone)]
pub struct Playlist<'a> {
    title: String,
    cover: Option<&'a AlbumArt>,
    tracks: Vec<&'a Song>,
    play_count: i32,
    play_time: Duration,
}
impl<'a> Playlist<'a> {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn play_count(&self) -> i32 {
        self.play_count
    }
    pub fn play_time(&self) -> chrono::Duration {
        self.play_time
    }
    pub fn set_tracks(&mut self, songs: Vec<&'a Song>) -> Result<(), Error> {
        self.tracks = songs;
        Ok(())
    }
    pub fn add_track(&mut self, song: &'a Song) -> Result<(), Error> {
        self.tracks.push(song);
        Ok(())
    }
    pub fn remove_track(&mut self, index: i32) -> Result<(), Error> {
        let bun: usize = index as usize;
        let mut name = String::new();
        if self.tracks.len() >= bun {
            name = String::from(self.tracks[bun].tags.get_key_value(&Tag::Title).unwrap().1);
            self.tracks.remove(bun);
        }
        dbg!(name);
        Ok(())
    }
    pub fn get_index(&self, song_name: &str) -> Option<usize> {
        let mut index = 0;
        if self.contains(&Tag::Title, song_name) {
            for track in &self.tracks {
                index += 1;
                if song_name == track.tags.get_key_value(&Tag::Title).unwrap().1 {
                    dbg!("Index gotted! ", index);
                    return Some(index);
                }
            }
        }
        None
    }
    pub fn contains(&self, tag: &Tag, title: &str) -> bool {
        for track in &self.tracks {
            if title == track.tags.get_key_value(tag).unwrap().1 {
                return true;
            }
        }
        false
    }
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
impl Default for Playlist<'_> {
    fn default() -> Self {
        Playlist {
            title: String::default(),
            cover: None,
            tracks: Vec::default(),
            play_count: -1,
            play_time: Duration::zero(),
        }
    }
}
