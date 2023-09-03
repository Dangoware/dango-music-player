use std::path::{PathBuf, Path};
use music_controller::music_controller::MusicController;
use music_storage::music_db::{find_all_music, create_db, query, Tag, MusicObject, URI, Service};

use music_player::music_player::{PlayerStatus, PlayerMessage};
use music_tracker::music_tracker::LastFM;
use async_std::task;
use std::thread;
use std::time::Duration;

mod music_storage;
mod music_controller;
mod music_player;
mod music_processor;
mod music_tracker;

fn main() {
    let config_path = PathBuf::from("config.toml");
    let mut controller = MusicController::new(&config_path).unwrap();

    if !Path::new("./music_database.db3").exists() {
        create_db().unwrap();
        find_all_music(&controller.config, "/media/g2/Storage1/Backups/music/").unwrap();
    }

    let song_local = URI::Local(String::from("choc.mp3"));
    controller.open_song(&song_local);
    controller.set_vol(1.0);
    thread::sleep(Duration::from_secs(5));
    controller.song_control(PlayerMessage::Stop);
    
    let song_remote = URI::Remote(Service::InternetRadio, String::from("https://stream.gensokyoradio.net/3"));
    controller.open_song(&song_remote);
    controller.set_vol(1.0);
    thread::sleep(Duration::from_secs(10));
    controller.song_control(PlayerMessage::SeekTo(3));
    println!("seeked!");

    thread::sleep(Duration::from_secs(2000));
}
