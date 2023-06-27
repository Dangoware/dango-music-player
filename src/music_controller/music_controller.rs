use rusqlite::{Connection, Result};
use crate::music_controller::config::Config;
use std::path::PathBuf;

enum Status {
    Playing,
    Pausing,
    Paused,
    Stopping,
    Stopped
}

pub struct MusicController {
    connection: Connection,
    config: Config,
    playback_status: Status,
}

impl MusicController {
    pub fn from(config_path: &PathBuf) -> MusicController {
        let config = Config::from(config_path);
        let connection = Connection::open(*config.db_path).unwrap();

        MusicController{
            config,
            connection,
            playback_status: Status::Stopped;
        }
    }
}
