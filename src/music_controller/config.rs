use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::fs::{File, read_to_string};
use std::io::{Read, Write};
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub db_path: Box<PathBuf>,
}

impl Config {
    fn new() -> Config {
        let path = "./music_database.db3";
        
        Config {
            db_path: Box::new(PathBuf::new()),
        }   
    }

    pub fn from(config_file: &PathBuf) -> Config {
        return toml::from_str(&read_to_string(config_file)
            .expect("Failed to initalize music config: File not found!")).unwrap();
    }
    
    // Saves config to given path
    // Saves -> temp file, if successful, removes old config, and renames temp to given path
    pub fn save(&self, config_file: &PathBuf) -> std::io::Result<()> {
        let toml = toml::to_string_pretty(self).unwrap();
        
        let mut temp_file = config_file.clone();
        temp_file.set_extension("tomltemp");
        
        fs::write(&temp_file, toml)?;
        
        fs::remove_file(config_file)?;
        
        fs::rename(temp_file, config_file)?;
        Ok(())
    }
}
