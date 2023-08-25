use std::path::{PathBuf, Path};
use music_controller::music_controller::MusicController;
use music_storage::music_db::{find_all_music, create_db, query, Tag, MusicObject};

use music_player::music_player::PlayerStatus;
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

    let mut lastfm = LastFM::new();
    
    let token = async {
        lastfm.get_auth_url().await
    };
    
    let now_token = task::block_on(token);
    println!("{:?}", now_token);
    
    std::thread::sleep(Duration::from_secs(10));
    
    task::block_on(lastfm.set_session());
    
    controller.config.lastfm = Some(lastfm);
    controller.config.save(&PathBuf::from("config.toml"));
  
    let song = String::from("choc.mp3");

    controller.open_song(song);
    
    controller.set_vol(0.10);
    
    thread::sleep(Duration::from_secs(5));
    
    controller.set_vol(3.0);

    thread::sleep(Duration::from_secs(2000));
}
