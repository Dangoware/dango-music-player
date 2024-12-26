use std::{
    fs::{self, File, OpenOptions},
    io::{Error, Read, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use thiserror::Error;
use uuid::Uuid;

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
            Err(e) => Err(e),
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
    pub fn set_default(mut self, uuid: &Uuid) {
        self.default_library = *uuid;
    }

    pub fn get_default(&self) -> Result<&ConfigLibrary, ConfigError> {
        for library in &self.libraries {
            if library.uuid == self.default_library {
                return Ok(library);
            }
        }
        Err(ConfigError::NoDefaultLibrary)
    }

    pub fn get_library(&self, uuid: &Uuid) -> Result<ConfigLibrary, ConfigError> {
        for library in &self.libraries {
            // dbg!(&library.uuid, &uuid);
            if &library.uuid == uuid {
                return Ok(library.to_owned());
            }
        }
        Err(ConfigError::NoConfigLibrary(*uuid))
    }

    pub fn uuid_exists(&self, uuid: &Uuid) -> bool {
        for library in &self.libraries {
            if &library.uuid == uuid {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ConfigConnections {
    pub listenbrainz_token: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub path: PathBuf,
    pub backup_folder: Option<PathBuf>,
    pub libraries: ConfigLibraries,
    pub connections: ConfigConnections,
    pub state_path: PathBuf,
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
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&writer)?;
        let config = to_string_pretty(self)?;
        // dbg!(&config);

        file.write_all(config.as_bytes())?;
        fs::rename(writer, self.path.as_path())?;
        Ok(())
    }

    pub fn save_backup(&self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.backup_folder {
            Some(path) => {
                let mut writer = path.clone();
                writer.set_extension("tmp");
                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .read(true)
                    .write(true)
                    .open(&writer)?;
                let config = to_string_pretty(self)?;
                // dbg!(&config);

                file.write_all(config.as_bytes())?;
                fs::rename(writer, self.path.as_path())?;
                Ok(())
            }
            None => Err(ConfigError::NoBackupLibrary.into()),
        }
    }

    pub fn read_file(path: PathBuf) -> Result<Self, Error> {
        let mut file: File = File::open(path)?;
        let mut buf: String = String::new();
        _ = file.read_to_string(&mut buf);
        let config: Config = serde_json::from_str::<Config>(&buf)?;
        Ok(config)
    }

    pub fn push_library(&mut self, lib: ConfigLibrary) {
        if self.libraries.libraries.is_empty() {
            self.libraries.default_library = lib.uuid;
        }
        self.libraries.libraries.push(lib);
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
    #[error("No backup Config folder present")]
    NoBackupLibrary,
}

#[cfg(test)]
pub mod tests {
    use super::{Config, ConfigLibrary};
    use crate::music_storage::library::MusicLibrary;
    use std::path::PathBuf;

    pub fn new_config_lib() -> (Config, MusicLibrary) {
        _ = std::fs::create_dir_all("test-config/music/");
        let lib = ConfigLibrary::new(
            PathBuf::from("test-config/library"),
            String::from("library"),
            None,
        );
        let mut config = Config {
            path: PathBuf::from("test-config/config_test.json"),
            ..Default::default()
        };

        config.push_library(lib);
        config.write_file().unwrap();

        let mut lib = MusicLibrary::init(
            config.libraries.get_default().unwrap().path.clone(),
            dbg!(config.libraries.default_library),
        )
        .unwrap();
        lib.scan_folder("test-config/music/").unwrap();
        lib.save(config.libraries.get_default().unwrap().path.clone())
            .unwrap();

        (config, lib)
    }

    pub fn read_config_lib() -> (Config, MusicLibrary) {
        let config = Config::read_file(PathBuf::from("test-config/config_test.json")).unwrap();

        // dbg!(&config);

        let mut lib = MusicLibrary::init(
            config.libraries.get_default().unwrap().path.clone(),
            config.libraries.get_default().unwrap().uuid,
        )
        .unwrap();

        lib.scan_folder("test-config/music/").unwrap();

        lib.save(config.libraries.get_default().unwrap().path.clone())
            .unwrap();

        (config, lib)
    }

    #[test]
    fn test3() {
        let (config, _) = read_config_lib();

        _ = config.write_file();

        dbg!(config);
    }
}
