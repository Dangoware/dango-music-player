use crate::music_controller::config::Config;
use file_format::{FileFormat, Kind};
use serde_json::Value;
use lofty::{Accessor, AudioFile, Probe, TaggedFileExt};
use rusqlite::{params, Connection};
use std::any::TypeId;
use std::fs;
use std::path::Path;
use std::time::Duration;
use time::Date;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct Song {
    pub path: Box<Path>,
    title:  Option<String>,
    album:  Option<String>,
    tracknum: Option<usize>,
    artist: Option<String>,
    date:   Option<Date>,
    genre:  Option<String>,
    plays:  Option<usize>,
    favorited: Option<bool>,
    format: Option<FileFormat>,
    duration: Option<Duration>,
    pub custom_tags: Option<Vec<crate::Tag>>,
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

pub fn find_all_music(
    config: &Config,
    target_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let db_connection = Connection::open(&*config.db_path).unwrap();

    db_connection.pragma_update(None, "synchronous", "0").unwrap();
    db_connection.pragma_update(None, "journal_mode", "WAL").unwrap();

    for entry in WalkDir::new(target_path).follow_links(true).into_iter().filter_map(|e| e.ok()) {
        let target_file = entry;
        let is_file = fs::metadata(target_file.path())?.is_file();

        // Ensure the target is a file and not a directory, if it isn't, skip this loop
        if !is_file {
            continue;
        }

        let format = FileFormat::from_file(target_file.path())?;
        let extension = target_file
            .path()
            .extension()
            .expect("Could not find file extension");

        if format.kind() == Kind::Audio {
            add_to_db(target_file.path(), &db_connection)
        }
        // TODO: implement cuesheet support
        /*else if extension == "cue" {
            if let Ok(ret) = fs::read_to_string(target_file.path()) {
                let contents = ret.to_string();
                let cuesheet = CD::parse(contents)?;
                println!("{}", cuesheet.get_track_count());
            }
        }*/
    }

    Ok(())
}

pub fn add_to_db(target_file: &Path, connection: &Connection) {
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

    let tag = match tagged_file.primary_tag() {
        Some(primary_tag) => primary_tag,

        None => tagged_file.first_tag().expect("No tags!~"),
    };
    
    let format = FileFormat::from_file(target_file).unwrap().to_string();
    
    let duration = tagged_file.properties().duration().as_secs().to_string();
    
    // TODO: Fix error handling
    let binding = fs::canonicalize(target_file).unwrap();
    let abs_path = binding.to_str().unwrap();

    // TODO: Ensure we can make custom tags
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
        ) VALUES (
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            ?7,
            ?8,
            ?9,
            ?10,
            ?11
        )",

        params![
            abs_path,
            tag.title(),
            tag.album(),
            tag.track(),
            tag.artist(),
            tag.year(),
            tag.genre(),
            0,
            false,
            format,
            duration
        ],
    ).unwrap();
}

#[derive(Debug)]
pub enum Tag {
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
    Custom{name: String, value: String},
}

impl Tag {
    fn as_str(&self) -> &str {
        match self {
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
            Tag::Custom{name, ..} => name,
        }
    }
}

#[derive(Debug)]
pub enum MusicObject {
    Song(Song),
    Album(Playlist),
    Playlist(Playlist),
}

/// Query the database, returning a list of items
pub fn query(
    config: &Config,
    text_input: &String,
    queried_tags: Vec<&Tag>,
    order_by_tags: Vec<&Tag>,
) -> Option<Vec<MusicObject>> {
    let db_connection = Connection::open(&*config.db_path).unwrap();

    // Set up some database settings
    db_connection.pragma_update(None, "synchronous", "0").unwrap();
    db_connection.pragma_update(None, "journal_mode", "WAL").unwrap();

    // Build the "WHERE" part of the SQLite query
    let mut where_string = "".to_owned();
    let mut loops = 0;
    for tag in queried_tags {
        if loops > 0 {
            where_string.push_str("OR ");
        }

        match tag {
            Tag::Custom{name, ..} => {
                // If the input tag name is blank, search without matching custom tag name
                if name == "" {
                    where_string.push_str(&format!("custom_tags.tag_value LIKE '{text_input}' "))
                } else {
                    where_string.push_str(&format!("custom_tags.tag = '{name}' AND custom_tags.tag_value LIKE '{text_input}' "))
                }
            },
            _ => {
                where_string.push_str(&format!("{} LIKE '{text_input}' ", tag.as_str()))
            }
        }

        loops += 1;
    }

    println!("{}", where_string);

    // Build the "ORDER BY" part of the SQLite query
    let mut order_by_string = "".to_owned();
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
        SELECT music_collection.*, JSON_GROUP_ARRAY(JSON_OBJECT('tag', custom_tags.tag, 'tag_value', custom_tags.tag_value)) AS custom_tags
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
        let custom_tag: Value = match row.get::<usize, String>(11) {
            Ok(result) => serde_json::from_str(&result).unwrap(),
            Err(_) => ().into()
        };

        let mut custom_tags: Vec<Tag> = vec![];

        for tag in custom_tag.as_array().unwrap() {
            println!("{}", tag);
            custom_tags.push(Tag::Custom{
                name: tag["name"].to_string(),
                value:  tag["value"].to_string(),
            });
        }

        let new_song = Song {
            // TODO: Implement proper errors here
            path:   Path::new(&row.get::<usize, String>(0).unwrap_or("".to_owned())).into(),
            title:  row.get::<usize, String>(1).ok(),
            album:  row.get::<usize, String>(2).ok(),
            tracknum: row.get::<usize, usize>(3).ok(),
            artist: row.get::<usize, String>(4).ok(),
            date:   Date::from_calendar_date(row.get::<usize, i32>(5).unwrap_or(0), time::Month::January, 1).ok(),
            genre:  row.get::<usize, String>(6).ok(),
            plays:  row.get::<usize, usize>(7).ok(),
            favorited: row.get::<usize, bool>(8).ok(),
            format: Some(FileFormat::OggOpus),
            duration: Some(Duration::from_secs(row.get::<usize, u64>(10).unwrap_or(0))),
            custom_tags: Some(custom_tags),
        };

        final_result.push(MusicObject::Song(new_song));
    };

    Some(final_result)
}
