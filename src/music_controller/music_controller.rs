use rusqlite::{Connection, Result};
use crate::{music_controller::config::Config, music_player::{music_player::{MusicPlayer, PlayerStatus, PlayerMessage}, self}};
use std::{path::PathBuf, task::Context};

pub struct MusicController {
    pub config: Config,
    music_player: MusicPlayer,
}

impl MusicController {
    pub fn new(config_path: &PathBuf) -> Result<MusicController, std::io::Error>{
        let config = Config::new(config_path)?;
        let music_player = MusicPlayer::new();
        
        let controller = MusicController {
            config,
            music_player,
        };
        
        return Ok(controller)
    }
    
    pub fn from(config_path: &PathBuf) -> std::result::Result<MusicController, toml::de::Error> {
        let config = Config::from(config_path)?;
        let music_player = MusicPlayer::new();
        
        let controller = MusicController {
            config,
            music_player,
        };
        
        return Ok(controller)
    }
    
    pub fn open_song(&mut self, path: &Box<PathBuf>) {
        self.music_player.open_song(path);
    }
    
    pub fn song_control(&mut self, message: PlayerMessage) {
        self.music_player.send_message(message);
    }
    
    pub fn player_status(&mut self) -> PlayerStatus {
        return self.music_player.get_status();
    }
}
