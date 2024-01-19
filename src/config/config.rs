use std::{path::{PathBuf, Path}, marker::PhantomData, fs::{File, OpenOptions, self}, io::{Error, Write, Read}, default};

use serde::{Serialize, Deserialize};
use serde_json::{to_string, to_string_pretty};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigLibrary {
    pub name: String,
    pub path: PathBuf,
    pub uuid: Uuid
}
impl ConfigLibrary {
    pub fn new() -> Self {
        ConfigLibrary::default()
    }
    pub fn open(&self) -> Result<File, Error> {
        match File::open(self.path.as_path()) {
            Ok(ok) => Ok(ok),
            Err(e) => Err(e)
        }
    }
}
impl Default for ConfigLibrary {
    fn default() -> Self {
        ConfigLibrary {
            name: String::default(),
            path: PathBuf::default(),
            uuid: Uuid::new_v4()
        }
    }
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub path: PathBuf,
    default_library: Uuid,
    pub libraries: Vec<ConfigLibrary>,
    pub library_folder: PathBuf,
    volume: f32,
}

impl Config {
    pub fn new() -> Self {
        Config {
            libraries: vec![ConfigLibrary::default()],
            ..Default::default()
        }
    }
    pub fn new_main() -> Self {
        Config::default()
    }
    //TODO: Add new function for test tube
    pub fn set_default_library(mut self, uuid: &Uuid) {
        self.default_library = *uuid;
    }
    pub fn get_default_library(&self) -> Result<&ConfigLibrary, ConfigError> {
        for library in &self.libraries {
            if library.uuid == self.default_library {
                return Ok(library)
            }
        }
        Err(ConfigError::NoDefaultLibrary)
    }
    pub fn get_library(&self, uuid: &Uuid) -> Result<ConfigLibrary, ConfigError> {
        for library in &self.libraries {
            if &library.uuid == uuid {
                return Ok(library.to_owned())
            }
        }
        Err(ConfigError::NoConfigLibrary(*uuid))
    }
    pub fn library_exists(&self, uuid: &Uuid) -> bool {
        for library in &self.libraries {
            if &library.uuid == uuid {
                return true
            }
        }
        false
    }
    pub fn to_file(&self) -> Result<(), Error> {
        let mut writer = self.path.clone();
        writer.set_extension("tmp");
        let mut file = OpenOptions::new().create(true).truncate(true).read(true).write(true).open(&writer)?;
        let config = to_string_pretty(self)?;

        file.write_all(&config.as_bytes())?;
        fs::rename(writer, self.path.as_path())?;
        Ok(())
    }
    pub fn load_file(path: PathBuf) -> Result<Self, Error> {
        let mut file: File = File::open(path)?;
        let mut bun: String = String::new();
        _ = file.read_to_string(&mut bun);
        let ny: Config = serde_json::from_str::<Config>(&bun)?;
        Ok(ny)
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("No Library Found for {0}!")]
    NoConfigLibrary(Uuid),
    #[error("There is no Default Library for this Config")]
    NoDefaultLibrary
}
