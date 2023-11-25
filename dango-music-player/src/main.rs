use std::path::Path;
use std::sync::{Arc, RwLock};

use dango_core::{
    music_controller::config::Config,
    music_storage::music_db::{MusicLibrary, Tag},
};

fn main() {
    let config = Arc::new(RwLock::new(Config::default()));

    let now = std::time::Instant::now();
    let mut library = MusicLibrary::init(config.clone()).unwrap();
    let time = now.elapsed().as_micros() as f32 / 1000.0;
    println!("Initialization took {}ms", time);

    if !Path::new("music_database").exists()
        || Path::new("music_database").metadata().unwrap().len() <= 21
    {
        let now = std::time::Instant::now();
        //let total = library.scan_folder("/home/g2/Music/Random Songs/KICM-3158.cue", &config.clone().read().unwrap()).unwrap();
        //let total = library.scan_folder("/home/g2/Downloads/Albums", &config.clone().read().unwrap()).unwrap();
        let total = library.scan_folder("/home/g2/Music/Albums/", &config.clone().read().unwrap()).unwrap();
        let time = now.elapsed().as_micros() as f32 / 1000.0;
        println!("{} songs in {}ms", total, time);
    }

    let lib_size = library.size();
    println!("{} songs total", lib_size);

    let now = std::time::Instant::now();
    let albums = library.albums();
    let time = now.elapsed().as_micros() as f32 / 1000.0;
    println!("{} albums total in {}ms", &albums.len(), time);

    let query_text = String::from("物語");

    println!("\nQuery Text: {query_text}");

    let now = std::time::Instant::now();
    let queried_songs = library
        .query_tracks(
            &query_text,
            &vec![
                Tag::Field("location".to_string()),
                Tag::Title,
                Tag::Album,
                Tag::AlbumArtist,
            ],
            &vec![
                Tag::Field("location".to_string()),
                Tag::Album,
                Tag::Disk,
                Tag::Track,
            ],
        )
        .unwrap();
    let time = now.elapsed().as_micros() as f32 / 1000.0;
    println!("{} songs queried in {}ms", queried_songs.len(), time);

    let now = std::time::Instant::now();
    let queried_albums = library.query_albums(&query_text).unwrap();
    let time = now.elapsed().as_micros() as f32 / 1000.0;
    println!("{} albums queried in {}ms", &queried_albums.len(), time);

    //std::process::exit(0);

    for album in &queried_albums {
        println!("{} songs in [{}] with album art {:?}:", album.len(), album.title(), album.cover());
        for disc in album.discs() {
            println!("   Disc {} ]-----", disc.0);
            for track in disc.1 {
                println!("     {: >2}) {}",
                         track.get_tag(&Tag::Track).unwrap_or(&String::from("")),
                         track.get_tag(&Tag::Title).unwrap_or(&String::from(""))
                );
            }
        }
        println!();
    }

    /*
    for song in queried_songs {
        println!("{}", song.get_tag(&Tag::Title).unwrap_or(&"".to_string()));
    }
    */
}
