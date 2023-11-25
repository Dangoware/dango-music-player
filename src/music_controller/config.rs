use std::fs;
use std::fs::read_to_string;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub db_path: Box<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        let path = PathBuf::from("./music_database");

        Config {
            db_path: Box::new(path),
        }
    }
}

impl Config {
    /// Creates and saves a new config with default values
    pub fn new(config_file: &PathBuf) -> std::io::Result<Config> {
        let config = Config::default();
        config.save(config_file)?;

        Ok(config)
    }

    /// Loads config from given file path
    pub fn from(config_file: &PathBuf) -> std::result::Result<Config, toml::de::Error> {
        toml::from_str(
            &read_to_string(config_file)
                .expect("Failed to initalize music config: File not found!"),
        )
    }

    /// Saves config to given path
    /// Saves -> temp file, if successful, removes old config, and renames temp to given path
    pub fn save(&self, config_file: &PathBuf) -> std::io::Result<()> {
        let toml = toml::to_string_pretty(self).unwrap();

        let mut temp_file = config_file.clone();
        temp_file.set_extension("tomltemp");

        fs::write(&temp_file, toml)?;

        // If configuration file already exists, delete it
        if fs::metadata(config_file).is_ok() {
            fs::remove_file(config_file)?
        }

        fs::rename(temp_file, config_file)?;
        Ok(())
    }
}
