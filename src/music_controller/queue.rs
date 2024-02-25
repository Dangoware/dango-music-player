use crate::{music_player::Player, music_storage::library::Song};
use std::error::Error;

pub struct Queue {
    pub player: Player,
    pub name: String,
    pub songs: Vec<Song>,
}
impl Queue {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(
            Queue {
            player: Player::new()?,
            name: String::new(),
            songs: Vec::new()
            }
        )
    }

    pub fn set_tracks(&mut self, tracks: Vec<Song>) {
        let mut tracks = tracks;
        self.songs.clear();
        self.songs.append(&mut tracks);
    }
}
