use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::fs::{File, read_to_string};
use std::io::Read;

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
}
