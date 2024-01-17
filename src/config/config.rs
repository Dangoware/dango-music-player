use std::{path::{PathBuf, Path}, marker::PhantomData, fs::{File, OpenOptions, self}, io::{Error, Write, Read}, default};

use serde::{Serialize, Deserialize};
use serde_json::{to_string, to_string_pretty};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigLibrary {
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
    volume: f32,
}

impl Config {
    pub fn new_main() -> Self {
        Config::default()
    }
    //TODO: Add new function for test tube
    pub fn set_default_library(&self, uuid: Uuid) {
        self.default_library = uuid;
    }
    //TODO: make this a ConfigError type
    pub fn default_library(&self) -> Result<&ConfigLibrary, String> {
        for library in &self.libraries {
            if library.uuid == self.default_library {
                return Ok(library)
            }
            else {
                continue;
            }
        }
        Err("No default library!".to_string())
    }
    pub fn to_file(&self) -> Result<(), Error> {
        let mut writer = self.path.clone();
        writer.set_extension("tmp");
        let mut file = OpenOptions::new().create(true).truncate(true).read(true).write(true).open(writer)?;
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
