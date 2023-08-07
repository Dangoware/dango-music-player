use std::path::{PathBuf, Path};
use music_controller::music_controller::MusicController;
use music_storage::music_db::{find_all_music, create_db, query, Tag, MusicObject};

mod music_storage;
mod music_controller;
mod music_player;


fn main() {
    let config_path = PathBuf::from("config.toml");
    let controller = MusicController::new(&config_path).unwrap();

    if !Path::new("./music_database.db3").exists() {
        create_db().unwrap();
        find_all_music(&controller.config, "/media/g2/Storage1/Backups/music/").unwrap();
    }

    let custom_tag = &Tag::Custom("my_custom_tag".to_owned());

    let search_tags = vec![
        &Tag::Title,
        custom_tag
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
        &"%girls%".to_owned(),
        search_tags,
        sort_by
    );

    match &query_result[0] {
        MusicObject::Song(song) => println!("{:#?}", song),
        _ => return
    };
}
