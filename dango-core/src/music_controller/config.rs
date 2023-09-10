use std::path::PathBuf;
use std::fs::read_to_string;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::music_tracker::music_tracker::{LastFM, LastFMConfig, DiscordRPCConfig};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub db_path: Box<PathBuf>,
    pub lastfm: Option<LastFMConfig>,
    pub discord: Option<DiscordRPCConfig>,
}

impl Default for Config {
    fn default() -> Self {
        let path = PathBuf::from("./music_database.db3");

        return Config {
            db_path: Box::new(path),

            lastfm: Some (LastFMConfig {
                enabled: true,
                dango_api_key: String::from("29a071e3113ab8ed36f069a2d3e20593"),
                auth_token: None,
                shared_secret: Some(String::from("5400c554430de5c5002d5e4bcc295b3d")),
                session_key: None,
            }),

            discord: Some(DiscordRPCConfig {
                enabled: true,
                dango_client_id: 1144475145864499240,
                dango_icon: String::from("flat"),
            }),
        };
    }
}

impl Config {
    // Creates and saves a new config with default values
    pub fn new(config_file: &PathBuf) -> std::io::Result<Config> {        
        let config = Config::default();
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
