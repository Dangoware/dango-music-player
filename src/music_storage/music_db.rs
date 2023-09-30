use file_format::{FileFormat, Kind};
use lofty::{AudioFile, Probe, TaggedFileExt, ItemKey, ItemValue, TagType};
use std::{error::Error, io::BufReader};

use std::time::Duration;
use chrono::{DateTime, Utc, serde::ts_seconds_option};
use walkdir::WalkDir;

use std::io::BufWriter;
use std::fs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use bincode::{serialize_into, deserialize_from};

use crate::music_controller::config::Config;

pub struct AlbumArt {
    pub path: Option<URI>;
}

/// Stores information about a single song
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Song {
    pub path: URI,
    pub plays: i32,
    pub skips: i32,
    pub favorited: bool,
    pub rating: u8,
    pub format: Option<FileFormat>,
    pub duration: Duration,
    pub play_time: Duration,
    #[serde(with = "ts_seconds_option")]
    pub last_played: Option<DateTime<Utc>>,
    #[serde(with = "ts_seconds_option")]
    pub date_added: Option<DateTime<Utc>>,
    pub tags: Vec<(String, String)>,
}

impl Song {
    pub fn get_tag(&self, target_key: String) -> Option<String> {
        for tag in self.tags {
            if tag.0 == target_key {
                return Some(tag.1)
            }
        }
        None
    }

    pub fn get_tags(&self, target_keys: Vec<String>) -> Vec<Option<String>> {
        let mut results = Vec::new();
        for tag in self.tags {
            for key in target_keys {
                if tag.0 == key {
                    results.push(Some(tag.1))
                }
            }
            results.push(None);
        }
        results
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum URI{
    Local(String),
    Remote(Service, String),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

/// Initialize the database
///
/// If the database file already exists, return the database, otherwise create it first
/// This needs to be run before anything else to retrieve the library vec
pub fn init_db(config: &Config) -> Result<Vec<Song>, Box<dyn Error>> {
    let mut library: Vec<Song> = Vec::new();

    match config.db_path.try_exists() {
        Ok(_) => {
            // The database exists, so get it from the file
            let database = fs::File::open(config.db_path.into_boxed_path())?;
            let reader = BufReader::new(database);
            library = deserialize_from(reader)?;
        },
        Err(_) => {
            // Create the database if it does not exist
            let mut writer = BufWriter::new(
                fs::File::create(config.db_path.into_boxed_path())?
            );
            serialize_into(&mut writer, &library)?;
        }
    };

    Ok(library)
}

fn path_in_db(query_path: &Path, library: &Vec<Song>) -> bool {
    unimplemented!()
}

pub fn find_all_music(
    config: &Config,
    target_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {

    let mut current_dir = PathBuf::new();
    for entry in WalkDir::new(target_path).follow_links(true).into_iter().filter_map(|e| e.ok()) {
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
            add_file_to_db(target_file.path())
        } else if extension.to_ascii_lowercase() == "cue" {
            // TODO: implement cuesheet support
        }
    }

    Ok(())
}

pub fn add_file_to_db(target_file: &Path) {
    // TODO: Fix error handling here
    let tagged_file = match lofty::read_from_path(target_file) {
        Ok(tagged_file) => tagged_file,

        Err(_) => match Probe::open(target_file)
            .expect("ERROR: Bad path provided!")
            .read() {
                Ok(tagged_file) => tagged_file,

                Err(_) => return
            }
    };

    // Ensure the tags exist, if not, insert blank data
    let blank_tag = &lofty::Tag::new(TagType::Id3v2);
    let tag = match tagged_file.primary_tag() {
        Some(primary_tag) => primary_tag,

        None => match tagged_file.first_tag() {
            Some(first_tag) => first_tag,
            None => blank_tag
        },
    };

    let mut custom_insert = String::new();
    let mut loops = 0;
    for (loops, item) in tag.items().enumerate() {
        let mut custom_key = String::new();
        match item.key() {
            ItemKey::TrackArtist |
            ItemKey::TrackTitle  |
            ItemKey::AlbumTitle  |
            ItemKey::Genre       |
            ItemKey::TrackNumber |
            ItemKey::Year        |
            ItemKey::RecordingDate  => continue,
            ItemKey::Unknown(unknown) => custom_key.push_str(&unknown),
            custom => custom_key.push_str(&format!("{:?}", custom))
            // TODO: This is kind of cursed, maybe fix?
        };

        let custom_value = match item.value() {
            ItemValue::Text(value) => value,
            ItemValue::Locator(value) => value,
            ItemValue::Binary(_) => ""
        };

        if loops > 0 {
            custom_insert.push_str(", ");
        }
    }
    
    // Get the format as a string
    let short_format: Option<String> = match FileFormat::from_file(target_file) {
        Ok(fmt) => Some(fmt.to_string()),
        Err(_) => None
    };

    println!("{}", short_format.as_ref().unwrap());
    
    let duration = tagged_file.properties().duration().as_secs().to_string();
    
    // TODO: Fix error handling
    let binding = fs::canonicalize(target_file).unwrap();
    let abs_path = binding.to_str().unwrap();
}

pub fn add_song_to_db(new_song: Song) {

}

#[derive(Debug)]
pub enum MusicObject {
    Song(Song),
    Album(Playlist),
    Playlist(Playlist),
}

impl MusicObject {
    pub fn as_song(&self) -> Option<&Song> {
        match self {
            MusicObject::Song(data) => Some(data),
            _ => None
        }
    }
}

/// Query the database, returning a list of items
pub fn query (
    query_string: &String,      // The query itself
    target_tags: &Vec<String>,  // The tags to search
    sort_by: &Vec<String>,      // Tags to sort the resulting data by
) -> Option<Vec<MusicObject>> {
    unimplemented!()
}
