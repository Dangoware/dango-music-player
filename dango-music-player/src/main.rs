use std::sync::{Arc, RwLock};
use std::path::Path;

use dango_core::{
    music_controller::config::Config,
    music_storage::music_db::{MusicLibrary, Tag},
};

fn main() {
    let config = Arc::new(RwLock::new(Config::default()));
    let mut library = MusicLibrary::init(config.clone()).unwrap();

    if Path::new("music_database").metadata().unwrap().len() <= 8 {
        let now = std::time::Instant::now();
        //let total = library.scan_folder("/home/g2/Music/Random Songs/KICM-3158.cue", &config.clone().read().unwrap()).unwrap();
        //let total = library.scan_folder("/home/g2/Downloads/Albums", &config.clone().read().unwrap()).unwrap();
        let total = library.scan_folder("/home/g2/Music/Albums/", &config.clone().read().unwrap()).unwrap();
        let time = now.elapsed().as_micros() as f32 / 1000.0;
        println!("{} songs in {}ms", total, time);
    }

    let now = std::time::Instant::now();
    let query_res = library
        .query(
            &String::from(""),
            &vec![
                Tag::Field("location".to_string()),
                Tag::Title,
                Tag::Album,
                Tag::AlbumArtist
            ],
            &vec![
                Tag::Field("location".to_string()),
                Tag::Album,
                Tag::Disk,
                Tag::Track
            ],
        )
        .unwrap();
    let time = now.elapsed().as_micros() as f32 / 1000.0;
    println!("{} songs in {}ms", query_res.len(), time);

    let now = std::time::Instant::now();
    let albums = library.albums().unwrap();
    let time = now.elapsed().as_micros() as f32 / 1000.0;
    println!("{} albums in {}ms", albums.len(), time);

    println!("\n\nALBUMS ] ----------------\n");

    for album in albums {
        println!("{}", album.title);
    }
}
