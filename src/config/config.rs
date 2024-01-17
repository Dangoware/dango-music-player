use std::{path::PathBuf, marker::PhantomData};

use crate::music_storage::library::MusicLibrary;

#[derive(Debug, Default)]
struct ConfigLibrary {
    name: String,
    path: PathBuf,
}
#[derive(Debug, Default)]
pub struct Config {
    libraries: Vec<ConfigLibrary>
}

impl Config {
    pub fn new_main() -> Self {
        Config::default()
    }
    //TODO: Add new function for test tube
    pub fn load(&self) {

    }
}