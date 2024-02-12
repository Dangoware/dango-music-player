use std::{
    path::PathBuf,
    fs::{File, OpenOptions, self},
    io::{Error, Write, Read}, sync::{Arc, RwLock},
};

use serde::{Serialize, Deserialize};
use serde_json::to_string_pretty;
use thiserror::Error;
use uuid::Uuid;

use crate::music_storage::library::{MusicLibrary, self};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigLibrary {
    pub name: String,
    pub path: PathBuf,
    pub uuid: Uuid,
    pub scan_folders: Option<Vec<PathBuf>>,
}

impl Default for ConfigLibrary {
    fn default() -> Self {
        ConfigLibrary {
            name: String::new(),
            path: PathBuf::from("library"),
            uuid: Uuid::new_v4(),
            scan_folders: None,
        }
    }
}

impl ConfigLibrary {
    pub fn new(path: PathBuf, name: String, scan_folders: Option<Vec<PathBuf>>) -> Self {
        ConfigLibrary {
            name,
            path,
            uuid: Uuid::new_v4(),
            scan_folders,
        }
    }

    pub fn open(&self) -> Result<File, Error> {
        match File::open(self.path.as_path()) {
            Ok(ok) => Ok(ok),
            Err(e) => Err(e)
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ConfigLibraries {
    pub default_library: Uuid,
    pub library_folder: PathBuf,
    pub libraries: Vec<ConfigLibrary>,
}

impl ConfigLibraries {
    //TODO: Add new function for test tube
    pub fn set_default(mut self, uuid: &Uuid) {
        self.default_library = *uuid;
    }

    pub fn get_default(&self) -> Result<&ConfigLibrary, ConfigError> {
        for library in &self.libraries {
            if library.uuid == self.default_library {
                return Ok(library)
            }
        }
        Err(ConfigError::NoDefaultLibrary)
    }

    pub fn get_library(&self, uuid: &Uuid) -> Result<ConfigLibrary, ConfigError> {
        dbg!(&uuid);
        for library in &self.libraries {
            if &library.uuid == uuid {
                dbg!(&library.uuid);
                return Ok(library.to_owned())
            }
        }
        Err(ConfigError::NoConfigLibrary(*uuid))
    }

    pub fn uuid_exists(&self, uuid: &Uuid) -> bool {
        for library in &self.libraries {
            if &library.uuid == uuid {
                return true
            }
        }
        false
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Config {
    pub path: PathBuf,
    pub backup_folder: Option<PathBuf>,
    pub libraries: ConfigLibraries,
    pub volume: f32,
}

impl Config {
    pub fn new() -> Self {
        Config {
            libraries: ConfigLibraries {
                libraries: vec![ConfigLibrary::default()],
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn new_main() -> Self {
        Config::default()
    }

    pub fn write_file(&self) -> Result<(), Error> {
        let mut writer = self.path.clone();
        writer.set_extension("tmp");
        let mut file = OpenOptions::new().create(true).truncate(true).read(true).write(true).open(&writer)?;
        let config = to_string_pretty(self)?;
        // dbg!(&config);

        file.write_all(config.as_bytes())?;
        fs::rename(writer, self.path.as_path())?;
        Ok(())
    }

    pub fn read_file(path: PathBuf) -> Result<Self, Error> {
        let mut file: File = File::open(path)?;
        let mut bun: String = String::new();
        _ = file.read_to_string(&mut bun);
        let config: Config = serde_json::from_str::<Config>(&bun)?;
        Ok(config)
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("No Library Found for {0}!")]
    NoConfigLibrary(Uuid),
    #[error("There is no Default Library for this Config")]
    NoDefaultLibrary,
    //TODO: do something about playlists
    #[error("Please provide a better m3u8 Playlist")]
    BadPlaylist,

}

#[test]
fn config_test() {
    let lib_a = ConfigLibrary::new(PathBuf::from("test-config/library1"), String::from("library1"), None);
    let lib_b = ConfigLibrary::new(PathBuf::from("test-config/library2"), String::from("library2"), None);
    let lib_c = ConfigLibrary::new(PathBuf::from("test-config/library3"), String::from("library3"), None);
    let config = Config {
        path: PathBuf::from("test-config/config_test.json"),
        libraries: ConfigLibraries {
            libraries: vec![
                lib_a.clone(),
                lib_b.clone(),
                lib_c.clone(),
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    config.write_file();
    let arc = Arc::new(RwLock::from(config));
    MusicLibrary::init(arc.clone(), lib_a.uuid.clone()).unwrap();
    MusicLibrary::init(arc.clone(), lib_b.uuid.clone()).unwrap();
    MusicLibrary::init(arc.clone(), lib_c.uuid.clone()).unwrap();

}

#[test]
fn test2() {
    let config = Config::read_file(PathBuf::from("test-config/config_test.json")).unwrap();
    let uuid = config.libraries.get_default().unwrap().uuid.clone();
    let mut lib = MusicLibrary::init(Arc::new(RwLock::from(config.clone())), uuid).unwrap();
    lib.scan_folder("test-config/music/").unwrap();
    lib.save(config.clone()).unwrap();
    dbg!(&lib);
    dbg!(&config);
}

#[test]
fn test3() {
    let config = Config::read_file(PathBuf::from("test-config/config_test.json")).unwrap();
    let uuid = config.libraries.get_default().unwrap().uuid;
    let mut lib = MusicLibrary::init(Arc::new(RwLock::from(config.clone())), uuid).unwrap();

    dbg!(lib);
}