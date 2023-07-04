use rusqlite::{Connection, Result};
use crate::music_controller::config::Config;
use std::{path::PathBuf, task::Context};

enum Status {
    Playing,
    Pausing,
    Paused,
    Stopping,
    Stopped
}

pub struct MusicController {
    pub connection: Connection,
    config: Config,
    playback_status: Status,
}

impl MusicController {
    pub fn new(config_path: &PathBuf) -> Result<MusicController, std::io::Error>{
        let config = Config::new(config_path)?;
        let connection = Connection::open(&*config.db_path).unwrap();
        
        let controller = MusicController {
            config,
            connection,
            playback_status: Status::Stopped,
        };
        
        return Ok(controller)
    }
    
    pub fn from(config_path: &PathBuf) -> std::result::Result<MusicController, toml::de::Error> {
        let config = Config::from(config_path)?;
        let connection = Connection::open(&*config.db_path).unwrap();

        let controller = MusicController {
            config,
            connection,
            playback_status: Status::Stopped,
        };
        
        return Ok(controller)
    }
}
