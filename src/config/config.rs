use std::{path::PathBuf, marker::PhantomData};

#[derive(Debug, Default)]
pub struct Config {
    db_path: Option<PathBuf>,
}

impl Config {
    pub fn new_main() -> Self {
        Config::default()
    }
    //TODO: Add new function for test tube
    pub fn load(&self) {

    }
}