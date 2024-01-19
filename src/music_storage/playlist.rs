use std::path::Path;
use crate::music_controller::config::Config;
use rusqlite::{params, Connection};

pub fn playlist_add(
    config: &Config,
    playlist_name: &str,
    song_paths: &Vec<&Path>
) {
    let db_connection = Connection::open(&*config.db_path).unwrap();

    for song_path in song_paths {
        db_connection.execute(
            "INSERT INTO playlists (
                playlist_name,
                song_path
            ) VALUES (
                ?1,
                ?2
            )",
            params![
                playlist_name,
                song_path.to_str().unwrap()
            ],
        ).unwrap();
    }
}
