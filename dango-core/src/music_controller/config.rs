use std::path::PathBuf;
use std::fs::read_to_string;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::music_tracker::music_tracker::LastFM;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub db_path: Box<PathBuf>,
    pub lastfm: Option<LastFM>,
}

impl Config {
    // Creates and saves a new config with default values
    pub fn new(config_file: &PathBuf) -> std::io::Result<Config> {
        let path = PathBuf::from("./music_database.db3");
        
        let config = Config {
            db_path: Box::new(path),
            lastfm: None,
        };
        config.save(config_file)?;
        
        Ok(config)
    }

    // Loads config from given file path
    pub fn from(config_file: &PathBuf) -> std::result::Result<Config, toml::de::Error> {
        return toml::from_str(&read_to_string(config_file)
            .expect("Failed to initalize music config: File not found!"));
    }
    
    // Saves config to given path
    // Saves -> temp file, if successful, removes old config, and renames temp to given path
    pub fn save(&self, config_file: &PathBuf) -> std::io::Result<()> {
        let toml = toml::to_string_pretty(self).unwrap();
        
        let mut temp_file = config_file.clone();
        temp_file.set_extension("tomltemp");
        
        fs::write(&temp_file, toml)?;

        // If configuration file already exists, delete it
        match fs::metadata(config_file) {
            Ok(_) => fs::remove_file(config_file)?,
            Err(_) => {},
        }

        fs::rename(temp_file, config_file)?;
        Ok(())
    }
}
