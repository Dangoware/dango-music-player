use std::sync::{Arc, RwLock};

use dango_core::{
    music_controller::config::Config,
    music_storage::music_db::{MusicLibrary, Tag},
};

fn main() {
    let config = Arc::new(RwLock::new(Config::default()));
    let mut library = MusicLibrary::init(config.clone()).unwrap();


    let now = std::time::Instant::now();
    //let total = library.find_all_music("/home/g2/Music/Random Songs/KICM-3158.cue", &config.clone().read().unwrap()).unwrap();
    let total = library.find_all_music("/home/g2/Downloads/Albums", &config.clone().read().unwrap()).unwrap();
    //let total = library.find_all_music("/home/g2/Music/Albums/", &config.clone().read().unwrap()).unwrap();
    let time = now.elapsed().as_micros() as f32 / 1000.0;
    println!("{} songs in {}ms", total, time);

    let query_res = library.query(
        &String::from("KICM"),
        &vec![Tag::Title],
        true,
        &vec![Tag::Field(String::from("location"))]
    ).unwrap();

    for result in query_res {
        println!("Location: {}\nStart/End: {:?}/{:?}\nTitle: {}\nAlbum: {}\n",
            result.location.path_string(),
            result.location.start().unwrap_or(&std::time::Duration::from_secs(0)),
            result.location.end().unwrap_or(&std::time::Duration::from_secs(0)),
            result.get_tag(&Tag::Title).unwrap_or(&"".to_string()),
            result.get_tag(&Tag::Album).unwrap_or(&"".to_string())
        );
    }
}
