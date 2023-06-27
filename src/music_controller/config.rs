use std::path::{Path, PathBuf};
       
pub struct Config {
    db_path: Box<PathBuf>,
}

impl Config {
    fn new() -> Config {
        let path = "./music_database.db3";
        
        Config {
            db_path: Box::new(PathBuf::new()),
        }   
    }
}
