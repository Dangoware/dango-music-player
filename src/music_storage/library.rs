// Crate things
use super::utils::{find_images, normalize, read_file, write_file};
use crate::config::config::Config;

// Various std things
use std::collections::BTreeMap;
use std::error::Error;
use std::io::BufWriter;
use std::ops::ControlFlow::{Break, Continue};
use std::ops::Deref;

// Files
use file_format::{FileFormat, Kind};
use glib::filename_to_uri;
use image::guess_format;
use lofty::{AudioFile, ItemKey, ItemValue, ParseOptions, Probe, TagType, TaggedFileExt};
use rcue::parser::parse_from_file;
use tempfile::tempfile;
use uuid::Uuid;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Time
use chrono::{serde::ts_milliseconds_option, DateTime, Utc};
use std::time::Duration;

// Serialization/Compression
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

// Fun parallel stuff
use rayon::prelude::*;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum AlbumArt {
    Embedded(usize),
    External(URI),
}

impl AlbumArt {
    pub fn uri(&self) -> Option<&URI> {
        match self {
            Self::Embedded(_) => None,
            Self::External(uri) => Some(uri),
        }
    }
}

/// A tag for a song
#[non_exhaustive]
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

/// A field within a Song struct
#[derive(Debug)]
pub enum Field {
    Location(URI),
    Plays(i32),
    Skips(i32),
    Favorited(bool),
    Rating(u8),
    Format(FileFormat),
    Duration(Duration),
    PlayTime(Duration),
    LastPlayed(DateTime<Utc>),
    DateAdded(DateTime<Utc>),
    DateModified(DateTime<Utc>),
}

impl ToString for Field {
    fn to_string(&self) -> String {
        match self {
            Self::Location(location) => location.to_string(),
            Self::Plays(plays) => plays.to_string(),
            Self::Skips(skips) => skips.to_string(),
            Self::Favorited(fav) => fav.to_string(),
            Self::Rating(rating) => rating.to_string(),
            Self::Format(format) => match format.short_name() {
                Some(name) => name.to_string(),
                None => format.to_string(),
            },
            Self::Duration(duration) => duration.as_millis().to_string(),
            Self::PlayTime(time) => time.as_millis().to_string(),
            Self::LastPlayed(last) => last.to_rfc2822(),
            Self::DateAdded(added) => added.to_rfc2822(),
            Self::DateModified(modified) => modified.to_rfc2822(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum BannedType {
    Shuffle,
    All,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum DoNotTrack {
    // TODO: add services to not track
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
enum SongType {
    // TODO: add song types
    Main,
    Instrumental,
    Remix,
    Custom(String)
}

/// Stores information about a single song
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Song {
    pub location: URI,
    pub uuid: Uuid,
    pub plays: i32,
    pub skips: i32,
    pub favorited: bool,
    // pub banned: Option<BannedType>,
    pub rating: Option<u8>,
    pub format: Option<FileFormat>,
    pub duration: Duration,
    pub play_time: Duration,
    #[serde(with = "ts_milliseconds_option")]
    pub last_played: Option<DateTime<Utc>>,
    #[serde(with = "ts_milliseconds_option")]
    pub date_added: Option<DateTime<Utc>>,
    #[serde(with = "ts_milliseconds_option")]
    pub date_modified: Option<DateTime<Utc>>,
    pub album_art: Vec<AlbumArt>,
    pub tags: BTreeMap<Tag, String>,
}

#[test]
fn get_art_test() {
    use urlencoding::decode;

    let s = Song::from_file(Path::new("F:\\Music\\Mp3\\ななひら\\Colory Starry\\05 - Fly Away!.mp3")).unwrap();
    s.open_album_art(0).inspect_err(|e| println!("{e:?}"));

}

impl Song {
    /// Get a tag's value
    ///
    /// ```
    /// use dango_core::music_storage::music_db::Tag;
    /// // Assuming an already created song:
    ///
    /// let tag = this_song.get_tag(Tag::Title);
    ///
    /// assert_eq!(tag, "Some Song Title");
    /// ```
    pub fn get_tag(&self, target_key: &Tag) -> Option<&String> {
        self.tags.get(target_key)
    }

    /// Gets an internal field from a song
    pub fn get_field(&self, target_field: &str) -> Option<Field> {
        let lower_target = target_field.to_lowercase();
        match lower_target.as_str() {
            "location" => Some(Field::Location(self.location.clone())),
            "plays" => Some(Field::Plays(self.plays)),
            "skips" => Some(Field::Skips(self.skips)),
            "favorited" => Some(Field::Favorited(self.favorited)),
            "rating" => self.rating.map(Field::Rating),
            "duration" => Some(Field::Duration(self.duration)),
            "play_time" => Some(Field::PlayTime(self.play_time)),
            "format" => self.format.map(Field::Format),
            _ => todo!(), // Other field types are not yet supported
        }
    }

    /// Sets the value of a tag in the song
    pub fn set_tag(&mut self, target_key: Tag, new_value: String) {
        self.tags.insert(target_key, new_value);
    }

    /// Deletes a tag from the song
    pub fn remove_tag(&mut self, target_key: &Tag) {
        self.tags.remove(target_key);
    }

    /// Creates a `Song` from a song file
    pub fn from_file(target_file: &Path) -> Result<Self, Box<dyn Error>> {
        let normal_options = ParseOptions::new().parsing_mode(lofty::ParsingMode::Relaxed);

        let blank_tag = &lofty::Tag::new(TagType::Id3v2);
        let tagged_file: lofty::TaggedFile;
        let mut duration = Duration::from_secs(0);
        let tag = match Probe::open(target_file)?.options(normal_options).read() {
            Ok(file) => {
                tagged_file = file;

                duration = tagged_file.properties().duration();

                // Ensure the tags exist, if not, insert blank data
                match tagged_file.primary_tag() {
                    Some(primary_tag) => primary_tag,

                    None => match tagged_file.first_tag() {
                        Some(first_tag) => first_tag,
                        None => blank_tag,
                    },
                }
            }

            Err(_) => blank_tag,
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
                ItemKey::Unknown(unknown)
                    if unknown == "ACOUSTID_FINGERPRINT" || unknown == "Acoustid Fingerprint" =>
                {
                    continue
                }
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
            let new_art = AlbumArt::Embedded(i);

            album_art.push(new_art)
        }

        // Find images around the music file that can be used
        let mut found_images = find_images(target_file).unwrap();
        album_art.append(&mut found_images);

        // Get the format as a string
        let format: Option<FileFormat> = match FileFormat::from_file(target_file) {
            Ok(fmt) => Some(fmt),
            Err(_) => None,
        };

        // TODO: Fix error handling
        let binding = fs::canonicalize(target_file).unwrap();

        let new_song = Song {
            location: URI::Local(binding),
            uuid: Uuid::new_v4(),
            plays: 0,
            skips: 0,
            favorited: false,
            // banned: None,
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
        Ok(new_song)
    }

    /// creates a `Vec<Song>` from a cue file

    pub fn from_cue(cuesheet: &Path) -> Result<Vec<(Self, PathBuf)>, Box<dyn Error>> {
        let mut tracks = Vec::new();

        let cue_data = parse_from_file(&cuesheet.to_string_lossy(), false).unwrap();

        // Get album level information
        let album_title = &cue_data.title;
        let album_artist = &cue_data.performer;

        let parent_dir = cuesheet.parent().expect("The file has no parent path??");
        for file in cue_data.files.iter() {
            let audio_location = &parent_dir.join(file.file.clone());


            if !audio_location.exists() {
                continue;
            }

            let next_track = file.tracks.clone();
            let mut next_track = next_track.iter().skip(1);
            for (i, track) in file.tracks.iter().enumerate() {
                // Get the track timing information
                let pregap = match track.pregap {
                    Some(pregap) => pregap,
                    None => Duration::from_secs(0),
                };
                let postgap = match track.postgap {
                    Some(postgap) => postgap,
                    None => Duration::from_secs(0),
                };

                let mut start;
                if track.indices.len() > 1 {
                    start = track.indices[1].1;
                } else {
                    start = track.indices[0].1;
                }
                if !start.is_zero() {
                    start -= pregap;
                }

                let duration = match next_track.next() {
                    Some(future) => match future.indices.first() {
                        Some(val) => val.1 - start,
                        None => Duration::from_secs(0),
                    },
                    None => match lofty::read_from_path(audio_location) {
                        Ok(tagged_file) => tagged_file.properties().duration() - start,

                        Err(_) => match Probe::open(audio_location)?.read() {
                            Ok(tagged_file) => tagged_file.properties().duration() - start,

                            Err(_) => Duration::from_secs(0),
                        },
                    },
                };
                let end = start + duration + postgap;

                // Get the format as a string
                let format: Option<FileFormat> = match FileFormat::from_file(audio_location) {
                    Ok(fmt) => Some(fmt),
                    Err(_) => None,
                };

                // Get some useful tags
                let mut tags: BTreeMap<Tag, String> = BTreeMap::new();
                match album_title {
                    Some(title) => {
                        tags.insert(Tag::Album, title.clone());
                    }
                    None => (),
                }
                match album_artist {
                    Some(artist) => {
                        tags.insert(Tag::Artist, artist.clone());
                    }
                    None => (),
                }
                tags.insert(Tag::Track, track.no.parse().unwrap_or((i + 1).to_string()));
                match track.title.clone() {
                    Some(title) => tags.insert(Tag::Title, title),
                    None => match track.isrc.clone() {
                        Some(title) => tags.insert(Tag::Title, title),
                        None => {
                            let namestr = format!("{} - {}", i, file.file.clone());
                            tags.insert(Tag::Title, namestr)
                        }
                    },
                };
                match track.performer.clone() {
                    Some(artist) => tags.insert(Tag::Artist, artist),
                    None => None,
                };

                // Find images around the music file that can be used
                let album_art = find_images(&audio_location.to_path_buf()).unwrap();

                let new_song = Song {
                    location: URI::Cue {
                        location: audio_location.clone(),
                        index: i,
                        start,
                        end,
                    },
                    uuid: Uuid::new_v4(),
                    plays: 0,
                    skips: 0,
                    favorited: false,
                    // banned: None,
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
                tracks.push((new_song, audio_location.clone()));
            }
        }
     Ok(tracks)
    }

    pub fn open_album_art(&self, index: usize) -> Result<(), Box<dyn Error>> {
        use opener::open;
        use urlencoding::decode;

        if index >= self.album_art.len() {
            return Err("index out of bounds?".into());
        }

        let uri: String = match &self.album_art[index] {
            AlbumArt::External(uri) => {
                decode(match uri.as_uri().strip_prefix("file:///") { Some(e) => e, None => return Err("Invalid path?".into()) })?.into_owned()
            },
            AlbumArt::Embedded(_) => {

                let normal_options = ParseOptions::new().parsing_mode(lofty::ParsingMode::Relaxed);
                let blank_tag = &lofty::Tag::new(TagType::Id3v2);
                let tagged_file: lofty::TaggedFile;

                let uri = dbg!(urlencoding::decode(self.location.as_uri().strip_prefix("file:///").unwrap())?.into_owned());

                let tag = match Probe::open(uri)?.options(normal_options).read() {
                    Ok(file) => {
                        tagged_file = file;

                        match tagged_file.primary_tag() {
                            Some(primary_tag) => primary_tag,

                            None => match tagged_file.first_tag() {
                                Some(first_tag) => first_tag,
                                None => blank_tag,
                            },
                        }
                    }

                    Err(_) => blank_tag,
                };

                let data = tag.pictures()[index].data();
                let format = dbg!(guess_format(data)?);
                let img = image::load_from_memory(data)?;

                let mut location = String::new();
                let i: u32 = 0;
                loop {
                    use image::ImageFormat::*;
                    //TODO: create a place for temporary images
                    let fmt = match format {
                        Jpeg => "jpeg",
                        Png => "png",
                        _ => todo!(),
                    };

                    location = format!("./test-config/images/tempcover{i}.{fmt}.tmp");
                    break;
                };
                img.save_with_format(&location, format)?;

                location.to_string()
            },
        };
        open(uri)?;

        Ok(())
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
    Remote(Service, String),
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
    pub fn path(&self) -> PathBuf {
        match self {
            URI::Local(location) => location.clone(),
            URI::Cue { location, .. } => location.clone(),
            URI::Remote(_, location) => PathBuf::from(location),
        }
    }

    pub fn as_uri(&self) -> String {
        let path_str = match self {
            URI::Local(location) => filename_to_uri(location, None)
                .expect("couldn't convert path to URI")
                .to_string(),
            URI::Cue { location, .. } => filename_to_uri(location, None)
                .expect("couldn't convert path to URI")
                .to_string(),
            URI::Remote(_, location) => location.clone(),
        };
        path_str.to_string()
    }

    pub fn exists(&self) -> Result<bool, std::io::Error> {
        match self {
            URI::Local(loc) => loc.try_exists(),
            URI::Cue {location, ..} => location.try_exists(),
            URI::Remote(_, _loc) => todo!(),
        }
    }
}

impl ToString for URI {
    fn to_string(&self) -> String {
        let path_str = match self {
            URI::Local(location) => location.as_path().to_string_lossy(),
            URI::Cue { location, .. } => location.as_path().to_string_lossy(),
            URI::Remote(_, location) => location.into(),
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

#[derive(Clone, Debug, PartialEq)]
pub struct Album<'a> {
    title: &'a String,
    artist: Option<&'a String>,
    cover: Option<&'a AlbumArt>,
    discs: BTreeMap<usize, Vec<&'a Song>>,
}

#[allow(clippy::len_without_is_empty)]
impl Album<'_> {
    //returns the Album title
    pub fn title(&self) -> &String {
        self.title
    }

    /// Returns the album cover as an AlbumArt struct, if it exists
    fn cover(&self) -> Option<&AlbumArt> {
        self.cover
    }

    /// Returns the Album Artist, if they exist
    pub fn artist(&self) -> Option<&String> {
        self.artist
    }

    pub fn discs(&self) -> &BTreeMap<usize, Vec<&Song>> {
        &self.discs
    }
    /// Returns the specified track at `index` from the album, returning
    /// an error if the track index is out of range
    pub fn track(&self, disc: usize, index: usize) -> Option<&Song> {
        Some(self.discs.get(&disc)?[index])
    }

    fn tracks(&self) -> Vec<&Song> {
        let mut songs = Vec::new();
        for disc in &self.discs {
            songs.append(&mut disc.1.clone())
        }
        songs
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

const BLOCKED_EXTENSIONS: [&str; 4] = ["vob", "log", "txt", "sf2"];

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicLibrary {
    pub name: String,
    pub uuid: Uuid,
    pub library: Vec<Song>,
}

#[test]
fn library_init() {
    let config = Config::read_file(PathBuf::from("test_config/config_test.json")).unwrap();
    let target_uuid = config.libraries.libraries[0].uuid;
    let a = MusicLibrary::init(Arc::new(RwLock::from(config)), target_uuid).unwrap();
    dbg!(a);
}

impl MusicLibrary {
    /// Create a new library from a name and [Uuid]
    fn new(name: String, uuid: Uuid) -> Self {
        MusicLibrary {
            name,
            uuid,
            library: Vec::new(),
        }
    }

    /// Initialize the database
    ///
    /// If the database file already exists, return the [MusicLibrary], otherwise create
    /// the database first. This needs to be run before anything else to retrieve
    /// the [MusicLibrary] Vec
    pub fn init(config: Arc<RwLock<Config>>, uuid: Uuid) -> Result<Self, Box<dyn Error>> {
        let global_config = &*config.read().unwrap();
        let path = global_config.libraries.get_library(&uuid)?.path;

        let library: MusicLibrary = match path.exists() {
            true => read_file(path)?,
            false => {
                // If the library does not exist, re-create it
                let lib = MusicLibrary::new(String::new(), uuid);
                write_file(&lib, path)?;
                lib
            }
        };

        Ok(library)
    }

    //#[cfg(debug_assertions)] // We probably wouldn't want to use this for real, but maybe it would have some utility?
    pub fn from_path(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        let library: MusicLibrary = match path.exists() {
            true => read_file(path)?,
            false => {
                let lib = MusicLibrary::new(String::new(), Uuid::new_v4());
                write_file(&lib, path)?;
                lib
            }
        };
        Ok(library)
    }

    /// Serializes the database out to the file specified in the config
    pub fn save(&self, config: Config) -> Result<(), Box<dyn Error>> {
        let path = config.libraries.get_library(&self.uuid)?.path;
        match path.try_exists() {
            Ok(_) => write_file(self, path)?,
            Err(error) => return Err(error.into()),
        }

        Ok(())
    }

    /// Returns the library size in number of tracks
    pub fn len_tracks(&self) -> usize {
        self.library.len()
    }

    /// Returns the library size in number of albums
    pub fn len_albums(&self) -> usize {
        self.albums().len()
    }

    /// Queries for a [Song] by its [URI], returning a single `Song`
    /// with the `URI` that matches along with its position in the library
    pub fn query_uri(&self, path: &URI) -> Option<(&Song, usize)> {
        let result = self
            .library
            .par_iter()
            .enumerate()
            .try_for_each(|(i, track)| {
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

    /// Queries for a [Song] by its [Uuid], returning a single `Song`
    /// with the `Uuid` that matches along with its position in the library
    pub fn query_uuid(&self, uuid: &Uuid) -> Option<(&Song, usize)> {
        let result = self
            .library
            .par_iter()
            .enumerate()
            .try_for_each(|(i, track)| {
                if uuid == &track.uuid {
                    return std::ops::ControlFlow::Break((track, i));
                }
                Continue(())
            });

        match result {
            Break(song) => Some(song),
            Continue(_) => None,
        }
    }

    /// Queries for a [Song] by its [PathBuf], returning a `Vec<&Song>`
    /// with matching `PathBuf`s
    fn query_path(&self, path: PathBuf) -> Option<Vec<&Song>> {
        let result: Arc<Mutex<Vec<&Song>>> = Arc::new(Mutex::new(Vec::new()));
        self.library.par_iter().for_each(|track| {
            if path == track.location.path() {
                result.clone().lock().unwrap().push(track);
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
    ) -> Result<i32, Box<dyn std::error::Error>> {
        let mut total = 0;
        let mut errors = 0;
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

            let format = FileFormat::from_file(path)?;
            let extension = match path.extension() {
                Some(ext) => ext.to_string_lossy().to_ascii_lowercase(),
                None => String::new(),
            };

            // If it's a normal file, add it to the database
            // if it's a cuesheet, do a bunch of fancy stuff
            if (format.kind() == Kind::Audio || format.kind() == Kind::Video)
                && !BLOCKED_EXTENSIONS.contains(&extension.as_str())
            {
                match self.add_file(target_file.path()) {
                    Ok(_) => total += 1,
                    Err(_error) => {
                        errors += 1;
                        //println!("{}, {:?}: {}", format, target_file.file_name(), _error)
                    } // TODO: Handle more of these errors
                };
            } else if extension == "cue" {
                total += match self.add_cuesheet(target_file.path()) {
                    Ok(added) => added,
                    Err(error) => {
                        errors += 1;
                        //println!("{}", error);
                        0
                    }
                }
            }
        }

        Ok(total)
    }

    pub fn add_file(&mut self, target_file: &Path) -> Result<(), Box<dyn Error>> {

        let new_song = Song::from_file(target_file)?;
        match self.add_song(new_song) {
            Ok(_) => (),
            Err(_) => {
                //return Err(error)
            }
        };

        Ok(())
    }

    pub fn add_cuesheet(&mut self, cuesheet: &Path) -> Result<i32, Box<dyn Error>> {
        let tracks = Song::from_cue(cuesheet)?;
        let mut tracks_added = tracks.len() as i32;


        for (new_song, location) in tracks {
            // Try to remove the original audio file from the db if it exists
            if self.remove_uri(&URI::Local(location.clone())).is_ok() {
                tracks_added -= 1
            }
            match self.add_song(new_song) {
                Ok(_) => {},
                Err(_error) => {
                    //println!("{}", _error);
                    continue;
                }
            };
        }
        Ok(tracks_added)
    }

    pub fn add_song(&mut self, new_song: Song) -> Result<(), Box<dyn Error>> {
        if self.query_uri(&new_song.location).is_some() {
            return Err(format!("URI already in database: {:?}", new_song.location).into());
        }

        match new_song.location {
            URI::Local(_) if self.query_path(new_song.location.path()).is_some() => {
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
    pub fn update_uri(
        &mut self,
        target_uri: &URI,
        new_tags: Vec<Tag>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let target_song = match self.query_uri(target_uri) {
            Some(song) => song,
            None => return Err("URI not in database!".to_string().into()),
        };

        println!("{:?}", target_song.0.location);

        for tag in new_tags {
            println!("{:?}", tag);
        }

        todo!()
    }

    /// Query the database, returning a list of [Song]s
    ///
    /// The order in which the `sort by` Vec is arranged
    /// determines the output sorting.
    ///
    /// Example:
    /// ```
    /// use dango_core::music_storage::music_db::Tag;
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
                    Tag::Field(target) => match track.get_field(target) {
                        Some(value) => value.to_string(),
                        None => continue,
                    },
                    _ => match track.get_tag(tag) {
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

                if normalize(&track_result.to_string())
                    .contains(&normalize(&query_string.to_owned()))
                {
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
                        Some(field_value) => field_value.to_string(),
                        None => continue,
                    },
                    _ => match a.get_tag(sort_option) {
                        Some(tag_value) => tag_value.to_owned(),
                        None => continue,
                    },
                };

                let tag_b = match sort_option {
                    Tag::Field(field_selection) => match b.get_field(field_selection) {
                        Some(field_value) => field_value.to_string(),
                        None => continue,
                    },
                    _ => match b.get_tag(sort_option) {
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
            let path_a = PathBuf::from(a.get_field("location").unwrap().to_string());
            let path_b = PathBuf::from(b.get_field("location").unwrap().to_string());

            path_a.file_name().cmp(&path_b.file_name())
        });

        if !new_songs.is_empty() {
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
                    let album_art = result.album_art.first();

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
                        let path_a = PathBuf::from(a.get_field("location").unwrap().to_string());
                        let path_b = PathBuf::from(b.get_field("location").unwrap().to_string());

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
        query_string: &str, // The query itself
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