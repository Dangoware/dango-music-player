/// SQLite Database handler for the music player
use uuid::Uuid;
use std::path::Path;
use std::time::Duration;
use std::fs;
use cue::cd::CD;
use time::Date;
use file_format::{FileFormat, Kind};
use walkdir::WalkDir;
use rusqlite::Connection;

struct Song {
    uuid: Uuid,
    path: Box<Path>,
    title: String,
    album: String,
    tracknum: usize,
    artist: String,
    plays: usize,
    favorited: bool,
    date: Date,
    format: FileFormat,
    duration: Duration,
    genre: String,
    rating: i8,
}

pub fn create_db() -> Result<(), rusqlite::Error> {
    let path = "./music_database.db3";
    Connection::open(path)?;
    Ok(())
}

pub fn find_all_music(target_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut i = 0;
    let mut x = 0;
    for entry in WalkDir::new(target_path).follow_links(true) {
        let target_file = entry?;
        let is_file = fs::metadata(target_file.path())?.is_file();
        x += 1; // Count the total of all things scanned

        // Ensure the target is a file and not a directory, if it isn't, skip this loop
        if !is_file { continue }

        let format = FileFormat::from_file(target_file.path())?;
        let extension =  target_file.path().extension().expect("Could not find file extension");

        if format.kind() == Kind::Audio {
            println!("{:?}", format.kind());
            println!("{}", target_file.path().display());
            i += 1; // Count the number of files, temporary
        } else if extension == "cue" {
            if let Ok(ret) = fs::read_to_string(target_file.path()) {
                let contents = ret.to_string();
                let cuesheet = CD::parse(contents)?;
                println!("{}", cuesheet.get_track_count());
                i += cuesheet.get_track_count();
            }
        }

    }

    println!("{}", i);
    println!("{}", x);

    Ok(())
}
