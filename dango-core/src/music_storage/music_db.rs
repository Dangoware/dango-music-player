use file_format::{FileFormat, Kind};
use serde::Deserialize;
use lofty::{Accessor, AudioFile, Probe, TaggedFileExt, ItemKey, ItemValue, TagType};
use rusqlite::{params, Connection};
use cue::{cd_text::PTI, cd::CD};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use time::Date;
use walkdir::WalkDir;

use crate::music_controller::config::Config;

#[derive(Debug, Clone)]
pub struct Song {
    pub path: URI,
    pub title:  Option<String>,
    pub album:  Option<String>,
    pub tracknum: Option<usize>,
    pub artist: Option<String>,
    pub date:   Option<Date>,
    pub genre:  Option<String>,
    pub plays:  Option<usize>,
    pub favorited: Option<bool>,
    pub format: Option<FileFormat>, // TODO: Make this a proper FileFormat eventually
    pub duration: Option<Duration>,
    pub custom_tags: Option<Vec<Tag>>,
}

#[derive(Clone, Debug)]
pub enum URI{
    Local(String),
    Remote(Service, String),
}

#[derive(Clone, Copy, Debug)]
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

pub fn create_db() -> Result<(), rusqlite::Error> {
    let path = "./music_database.db3";
    let db_connection = Connection::open(path)?;

    db_connection.pragma_update(None, "synchronous", "0")?;
    db_connection.pragma_update(None, "journal_mode", "WAL")?;

    // Create the important tables
    db_connection.execute(
        "CREATE TABLE music_collection (
            song_path TEXT PRIMARY KEY,
            title   TEXT,
            album   TEXT,
            tracknum INTEGER,
            artist  TEXT,
            date    INTEGER,
            genre   TEXT,
            plays   INTEGER,
            favorited BLOB,
            format  TEXT,
            duration INTEGER
        )",
        (), // empty list of parameters.
    )?;

    db_connection.execute(
        "CREATE TABLE playlists (
            playlist_name TEXT NOT NULL,
            song_path   TEXT NOT NULL,
            FOREIGN KEY(song_path) REFERENCES music_collection(song_path)
        )",
        (), // empty list of parameters.
    )?;

    db_connection.execute(
        "CREATE TABLE custom_tags (
            song_path TEXT NOT NULL,
            tag TEXT NOT NULL,
            tag_value TEXT,
            FOREIGN KEY(song_path) REFERENCES music_collection(song_path)
        )",
        (), // empty list of parameters.
    )?;

    Ok(())
}

fn path_in_db(query_path: &Path, connection: &Connection) -> bool {
    let query_string = format!("SELECT EXISTS(SELECT 1 FROM music_collection WHERE song_path='{}')", query_path.to_string_lossy());

    let mut query_statement = connection.prepare(&query_string).unwrap();
    let mut rows = query_statement.query([]).unwrap();

    match rows.next().unwrap() {
        Some(value) => value.get::<usize, bool>(0).unwrap(),
        None => false
    }
}

/// Parse a cuesheet given a path and a directory it is located in,
/// returning a Vec of Song objects
fn parse_cuesheet(
    cuesheet_path: &Path,
    current_dir: &PathBuf
) -> Result<Vec<Song>, Box<dyn std::error::Error>>{
    let cuesheet = CD::parse_file(cuesheet_path.to_path_buf())?;

    let album = cuesheet.get_cdtext().read(PTI::Title);

    let mut song_list:Vec<Song> = vec![];

    for (index, track) in cuesheet.tracks().iter().enumerate() {
        let track_string_path = format!("{}/{}", current_dir.to_string_lossy(), track.get_filename());
        let track_path = Path::new(&track_string_path);

        if !track_path.exists() {continue};

        // Get the format as a string
        let short_format = match FileFormat::from_file(track_path) {
            Ok(fmt) => Some(fmt),
            Err(_) => None
        };

        let duration = Duration::from_secs(track.get_length().unwrap_or(-1) as u64);

        let custom_index_start = Tag::Custom{
            tag: String::from("dango_cue_index_start"),
            tag_value: track.get_index(0).unwrap_or(-1).to_string()
        };
        let custom_index_end = Tag::Custom{
            tag: String::from("dango_cue_index_end"),
            tag_value: track.get_index(0).unwrap_or(-1).to_string()
        };

        let custom_tags: Vec<Tag> = vec![custom_index_start, custom_index_end];

        let tags = track.get_cdtext();
        let cue_song = Song {
            path: URI::Local(String::from("URI")),
            title: tags.read(PTI::Title),
            album: album.clone(),
            tracknum: Some(index + 1),
            artist: tags.read(PTI::Performer),
            date: None,
            genre: tags.read(PTI::Genre),
            plays: Some(0),
            favorited: Some(false),
            format: short_format,
            duration: Some(duration),
            custom_tags: Some(custom_tags)
        };

        song_list.push(cue_song);
    }

    Ok(song_list)
}

pub fn find_all_music(
    config: &Config,
    target_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let db_connection = Connection::open(&*config.db_path)?;

    db_connection.pragma_update(None, "synchronous", "0")?;
    db_connection.pragma_update(None, "journal_mode", "WAL")?;

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
            add_file_to_db(target_file.path(), &db_connection)
        } else if extension.to_ascii_lowercase() == "cue" {
            // TODO: implement cuesheet support
            parse_cuesheet(target_file.path(), &current_dir);
        }
    }

    // create the indexes after all the data is inserted
    db_connection.execute(
        "CREATE INDEX path_index ON music_collection (song_path)", ()
    )?;

    db_connection.execute(
        "CREATE INDEX custom_tags_index ON custom_tags (song_path)", ()
    )?;

    Ok(())
}

pub fn add_file_to_db(target_file: &Path, connection: &Connection) {
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
    for item in tag.items() {
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

        custom_insert.push_str(&format!(" (?1, '{}', '{}')", custom_key.replace("\'", "''"), custom_value.replace("\'", "''")));

        loops += 1;
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

    // Add all the info into the music_collection table
    connection.execute(
        "INSERT INTO music_collection (
            song_path,
            title,
            album,
            tracknum,
            artist,
            date,
            genre,
            plays,
            favorited,
            format,
            duration
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![abs_path, tag.title(), tag.album(), tag.track(), tag.artist(), tag.year(), tag.genre(), 0, false, short_format, duration],
    ).unwrap();

    //TODO: Fix this, it's horrible
    if custom_insert != "" {
        connection.execute(
            &format!("INSERT INTO custom_tags ('song_path', 'tag', 'tag_value') VALUES {}", &custom_insert),
            params![
                abs_path,
            ]
        ).unwrap();
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum Tag {
    SongPath,
    Title,
    Album,
    TrackNum,
    Artist,
    Date,
    Genre,
    Plays,
    Favorited,
    Format,
    Duration,
    Custom{tag: String, tag_value: String},
}

impl Tag {
    fn as_str(&self) -> &str {
        match self {
            Tag::SongPath => "song_path",
            Tag::Title  => "title",
            Tag::Album  => "album",
            Tag::TrackNum => "tracknum",
            Tag::Artist => "artist",
            Tag::Date   => "date",
            Tag::Genre  => "genre",
            Tag::Plays  => "plays",
            Tag::Favorited => "favorited",
            Tag::Format => "format",
            Tag::Duration => "duration",
            Tag::Custom{tag, ..} => tag,
        }
    }
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
    config: &Config,
    text_input: &String,
    queried_tags: &Vec<&Tag>,
    order_by_tags: &Vec<&Tag>,
) -> Option<Vec<MusicObject>> {
    let db_connection = Connection::open(&*config.db_path).unwrap();

    // Set up some database settings
    db_connection.pragma_update(None, "synchronous", "0").unwrap();
    db_connection.pragma_update(None, "journal_mode", "WAL").unwrap();

    // Build the "WHERE" part of the SQLite query
    let mut where_string = String::new();
    let mut loops = 0;
    for tag in queried_tags {
        if loops > 0 {
            where_string.push_str("OR ");
        }

        match tag {
            Tag::Custom{tag, ..} => where_string.push_str(&format!("custom_tags.tag = '{tag}' AND custom_tags.tag_value LIKE '{text_input}' ")),
            Tag::SongPath => where_string.push_str(&format!("music_collection.{} LIKE '{text_input}' ", tag.as_str())),
            _ => where_string.push_str(&format!("{} LIKE '{text_input}' ", tag.as_str()))
        }

        loops += 1;
    }

    // Build the "ORDER BY" part of the SQLite query
    let mut order_by_string = String::new();
    let mut loops = 0;
    for tag in order_by_tags {
        match tag {
            Tag::Custom{..} => continue,
            _ => ()
        }

        if loops > 0 {
            order_by_string.push_str(", ");
        }

        order_by_string.push_str(tag.as_str());

        loops += 1;
    }

    // Build the final query string
    let query_string = format!("
        SELECT music_collection.*, JSON_GROUP_ARRAY(JSON_OBJECT('Custom',JSON_OBJECT('tag', custom_tags.tag, 'tag_value', custom_tags.tag_value))) AS custom_tags
        FROM music_collection
        LEFT JOIN custom_tags ON music_collection.song_path = custom_tags.song_path
        WHERE {where_string}
        GROUP BY music_collection.song_path
        ORDER BY {order_by_string}
    ");
    
    let mut query_statement = db_connection.prepare(&query_string).unwrap();
    let mut rows = query_statement.query([]).unwrap();

    let mut final_result:Vec<MusicObject> = vec![];

    while let Some(row) = rows.next().unwrap() {
        let custom_tags: Vec<Tag> = match row.get::<usize, String>(11) {
            Ok(result) => serde_json::from_str(&result).unwrap_or(vec![]),
            Err(_) => vec![]
        };

        let file_format: FileFormat = FileFormat::from(row.get::<usize, String>(9).unwrap().as_bytes());

        let new_song = Song {
            // TODO: Implement proper errors here
            path:   URI::Local(String::from("URI")),
            title:  row.get::<usize, String>(1).ok(),
            album:  row.get::<usize, String>(2).ok(),
            tracknum: row.get::<usize, usize>(3).ok(),
            artist: row.get::<usize, String>(4).ok(),
            date:   Date::from_calendar_date(row.get::<usize, i32>(5).unwrap_or(0), time::Month::January, 1).ok(), // TODO: Fix this to get the actual date
            genre:  row.get::<usize, String>(6).ok(),
            plays:  row.get::<usize, usize>(7).ok(),
            favorited: row.get::<usize, bool>(8).ok(),
            format: Some(file_format),
            duration: Some(Duration::from_secs(row.get::<usize, u64>(10).unwrap_or(0))),
            custom_tags: Some(custom_tags),
        };

        final_result.push(MusicObject::Song(new_song));
    };

    Some(final_result)
}
