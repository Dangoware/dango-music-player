use std::path::{PathBuf, Path};
use std::thread;
use std::time::Duration;

use dango_backend::music_controller::music_controller::MusicController;
use dango_backend::music_storage::music_db::{find_all_music, create_db, URI, Service};
use dango_backend::music_player::music_player::PlayerMessage;

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
    controller.set_vol(0.1);
    thread::sleep(Duration::from_secs(10));
    controller.song_control(PlayerMessage::SeekTo(3));
    println!("seeked!");

    thread::sleep(Duration::from_secs(2000));
}
