use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::music_controller::config::Config;
use crate::music_storage::music_db::{MusicLibrary, Song, Tag};

pub struct MusicController {
    pub config: Arc<RwLock<Config>>,
    pub library: MusicLibrary,
}

impl MusicController {
    /// Creates new MusicController with config at given path
    pub fn new(config_path: &PathBuf) -> Result<MusicController, Box<dyn std::error::Error>> {
        let config = Arc::new(RwLock::new(Config::new(config_path)?));
        let library = match MusicLibrary::init(config.clone()) {
            Ok(library) => library,
            Err(error) => return Err(error),
        };

        let controller = MusicController {
            config,
            library,
        };

        return Ok(controller);
    }

    /// Creates new music controller from a config at given path
    pub fn from(config_path: &PathBuf) -> Result<MusicController, Box<dyn std::error::Error>> {
        let config = Arc::new(RwLock::new(Config::from(config_path)?));
        let library = match MusicLibrary::init(config.clone()) {
            Ok(library) => library,
            Err(error) => return Err(error),
        };

        let controller = MusicController {
            config,
            library,
        };

        return Ok(controller);
    }

    /// Queries the [MusicLibrary], returning a `Vec<Song>`
    pub fn query_library(
        &self,
        query_string: &String,
        target_tags: Vec<Tag>,
        _search_location: bool,
        sort_by: Vec<Tag>,
    ) -> Option<Vec<&Song>> {
        self.library
            .query_tracks(query_string, &target_tags, &sort_by)
    }
}
