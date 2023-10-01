use file_format::{FileFormat, Kind};
use lofty::{AudioFile, ItemKey, ItemValue, Probe, TagType, TaggedFileExt};
use std::{error::Error, io::BufReader};

use chrono::{serde::ts_seconds_option, DateTime, Utc};
use std::time::Duration;
use walkdir::WalkDir;

use bincode::{deserialize_from, serialize_into};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use crate::music_controller::config::Config;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlbumArt {
    pub index: u16,
    pub path: Option<URI>,
}

/// Stores information about a single song
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Song {
    pub location: URI,
    pub plays: i32,
    pub skips: i32,
    pub favorited: bool,
    pub rating: Option<u8>,
    pub format: Option<FileFormat>,
    pub duration: Duration,
    pub play_time: Duration,
    #[serde(with = "ts_seconds_option")]
    pub last_played: Option<DateTime<Utc>>,
    #[serde(with = "ts_seconds_option")]
    pub date_added: Option<DateTime<Utc>>,
    pub album_art: Vec<AlbumArt>,
    pub tags: Vec<(String, String)>,
}

impl Song {
    pub fn get_tag(&self, target_key: &str) -> Option<&String> {
        let index = self.tags.iter().position(|r| r.0 == target_key);

        match index {
            Some(i) => return Some(&self.tags[i].1),
            None => None,
        }
    }

    pub fn get_tags(&self, target_keys: &Vec<String>) -> Vec<Option<String>> {
        let mut results = Vec::new();
        for tag in &self.tags {
            for key in target_keys {
                if &tag.0 == key {
                    results.push(Some(tag.1.to_owned()))
                }
            }
            results.push(None);
        }
        results
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum URI {
    Local(String),
    //Cue(String, Duration), TODO: Make cue stuff work
    Remote(Service, String),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Service {
    InternetRadio,
    Spotify,
    Youtube,
}

#[derive(Debug)]
pub struct Playlist {
    title: String,
    cover_art: Box<Path>,
}

#[derive(Debug)]
pub enum MusicObject {
    Song(Song),
    Album(Playlist),
    Playlist(Playlist),
}

#[derive(Debug)]
pub struct MusicLibrary {
    pub library: Vec<Song>,
}

impl MusicLibrary {
    /// Initialize the database
    ///
    /// If the database file already exists, return the Library, otherwise create
    /// the database first. This needs to be run before anything else to retrieve
    /// the library vec
    pub fn init(config: &Config) -> Result<Self, Box<dyn Error>> {
        let mut library: Vec<Song> = Vec::new();
        let mut backup_path = config.db_path.clone();
        backup_path.set_extension("bkp");

        match config.db_path.try_exists() {
            Ok(true) => {
                // The database exists, so get it from the file
                let database = fs::File::open(config.db_path.to_path_buf())?;
                let reader = BufReader::new(database);
                library = deserialize_from(reader)?;
            }
            Ok(false) => {
                // Create the database if it does not exist
                // possibly from the backup file
                if backup_path.try_exists().is_ok_and(|x| x == true) {
                    let database = fs::File::open(config.db_path.to_path_buf())?;
                    let reader = BufReader::new(database);
                    library = deserialize_from(reader)?;
                } else {
                    let mut writer = BufWriter::new(fs::File::create(config.db_path.to_path_buf())?);
                    serialize_into(&mut writer, &library)?;
                }
            },
            Err(error) => return Err(error.into())
        };

        Ok(Self { library })
    }

    pub fn save(&self, config: &Config) -> Result<(), Box<dyn Error>> {
        match config.db_path.try_exists() {
            Ok(true) => {
                // The database exists, rename it to `.bkp` and
                // write the new database
                let mut backup_name = config.db_path.clone();
                backup_name.set_extension("bkp");
                fs::rename(config.db_path.as_path(), backup_name.as_path())?;

                // TODO: Make this save properly like in config.rs

                let mut writer = BufWriter::new(fs::File::create(config.db_path.to_path_buf())?);
                serialize_into(&mut writer, &self.library)?;
            }
            Ok(false) => {
                // Create the database if it does not exist
                let mut writer = BufWriter::new(fs::File::create(config.db_path.to_path_buf())?);
                serialize_into(&mut writer, &self.library)?;
            },
            Err(error) => return Err(error.into())
        }

        Ok(())
    }

    fn find_by_uri(&self, path: &URI) -> Option<Song> {
        for track in &self.library {
            if path == &track.location {
                return Some(track.clone());
            }
        }
        None
    }

    pub fn find_all_music(&mut self, target_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut current_dir = PathBuf::new();
        for entry in WalkDir::new(target_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let target_file = entry;
            let is_file = fs::metadata(target_file.path())?.is_file();

            // Ensure the target is a file and not a directory, if it isn't, skip this loop
            if !is_file {
                current_dir = target_file.into_path();
                continue;
            }

            let format = FileFormat::from_file(target_file.path())?;
            let extension = target_file
                .path()
                .extension()
                .expect("Could not find file extension");

            // If it's a normal file, add it to the database
            // if it's a cuesheet, do a bunch of fancy stuff
            if format.kind() == Kind::Audio {
                match self.add_file_to_db(target_file.path()) {
                    Ok(_) => (),
                    Err(_error) => () //println!("{}, {:?}: {}", format, target_file.file_name(), error)
                };
            } else if extension.to_ascii_lowercase() == "cue" {
                // TODO: implement cuesheet support
            }
        }

        Ok(())
    }

    pub fn add_file_to_db(&mut self, target_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Fix error handling here
        let tagged_file = match lofty::read_from_path(target_file) {
            Ok(tagged_file) => tagged_file,

            Err(_) => match Probe::open(target_file)?.read() {
                Ok(tagged_file) => tagged_file,

                Err(error) => return Err(error.into()),
            },
        };

        // Ensure the tags exist, if not, insert blank data
        let blank_tag = &lofty::Tag::new(TagType::Id3v2);
        let tag = match tagged_file.primary_tag() {
            Some(primary_tag) => primary_tag,

            None => match tagged_file.first_tag() {
                Some(first_tag) => first_tag,
                None => blank_tag,
            },
        };

        let mut tags: Vec<(String, String)> = Vec::new();
        for item in tag.items() {
            let mut key = String::new();
            match item.key() {
                ItemKey::Unknown(unknown) => key.push_str(&unknown),
                custom => key = format!("{:?}", custom),
            };

            let value = match item.value() {
                ItemValue::Text(value) => String::from(value),
                ItemValue::Locator(value) => String::from(value),
                ItemValue::Binary(_) => String::from(""),
            };

            tags.push((key, value))
        }

        // Get all the album artwork information
        let mut album_art: Vec<AlbumArt> = Vec::new();
        for (i, _art) in tag.pictures().iter().enumerate() {
            let new_art = AlbumArt {
                index: i as u16,
                path: None,
            };

            album_art.push(new_art)
        }

        // Get the format as a string
        let format: Option<FileFormat> = match FileFormat::from_file(target_file) {
            Ok(fmt) => Some(fmt),
            Err(_) => None,
        };

        let duration = tagged_file.properties().duration();

        // TODO: Fix error handling
        let binding = fs::canonicalize(target_file).unwrap();
        let abs_path = binding.to_str().unwrap();

        let new_song = Song {
            location: URI::Local(abs_path.to_string()),
            plays: 0,
            skips: 0,
            favorited: false,
            rating: None,
            format,
            duration,
            play_time: Duration::from_secs(0),
            last_played: None,
            date_added: Some(chrono::offset::Utc::now()),
            tags,
            album_art,
        };

        match self.add_song_to_db(new_song) {
            Ok(_) => (),
            Err(error) => ()
        };

        Ok(())
    }

    pub fn add_song_to_db(&mut self, new_song: Song) -> Result<(), Box<dyn std::error::Error>> {
        match self.find_by_uri(&new_song.location) {
            Some(_) => return Err(format!("URI already in database: {:?}", new_song.location).into()),
            None => ()
        }

        self.library.push(new_song);

        Ok(())
    }

    pub fn update_song_tags(&mut self, new_tags: Song) -> Result<(), Box<dyn std::error::Error>> {
        match self.find_by_uri(&new_tags.location) {
            Some(_) => (),
            None => return Err(format!("URI not in database!").into())
        }

        todo!()
    }

    /// Query the database, returning a list of items
    pub fn query(
        &self,
        query_string: &String,     // The query itself
        target_tags: &Vec<String>, // The tags to search
        sort_by: &Vec<String>,     // Tags to sort the resulting data by
    ) -> Option<Vec<MusicObject>> {
        unimplemented!()
    }
}
