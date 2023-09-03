use rusqlite::{Connection, Result};
use crate::{music_controller::config::Config, music_player::{music_player::{MusicPlayer, PlayerStatus, PlayerMessage, DSPMessage}, self}, music_processor::{music_processor::MusicProcessor, self}, music_storage::music_db::URI};
use std::{path::PathBuf, task::Context};

pub struct MusicController {
    pub config: Config,
    music_player: MusicPlayer,
}

impl MusicController {
    // Creates new MusicController with config at given path
    pub fn new(config_path: &PathBuf) -> Result<MusicController, std::io::Error>{
        let config = Config::new(config_path)?;
        let music_processor = MusicProcessor::new();
        let music_player = MusicPlayer::new();
        
        let controller = MusicController {
            config,
            music_player,
        };
        
        return Ok(controller)
    }
        
    // Creates new music controller from a config at given path
    pub fn from(config_path: &PathBuf) -> std::result::Result<MusicController, toml::de::Error> {
        let config = Config::from(config_path)?;
        let music_player = MusicPlayer::new();
        
        let controller = MusicController {
            config,
            music_player,
        };
        
        return Ok(controller)
    }
    
    // Opens and plays song at given path
    pub fn open_song(&mut self, uri: &URI) {
        self.music_player.open_song(uri);
    }
    
    // Sends message to music player to play, pause, stop and seek
    pub fn song_control(&mut self, message: PlayerMessage) {
        self.music_player.send_message(message);
    }
    
    // Gets status of the music player
    pub fn player_status(&mut self) -> PlayerStatus {
        return self.music_player.get_status();
    }
    
    // Gets audio playback volume
    pub fn get_vol(&mut self) -> f32 {
        return self.music_player.music_processor.audio_volume;
    }
    
    // Sets audio playback volume
    pub fn set_vol(&mut self, volume: f32) {
        self.music_player.music_processor.audio_volume = volume;
        self.song_control(PlayerMessage::DSP(DSPMessage::UpdateProcessor(Box::new(self.music_player.music_processor.clone()))));
    }
    
}
