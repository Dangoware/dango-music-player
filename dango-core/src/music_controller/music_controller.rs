use std::path::PathBuf;
use std::sync::{RwLock, Arc};

use crate::music_controller::config::Config;
use crate::music_player::music_player::{MusicPlayer, PlayerStatus, DecoderMessage, DSPMessage};
use crate::music_storage::music_db::{MusicLibrary, Song, Tag};

pub struct MusicController {
    pub config: Arc<RwLock<Config>>,
    pub library: MusicLibrary,
    music_player: MusicPlayer,
}

impl MusicController {
    /// Creates new MusicController with config at given path
    pub fn new(config_path: &PathBuf) -> Result<MusicController, Box<dyn std::error::Error>>{
        let config = Arc::new(RwLock::new(Config::new(config_path)?));
        let music_player = MusicPlayer::new(config.clone());
        let library = match MusicLibrary::init(config.clone()) {
            Ok(library) => library,
            Err(error) => return Err(error)
        };
        
        let controller = MusicController {
            config,
            library,
            music_player,
        };
        
        return Ok(controller)
    }
        
    /// Creates new music controller from a config at given path
    pub fn from(config_path: &PathBuf) -> Result<MusicController, Box<dyn std::error::Error>> {
        let config = Arc::new(RwLock::new(Config::from(config_path)?));
        let music_player = MusicPlayer::new(config.clone());
        let library = match MusicLibrary::init(config.clone()) {
            Ok(library) => library,
            Err(error) => return Err(error)
        };
        
        let controller = MusicController {
            config,
            library,
            music_player,
        };
        
        return Ok(controller)
    }
    
    /// Sends given message to control music player
    pub fn song_control(&mut self, message: DecoderMessage) {
        self.music_player.send_message(message);
    }
    
    /// Gets status of the music player
    pub fn player_status(&mut self) -> PlayerStatus {
        return self.music_player.get_status();
    }
    
    /// Gets current song being controlled, if any
    pub fn get_current_song(&self) -> Option<Song> {
        return self.music_player.get_current_song();
    }
    
    /// Gets audio playback volume
    pub fn get_vol(&self) -> f32 {
        return self.music_player.music_processor.audio_volume;
    }
    
    /// Sets audio playback volume on a scale of 0.0 to 1.0
    pub fn set_vol(&mut self, volume: f32) {
        self.music_player.music_processor.audio_volume = volume;
        self.song_control(DecoderMessage::DSP(DSPMessage::UpdateProcessor(Box::new(self.music_player.music_processor.clone()))));
    }
    
    /// Queries the library for a `Vec<Song>`
    pub fn query_library(
        &self,
        query_string: &String,
        target_tags: Vec<Tag>,
        sort_by: Vec<Tag>
    ) -> Option<Vec<&Song>> {
        self.library.query(query_string, &target_tags, &sort_by)
    }
}
