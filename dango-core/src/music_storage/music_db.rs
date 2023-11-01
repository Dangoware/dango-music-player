use file_format::{FileFormat, Kind};
use lofty::{AudioFile, ItemKey, ItemValue, Probe, TagType, TaggedFileExt};
use std::ffi::OsStr;
use std::{error::Error, io::BufReader};

use chrono::{serde::ts_seconds_option, DateTime, Utc};
use std::time::Duration;
//use walkdir::WalkDir;
use cue::cd::CD;
use jwalk::WalkDir;

use bincode::{deserialize_from, serialize_into};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use unidecode::unidecode;

// Fun parallel stuff
use rayon::prelude::*;
use std::sync::{Arc, Mutex, RwLock};

use crate::music_controller::config::Config;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlbumArt {
    pub index: u16,
    pub path: Option<URI>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Tag {
    Title,
    Album,
    Artist,
    Genre,
    Comment,
    Track,
    Disk,
    Key(String),
    Field(String),
}

impl ToString for Tag {
    fn to_string(&self) -> String {
        match self {
            Self::Title => "TrackTitle".into(),
            Self::Album => "AlbumTitle".into(),
            Self::Artist => "TrackArtist".into(),
            Self::Genre => "Genre".into(),
            Self::Comment => "Comment".into(),
            Self::Track => "TrackNumber".into(),
            Self::Disk => "DiscNumber".into(),
            Self::Key(key) => key.into(),
            Self::Field(f) => f.into(),
        }
    }
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
    #[serde(with = "ts_seconds_option")]
    pub date_modified: Option<DateTime<Utc>>,
    pub album_art: Vec<AlbumArt>,
    pub tags: Vec<(Tag, String)>,
}

impl Song {
    /**
     * Get a tag's value
     *
     * ```
     * // Assuming an already created song:
     *
     * let tag = this_song.get_tag(Tag::Title);
     *
     * assert_eq!(tag, "Some Song Title");
     * ```
     **/
    pub fn get_tag(&self, target_key: &Tag) -> Option<&String> {
        let index = self.tags.iter().position(|r| r.0 == *target_key);

        match index {
            Some(i) => return Some(&self.tags[i].1),
            None => None,
        }
    }

    pub fn get_field(&self, target_field: &str) -> Option<String> {
        match target_field {
            "location" => Some(self.location.clone().path_string()),
            "plays" => Some(self.plays.clone().to_string()),
            _ => None, // Other field types are not yet supported
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum URI {
    Local(PathBuf),
    Cue {
        location: PathBuf,
        start: Duration,
        end: Duration,
    },
    Remote(Service, PathBuf),
}

impl URI {
    /// Returns the start time of a CUEsheet song, or an
    /// error if the URI is not a Cue variant
    pub fn start(&self) -> Result<&Duration, Box<dyn Error>> {
        match self {
            URI::Local(_) => Err("\"Local\" has no starting time".into()),
            URI::Remote(_, _) => Err("\"Remote\" has no starting time".into()),
            URI::Cue {
                location: _,
                start,
                end: _,
            } => Ok(start),
        }
    }

    /// Returns the end time of a CUEsheet song, or an
    /// error if the URI is not a Cue variant
    pub fn end(&self) -> Result<&Duration, Box<dyn Error>> {
        match self {
            URI::Local(_) => Err("\"Local\" has no starting time".into()),
            URI::Remote(_, _) => Err("\"Remote\" has no starting time".into()),
            URI::Cue {
                location: _,
                start: _,
                end,
            } => Ok(end),
        }
    }

    /// Returns the location as a PathBuf
    pub fn path(&self) -> &PathBuf {
        match self {
            URI::Local(location) => location,
            URI::Cue {
                location,
                start: _,
                end: _,
            } => location,
            URI::Remote(_, location) => location,
        }
    }

    pub fn path_string(&self) -> String {
        let path_str = match self {
            URI::Local(location) => location.as_path().to_string_lossy(),
            URI::Cue {
                location,
                start: _,
                end: _,
            } => location.as_path().to_string_lossy(),
            URI::Remote(_, location) => location.as_path().to_string_lossy(),
        };
        path_str.to_string()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Service {
    InternetRadio,
    Spotify,
    Youtube,
}

/* TODO: Rework this entirely
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
*/

#[derive(Debug)]
pub struct MusicLibrary {
    pub library: Vec<Song>,
}

pub fn normalize(input_string: &String) -> String {
    unidecode(input_string)
        .to_ascii_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "")
}

impl MusicLibrary {
    /// Initialize the database
    ///
    /// If the database file already exists, return the [MusicLibrary], otherwise create
    /// the database first. This needs to be run before anything else to retrieve
    /// the [MusicLibrary] Vec
    pub fn init(config: Arc<RwLock<Config>>) -> Result<Self, Box<dyn Error>> {
        let global_config = &*config.read().unwrap();
        let mut library: Vec<Song> = Vec::new();
        let mut backup_path = global_config.db_path.clone();
        backup_path.set_extension("bkp");

        match global_config.db_path.try_exists() {
            Ok(true) => {
                // The database exists, so get it from the file
                let database = fs::File::open(global_config.db_path.to_path_buf())?;
                let reader = BufReader::new(database);
                library = deserialize_from(reader)?;
            }
            Ok(false) => {
                // Create the database if it does not exist
                // possibly from the backup file
                if backup_path.try_exists().is_ok_and(|x| x == true) {
                    let database = fs::File::open(global_config.db_path.to_path_buf())?;
                    let reader = BufReader::new(database);
                    library = deserialize_from(reader)?;
                } else {
                    let mut writer =
                        BufWriter::new(fs::File::create(global_config.db_path.to_path_buf())?);
                    serialize_into(&mut writer, &library)?;
                }
            }
            Err(error) => return Err(error.into()),
        };

        Ok(Self { library })
    }

    /// Serializes the database out to the file
    /// specified in the config
    pub fn save(&self, config: &Config) -> Result<(), Box<dyn Error>> {
        match config.db_path.try_exists() {
            Ok(true) => {
                // The database exists, so rename it to `.bkp` and
                // write the new database file
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
            }
            Err(error) => return Err(error.into()),
        }

        Ok(())
    }

    pub fn size(&self) -> usize {
        self.library.len()
    }

    /// Queries for a [Song] by its [URI], returning a single `Song`
    /// with the `URI` that matches
    fn query_uri(&self, path: &URI) -> Option<(&Song, usize)> {
        let result = Arc::new(Mutex::new(None));
        let index = Arc::new(Mutex::new(0));
        let _ = &self.library.par_iter().enumerate().for_each(|(i, track)| {
            if path == &track.location {
                *result.clone().lock().unwrap() = Some(track);
                *index.clone().lock().unwrap() = i;
                return;
            }
        });
        let song = Arc::try_unwrap(result).unwrap().into_inner().unwrap();
        match song {
            Some(song) => Some((song, Arc::try_unwrap(index).unwrap().into_inner().unwrap())),
            None => None,
        }
    }

    /// Queries for a [Song] by its [PathBuf], returning a `Vec<Song>`
    /// with matching `PathBuf`s
    fn query_path(&self, path: &PathBuf) -> Option<Vec<&Song>> {
        let result: Arc<Mutex<Vec<&Song>>> = Arc::new(Mutex::new(Vec::new()));
        let _ = &self.library.par_iter().for_each(|track| {
            if path == track.location.path() {
                result.clone().lock().unwrap().push(&track);
                return;
            }
        });
        if result.lock().unwrap().len() > 0 {
            Some(Arc::try_unwrap(result).unwrap().into_inner().unwrap())
        } else {
            None
        }
    }

    /// Finds all the music files within a specified folder
    pub fn find_all_music(
        &mut self,
        target_path: &str,
        config: &Config,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let mut total = 0;
        let mut i = 0;
        for entry in WalkDir::new(target_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let target_file = entry;
            let path = target_file.path();

            // Ensure the target is a file and not a directory,
            // if it isn't a file, skip this loop
            if !path.is_file() {
                continue;
            }

            // Check if the file path is already in the db
            if self.query_uri(&URI::Local(path.to_path_buf())).is_some() {
                continue;
            }

            // Save periodically while scanning
            i += 1;
            if i % 250 == 0 {
                self.save(config).unwrap();
            }

            let format = FileFormat::from_file(&path)?;
            let extension: &OsStr = match path.extension() {
                Some(ext) => ext,
                None => OsStr::new(""),
            };

            // If it's a normal file, add it to the database
            // if it's a cuesheet, do a bunch of fancy stuff
            if (format.kind() == Kind::Audio || format.kind() == Kind::Video)
                && extension.to_ascii_lowercase() != "log"
                && extension.to_ascii_lowercase() != "vob"
            {
                match self.add_file(&target_file.path()) {
                    Ok(_) => {
                        //println!("{:?}", target_file.path());
                        total += 1
                    }
                    Err(_error) => {
                        //println!("{}, {:?}: {}", format, target_file.file_name(), _error)
                    } // TODO: Handle more of these errors
                };
            } else if extension.to_ascii_lowercase() == "cue" {
                total += match self.add_cuesheet(&target_file.path()) {
                    Ok(added) => added,
                    Err(error) => {
                        println!("{}", error);
                        0
                    }
                }
            }
        }

        // Save the database after scanning finishes
        self.save(&config).unwrap();

        Ok(total)
    }

    pub fn add_file(&mut self, target_file: &Path) -> Result<(), Box<dyn Error>> {
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

        let mut tags: Vec<(Tag, String)> = Vec::new();
        for item in tag.items() {
            let key = match item.key() {
                ItemKey::TrackTitle => Tag::Title,
                ItemKey::TrackNumber => Tag::Track,
                ItemKey::TrackArtist => Tag::Artist,
                ItemKey::Genre => Tag::Genre,
                ItemKey::Comment => Tag::Comment,
                ItemKey::AlbumTitle => Tag::Album,
                ItemKey::Unknown(unknown) => Tag::Key(unknown.to_string()),
                custom => Tag::Key(format!("{:?}", custom)),
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

        let new_song = Song {
            location: URI::Local(binding),
            plays: 0,
            skips: 0,
            favorited: false,
            rating: None,
            format,
            duration,
            play_time: Duration::from_secs(0),
            last_played: None,
            date_added: Some(chrono::offset::Utc::now()),
            date_modified: Some(chrono::offset::Utc::now()),
            tags,
            album_art,
        };

        match self.add_song(new_song) {
            Ok(_) => (),
            Err(error) => return Err(error),
        };

        Ok(())
    }

    pub fn add_cuesheet(&mut self, cuesheet: &PathBuf) -> Result<usize, Box<dyn Error>> {
        let mut tracks_added = 0;

        let cue_data = CD::parse_file(cuesheet.to_owned()).unwrap();

        // Get album level information
        let album_title = &cue_data
            .get_cdtext()
            .read(cue::cd_text::PTI::Title)
            .unwrap_or(String::new());
        let album_artist = &cue_data
            .get_cdtext()
            .read(cue::cd_text::PTI::Performer)
            .unwrap_or(String::new());

        let parent_dir = cuesheet.parent().expect("The file has no parent path??");
        for (i, track) in cue_data.tracks().iter().enumerate() {
            let audio_location = parent_dir.join(track.get_filename());

            if !audio_location.exists() {
                continue;
            }

            // Try to remove the original audio file from the db if it exists
            let _ = self.remove_uri(&URI::Local(audio_location.clone()));

            // Get the track timing information
            let pregap = match track.get_zero_pre() {
                Some(pregap) => Duration::from_micros((pregap as f32 * 13333.333333) as u64),
                None => Duration::from_secs(0),
            };
            let postgap = match track.get_zero_post() {
                Some(postgap) => Duration::from_micros((postgap as f32 * 13333.333333) as u64),
                None => Duration::from_secs(0),
            };
            let mut start = Duration::from_micros((track.get_start() as f32 * 13333.333333) as u64);
            start -= pregap;

            let duration = match track.get_length() {
                Some(len) => Duration::from_micros((len as f32 * 13333.333333) as u64),
                None => {
                    let tagged_file = match lofty::read_from_path(&audio_location) {
                        Ok(tagged_file) => tagged_file,

                        Err(_) => match Probe::open(&audio_location)?.read() {
                            Ok(tagged_file) => tagged_file,

                            Err(error) => return Err(error.into()),
                        },
                    };

                    tagged_file.properties().duration() - start
                }
            };
            let end = start + duration + postgap;

            // Get the format as a string
            let format: Option<FileFormat> = match FileFormat::from_file(&audio_location) {
                Ok(fmt) => Some(fmt),
                Err(_) => None,
            };

            // Get some useful tags
            let mut tags: Vec<(Tag, String)> = Vec::new();
            tags.push((Tag::Album, album_title.clone()));
            tags.push((Tag::Key("AlbumArtist".to_string()), album_artist.clone()));
            match track.get_cdtext().read(cue::cd_text::PTI::Title) {
                Some(title) => tags.push((Tag::Title, title)),
                None => match track.get_cdtext().read(cue::cd_text::PTI::UPC_ISRC) {
                    Some(title) => tags.push((Tag::Title, title)),
                    None => {
                        let namestr = format!("{} - {}", i, track.get_filename());
                        tags.push((Tag::Title, namestr))
                    }
                },
            };
            match track.get_cdtext().read(cue::cd_text::PTI::Performer) {
                Some(artist) => tags.push((Tag::Artist, artist)),
                None => (),
            };
            match track.get_cdtext().read(cue::cd_text::PTI::Genre) {
                Some(genre) => tags.push((Tag::Genre, genre)),
                None => (),
            };
            match track.get_cdtext().read(cue::cd_text::PTI::Message) {
                Some(comment) => tags.push((Tag::Comment, comment)),
                None => (),
            };

            let album_art = Vec::new();

            let new_song = Song {
                location: URI::Cue {
                    location: audio_location,
                    start,
                    end,
                },
                plays: 0,
                skips: 0,
                favorited: false,
                rating: None,
                format,
                duration,
                play_time: Duration::from_secs(0),
                last_played: None,
                date_added: Some(chrono::offset::Utc::now()),
                date_modified: Some(chrono::offset::Utc::now()),
                tags,
                album_art,
            };

            match self.add_song(new_song) {
                Ok(_) => tracks_added += 1,
                Err(_error) => {
                    //println!("{}", _error);
                    continue;
                }
            };
        }

        Ok(tracks_added)
    }

    pub fn add_song(&mut self, new_song: Song) -> Result<(), Box<dyn Error>> {
        match self.query_uri(&new_song.location) {
            Some(_) => {
                return Err(format!("URI already in database: {:?}", new_song.location).into())
            }
            None => (),
        }
        match new_song.location {
            URI::Local(_) if self.query_path(&new_song.location.path()).is_some() => {
                return Err(format!("Location exists for {:?}", new_song.location).into())
            }
            _ => (),
        }

        self.library.push(new_song);

        Ok(())
    }

    /// Removes a song indexed by URI, returning the position removed
    pub fn remove_uri(&mut self, target_uri: &URI) -> Result<usize, Box<dyn Error>> {
        let location = match self.query_uri(target_uri) {
            Some(value) => value.1,
            None => return Err("URI not in database".into()),
        };

        self.library.remove(location);

        Ok(location)
    }

    /// Scan the song by a location and update its tags
    pub fn update_by_file(&mut self, new_tags: Song) -> Result<(), Box<dyn std::error::Error>> {
        match self.query_uri(&new_tags.location) {
            Some(_) => (),
            None => return Err(format!("URI not in database!").into()),
        }

        todo!()
    }

    /// Query the database, returning a list of [Song]s
    ///
    /// The order in which the `sort_by` `Vec` is arranged
    /// determines the output sorting
    pub fn query(
        &self,
        query_string: &String,  // The query itself
        target_tags: &Vec<Tag>, // The tags to search
        search_location: bool,  // Whether to search the location field or not
        sort_by: &Vec<Tag>,     // Tags to sort the resulting data by
    ) -> Option<Vec<&Song>> {
        let songs = Arc::new(Mutex::new(Vec::new()));

        self.library.par_iter().for_each(|track| {
            for tag in target_tags {
                let track_result = match track.get_tag(&tag) {
                    Some(value) => value,
                    None => continue,
                };

                if normalize(track_result).contains(&normalize(&query_string)) {
                    songs.lock().unwrap().push(track);
                    return;
                }
            }

            if !search_location {
                return;
            }

            // Find a URL in the song
            if normalize(&track.location.path_string()).contains(&normalize(&query_string)) {
                songs.lock().unwrap().push(track);
                return;
            }
        });

        let lock = Arc::try_unwrap(songs).expect("Lock still has multiple owners!");
        let mut new_songs = lock.into_inner().expect("Mutex cannot be locked!");

        // Sort the returned list of songs
        new_songs.par_sort_by(|a, b| {
            for opt in sort_by {
                let tag_a = match opt {
                    Tag::Field(field_selection) => match a.get_field(field_selection) {
                        Some(field_value) => field_value,
                        None => continue,
                    },
                    _ => match a.get_tag(&opt) {
                        Some(tag_value) => tag_value.to_owned(),
                        None => continue,
                    },
                };

                let tag_b = match opt {
                    Tag::Field(field_selection) => match b.get_field(field_selection) {
                        Some(field_value) => field_value,
                        None => continue,
                    },
                    _ => match b.get_tag(&opt) {
                        Some(tag_value) => tag_value.to_owned(),
                        None => continue,
                    },
                };

                // Try to parse the tags as f64
                if let (Ok(num_a), Ok(num_b)) = (tag_a.parse::<i32>(), tag_b.parse::<i32>()) {
                    // If parsing succeeds, compare as numbers
                    if num_a < num_b {
                        return std::cmp::Ordering::Less;
                    } else if num_a > num_b {
                        return std::cmp::Ordering::Greater;
                    }
                } else {
                    // If parsing fails, compare as strings
                    if tag_a < tag_b {
                        return std::cmp::Ordering::Less;
                    } else if tag_a > tag_b {
                        return std::cmp::Ordering::Greater;
                    }
                }
            }

            // If all tags are equal, sort by Track number
            a.get_tag(&Tag::Track).cmp(&b.get_tag(&Tag::Track))
        });

        if new_songs.len() > 0 {
            Some(new_songs)
        } else {
            None
        }
    }
}
