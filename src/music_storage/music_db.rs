// Crate things
use super::utils::{normalize, read_library, write_library, find_images};
use crate::music_controller::config::Config;

// Various std things
use std::collections::BTreeMap;
use std::error::Error;
use std::ops::ControlFlow::{Break, Continue};

// Files
use cue::cd::CD;
use file_format::{FileFormat, Kind};
use walkdir::WalkDir;
use lofty::{AudioFile, ItemKey, ItemValue, Probe, TagType, TaggedFileExt};
use std::fs;
use std::path::{Path, PathBuf};

// Time
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use std::time::Duration;

// Serialization/Compression
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

// Fun parallel stuff
use rayon::prelude::*;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum AlbumArt {
    Embedded(usize),
    External(URI),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tag {
    Title,
    Album,
    Artist,
    AlbumArtist,
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
            Self::AlbumArtist => "AlbumArtist".into(),
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
    pub tags: BTreeMap<Tag, String>,
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
        self.tags.get(target_key)
    }

    pub fn get_field(&self, target_field: &str) -> Option<String> {
        match target_field {
            "location" => Some(self.location.clone().path_string()),
            "plays" => Some(self.plays.clone().to_string()),
            "format" => match self.format {
                Some(format) => match format.short_name() {
                    Some(short) => Some(short.to_string()),
                    None => None,
                },
                None => None,
            },
            _ => todo!(), // Other field types are not yet supported
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum URI {
    Local(PathBuf),
    Cue {
        location: PathBuf,
        index: usize,
        start: Duration,
        end: Duration,
    },
    Remote(Service, PathBuf),
}

impl URI {
    pub fn index(&self) -> Result<&usize, Box<dyn Error>> {
        match self {
            URI::Local(_) => Err("\"Local\" has no stored index".into()),
            URI::Remote(_, _) => Err("\"Remote\" has no stored index".into()),
            URI::Cue { index, .. } => Ok(index),
        }
    }

    /// Returns the start time of a CUEsheet song, or an
    /// error if the URI is not a Cue variant
    pub fn start(&self) -> Result<&Duration, Box<dyn Error>> {
        match self {
            URI::Local(_) => Err("\"Local\" has no starting time".into()),
            URI::Remote(_, _) => Err("\"Remote\" has no starting time".into()),
            URI::Cue { start, .. } => Ok(start),
        }
    }

    /// Returns the end time of a CUEsheet song, or an
    /// error if the URI is not a Cue variant
    pub fn end(&self) -> Result<&Duration, Box<dyn Error>> {
        match self {
            URI::Local(_) => Err("\"Local\" has no starting time".into()),
            URI::Remote(_, _) => Err("\"Remote\" has no starting time".into()),
            URI::Cue { end, .. } => Ok(end),
        }
    }

    /// Returns the location as a PathBuf
    pub fn path(&self) -> &PathBuf {
        match self {
            URI::Local(location) => location,
            URI::Cue { location, .. } => location,
            URI::Remote(_, location) => location,
        }
    }

    pub fn path_string(&self) -> String {
        let path_str = match self {
            URI::Local(location) => location.as_path().to_string_lossy(),
            URI::Cue { location, .. } => location.as_path().to_string_lossy(),
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
    None,
}

#[derive(Clone, Debug)]
pub struct Album<'a> {
    title: &'a String,
    artist: Option<&'a String>,
    cover: Option<&'a AlbumArt>,
    discs: BTreeMap<usize, Vec<&'a Song>>,
}

impl Album<'_> {
    /// Returns the album title
    pub fn title(&self) -> &String {
        self.title
    }

    /// Returns the Album Artist, if they exist
    pub fn artist(&self) -> Option<&String> {
        self.artist
    }

    /// Returns the album cover as an AlbumArt struct, if it exists
    pub fn cover(&self) -> Option<&AlbumArt> {
        self.cover
    }

    pub fn tracks(&self) -> Vec<&Song> {
        let mut songs = Vec::new();
        for disc in &self.discs {
            songs.append(&mut disc.1.clone())
        }
        songs
    }

    pub fn discs(&self) -> &BTreeMap<usize, Vec<&Song>> {
        &self.discs
    }

    /// Returns the specified track at `index` from the album, returning
    /// an error if the track index is out of range
    pub fn track(&self, disc: usize, index: usize) -> Option<&Song> {
        Some(self.discs.get(&disc)?[index])
    }

    /// Returns the number of songs in the album
    pub fn len(&self) -> usize {
        let mut total = 0;
        for disc in &self.discs {
            total += disc.1.len();
        }
        total
    }
}

const BLOCKED_EXTENSIONS: [&str; 3] = ["vob", "log", "txt"];

#[derive(Debug)]
pub struct MusicLibrary {
    pub library: Vec<Song>,
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

        match global_config.db_path.exists() {
            true => {
                library = read_library(*global_config.db_path.clone())?;
            }
            false => {
                // Create the database if it does not exist
                // possibly from the backup file
                if backup_path.exists() {
                    library = read_library(*backup_path.clone())?;
                    write_library(&library, global_config.db_path.to_path_buf(), false)?;
                } else {
                    write_library(&library, global_config.db_path.to_path_buf(), false)?;
                }
            }
        };

        Ok(Self { library })
    }

    /// Serializes the database out to the file specified in the config
    pub fn save(&self, config: &Config) -> Result<(), Box<dyn Error>> {
        match config.db_path.try_exists() {
            Ok(exists) => {
                write_library(&self.library, config.db_path.to_path_buf(), exists)?;
            }
            Err(error) => return Err(error.into()),
        }

        Ok(())
    }

    /// Returns the library size in number of tracks
    pub fn size(&self) -> usize {
        self.library.len()
    }

    /// Queries for a [Song] by its [URI], returning a single `Song`
    /// with the `URI` that matches
    fn query_uri(&self, path: &URI) -> Option<(&Song, usize)> {
        let result = self.library.par_iter().enumerate().try_for_each(|(i, track)| {
            if path == &track.location {
                return std::ops::ControlFlow::Break((track, i));
            }
            Continue(())
        });

        match result {
            Break(song) => Some(song),
            Continue(_) => None,
        }
    }

    /// Queries for a [Song] by its [PathBuf], returning a `Vec<Song>`
    /// with matching `PathBuf`s
    fn query_path(&self, path: &PathBuf) -> Option<Vec<&Song>> {
        let result: Arc<Mutex<Vec<&Song>>> = Arc::new(Mutex::new(Vec::new()));
        let _ = self.library.par_iter().for_each(|track| {
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

    /// Finds all the audio files within a specified folder
    pub fn scan_folder(
        &mut self,
        target_path: &str,
        config: &Config,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let mut total = 0;
        for target_file in WalkDir::new(target_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = target_file.path();

            // Ensure the target is a file and not a directory,
            // if it isn't a file, skip this loop
            if !path.is_file() {
                continue;
            }

            /* TODO: figure out how to increase the speed of this maybe
            // Check if the file path is already in the db
            if self.query_uri(&URI::Local(path.to_path_buf())).is_some() {
                continue;
            }

            // Save periodically while scanning
            i += 1;
            if i % 500 == 0 {
                self.save(config).unwrap();
            }
            */

            let format = FileFormat::from_file(&path)?;
            let extension = match path.extension() {
                Some(ext) => ext.to_string_lossy().to_ascii_lowercase(),
                None => String::new(),
            };

            // If it's a normal file, add it to the database
            // if it's a cuesheet, do a bunch of fancy stuff
            if (format.kind() == Kind::Audio || format.kind() == Kind::Video)
                && !BLOCKED_EXTENSIONS.contains(&extension.as_str())
            {
                match self.add_file(&target_file.path()) {
                    Ok(_) => total += 1,
                    Err(_error) => {
                        println!("{}, {:?}: {}", format, target_file.file_name(), _error)
                    } // TODO: Handle more of these errors
                };
            } else if extension == "cue" {
                total += match self.add_cuesheet(&target_file.path().to_path_buf()) {
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

        let mut tags: BTreeMap<Tag, String> = BTreeMap::new();
        for item in tag.items() {
            let key = match item.key() {
                ItemKey::TrackTitle => Tag::Title,
                ItemKey::TrackNumber => Tag::Track,
                ItemKey::TrackArtist => Tag::Artist,
                ItemKey::AlbumArtist => Tag::AlbumArtist,
                ItemKey::Genre => Tag::Genre,
                ItemKey::Comment => Tag::Comment,
                ItemKey::AlbumTitle => Tag::Album,
                ItemKey::DiscNumber => Tag::Disk,
                ItemKey::Unknown(unknown) if unknown == "ACOUSTID_FINGERPRINT" => continue,
                ItemKey::Unknown(unknown) => Tag::Key(unknown.to_string()),
                custom => Tag::Key(format!("{:?}", custom)),
            };

            let value = match item.value() {
                ItemValue::Text(value) => value.clone(),
                ItemValue::Locator(value) => value.clone(),
                ItemValue::Binary(bin) => format!("BIN#{}", general_purpose::STANDARD.encode(bin)),
            };

            tags.insert(key, value);
        }

        // Get all the album artwork information from the file
        let mut album_art: Vec<AlbumArt> = Vec::new();
        for (i, _art) in tag.pictures().iter().enumerate() {
            let new_art = AlbumArt::Embedded(i as usize);

            album_art.push(new_art)
        }

        // Find images around the music file that can be used
        let mut found_images = find_images(&target_file.to_path_buf()).unwrap();
        album_art.append(&mut found_images);

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
            match self.remove_uri(&URI::Local(audio_location.clone())) {
                Ok(_) => tracks_added -= 1,
                Err(_) => ()
            };

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
            let mut tags: BTreeMap<Tag, String> = BTreeMap::new();
            tags.insert(Tag::Album, album_title.clone());
            tags.insert(Tag::Key("AlbumArtist".to_string()), album_artist.clone());
            tags.insert(Tag::Track, (i + 1).to_string());
            match track.get_cdtext().read(cue::cd_text::PTI::Title) {
                Some(title) => tags.insert(Tag::Title, title),
                None => match track.get_cdtext().read(cue::cd_text::PTI::UPC_ISRC) {
                    Some(title) => tags.insert(Tag::Title, title),
                    None => {
                        let namestr = format!("{} - {}", i, track.get_filename());
                        tags.insert(Tag::Title, namestr)
                    }
                },
            };
            match track.get_cdtext().read(cue::cd_text::PTI::Performer) {
                Some(artist) => tags.insert(Tag::Artist, artist),
                None => None,
            };
            match track.get_cdtext().read(cue::cd_text::PTI::Genre) {
                Some(genre) => tags.insert(Tag::Genre, genre),
                None => None,
            };
            match track.get_cdtext().read(cue::cd_text::PTI::Message) {
                Some(comment) => tags.insert(Tag::Comment, comment),
                None => None,
            };

            let album_art = Vec::new();

            let new_song = Song {
                location: URI::Cue {
                    location: audio_location,
                    index: i,
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
    /// The order in which the sort by Vec is arranged
    /// determines the output sorting.
    ///
    /// Example:
    /// ```
    /// query_tracks(
    ///     &String::from("query"),
    ///     &vec![
    ///         Tag::Title
    ///     ],
    ///     &vec![
    ///         Tag::Field("location".to_string()),
    ///         Tag::Album,
    ///         Tag::Disk,
    ///         Tag::Track,
    ///     ],
    /// )
    /// ```
    /// This would find all titles containing the sequence
    /// "query", and would return the results sorted first
    /// by path, then album, disk number, and finally track number.
    pub fn query_tracks(
        &self,
        query_string: &String,  // The query itself
        target_tags: &Vec<Tag>, // The tags to search
        sort_by: &Vec<Tag>,     // Tags to sort the resulting data by
    ) -> Option<Vec<&Song>> {
        let songs = Arc::new(Mutex::new(Vec::new()));
        //let matcher = SkimMatcherV2::default();

        self.library.par_iter().for_each(|track| {
            for tag in target_tags {
                let track_result = match tag {
                    Tag::Field(target) => match track.get_field(&target) {
                        Some(value) => value,
                        None => continue,
                    },
                    _ => match track.get_tag(&tag) {
                        Some(value) => value.clone(),
                        None => continue,
                    },
                };

                /*
                let match_level = match matcher.fuzzy_match(&normalize(&track_result), &normalize(query_string)) {
                    Some(conf) => conf,
                    None => continue
                };

                if match_level > 100 {
                    songs.lock().unwrap().push(track);
                    return;
                }
                */

                if normalize(&track_result.to_string()).contains(&normalize(&query_string.to_owned())) {
                    songs.lock().unwrap().push(track);
                    return;
                }
            }
        });

        let lock = Arc::try_unwrap(songs).expect("Lock still has multiple owners!");
        let mut new_songs = lock.into_inner().expect("Mutex cannot be locked!");

        // Sort the returned list of songs
        new_songs.par_sort_by(|a, b| {
            for sort_option in sort_by {
                let tag_a = match sort_option {
                    Tag::Field(field_selection) => match a.get_field(field_selection) {
                        Some(field_value) => field_value,
                        None => continue,
                    },
                    _ => match a.get_tag(&sort_option) {
                        Some(tag_value) => tag_value.to_owned(),
                        None => continue,
                    },
                };

                let tag_b = match sort_option {
                    Tag::Field(field_selection) => match b.get_field(field_selection) {
                        Some(field_value) => field_value,
                        None => continue,
                    },
                    _ => match b.get_tag(&sort_option) {
                        Some(tag_value) => tag_value.to_owned(),
                        None => continue,
                    },
                };

                if let (Ok(num_a), Ok(num_b)) = (tag_a.parse::<i32>(), tag_b.parse::<i32>()) {
                    // If parsing succeeds, compare as numbers
                    return num_a.cmp(&num_b);
                } else {
                    // If parsing fails, compare as strings
                    return tag_a.cmp(&tag_b);
                }
            }

            // If all tags are equal, sort by Track number
            let path_a = PathBuf::from(a.get_field("location").unwrap());
            let path_b = PathBuf::from(b.get_field("location").unwrap());

            path_a.file_name().cmp(&path_b.file_name())
        });

        if new_songs.len() > 0 {
            Some(new_songs)
        } else {
            None
        }
    }

    /// Generates all albums from the track list
    pub fn albums(&self) -> BTreeMap<String, Album> {
        let mut albums: BTreeMap<String, Album> = BTreeMap::new();
        for result in &self.library {
            let title = match result.get_tag(&Tag::Album) {
                Some(title) => title,
                None => continue,
            };
            let norm_title = normalize(title);

            let disc_num = result
                .get_tag(&Tag::Disk)
                .unwrap_or(&"".to_string())
                .parse::<usize>()
                .unwrap_or(1);

            match albums.get_mut(&norm_title) {
                // If the album is in the list, add the track to the appropriate disc in it
                Some(album) => match album.discs.get_mut(&disc_num) {
                    Some(disc) => disc.push(result),
                    None => {
                        album.discs.insert(disc_num, vec![result]);
                    }
                },
                // If the album is not in the list, make a new one and add it
                None => {
                    let album_art = result.album_art.get(0);

                    let new_album = Album {
                        title,
                        artist: result.get_tag(&Tag::AlbumArtist),
                        discs: BTreeMap::from([(disc_num, vec![result])]),
                        cover: album_art,
                    };
                    albums.insert(norm_title, new_album);
                }
            }
        }

        // Sort the tracks in each disk in each album
        let blank = String::from("");
        albums.par_iter_mut().for_each(|album| {
            for disc in &mut album.1.discs {
                disc.1.par_sort_by(|a, b| {
                    let a_track = a.get_tag(&Tag::Track).unwrap_or(&blank);
                    let b_track = b.get_tag(&Tag::Track).unwrap_or(&blank);

                    if let (Ok(num_a), Ok(num_b)) = (a_track.parse::<i32>(), b_track.parse::<i32>())
                    {
                        // If parsing the track numbers succeeds, compare as numbers
                        num_a.cmp(&num_b)
                    } else {
                        // If parsing doesn't succeed, compare the locations
                        let path_a = PathBuf::from(a.get_field("location").unwrap());
                        let path_b = PathBuf::from(b.get_field("location").unwrap());

                        path_a.file_name().cmp(&path_b.file_name())
                    }
                });
            }
        });

        // Return the albums!
        albums
    }

    /// Queries a list of albums by title
    pub fn query_albums(
        &self,
        query_string: &String, // The query itself
    ) -> Result<Vec<Album>, Box<dyn Error>> {
        let all_albums = self.albums();

        let normalized_query = normalize(query_string);
        let albums: Vec<Album> = all_albums
            .par_iter()
            .filter_map(|album| {
                if normalize(album.0).contains(&normalized_query) {
                    Some(album.1.clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(albums)
    }
}
