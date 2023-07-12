use std::path::PathBuf;
use music_controller::music_controller::MusicController;
use music_storage::music_db::find_all_music;
use music_storage::music_db::create_db;

mod music_storage;
mod music_controller;

fn main() {
    create_db().unwrap();
    let config_path = PathBuf::from("config.toml");
    let controller = MusicController::new(&config_path).unwrap();
    
    find_all_music(&controller.config, "/media/g2/Storage1/Backups/music/").unwrap();
}
