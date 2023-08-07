use std::path::{PathBuf, Path};
use music_controller::music_controller::MusicController;
use music_storage::music_db::{find_all_music, create_db, query, Tag, MusicObject};

use music_player::music_player::PlayerStatus;

use std::thread;
use std::time::Duration;

mod music_storage;
mod music_controller;
mod music_player;


fn main() {
    let config_path = PathBuf::from("config.toml");
    let mut controller = MusicController::new(&config_path).unwrap();

    if !Path::new("./music_database.db3").exists() {
        create_db().unwrap();
        find_all_music(&controller.config, "/media/g2/Storage1/Backups/music/").unwrap();
    }

    let custom_tag = &Tag::Custom{name: "".to_owned(), value: "".to_owned()};

    let search_tags = vec![
        &Tag::Title,
        &Tag::Album
    ];

    let sort_by = vec![
        &Tag::Favorited,
        &Tag::Plays,
        &Tag::Album,
        &Tag::TrackNum,
        &Tag::Title
    ];

    let query_result = query(
        &controller.config,
        &"%Daydream caf%".to_owned(),
        search_tags,
        sort_by
    );

    match &query_result.unwrap()[0] {
        MusicObject::Song(song) => controller.open_song(&PathBuf::from(song.path.clone()).into()),
        _ => return
    }

    thread::sleep(Duration::from_secs(2000));
}
