use std::path::PathBuf;
use std::path::Path;
use music_controller::music_controller::MusicController;
use music_db::find_all_music;
use music_db::{create_db, add_to_db};
use rusqlite::Connection;

mod music_db;
mod music_controller;

fn main() {
    create_db().unwrap();
    let config_path = PathBuf::from("config.toml");
    let controller = MusicController::new(&config_path).unwrap();
    
    find_all_music(&controller.config, "/media/g2/Storage4/Media-Files/Music/Albums/").unwrap();
}
