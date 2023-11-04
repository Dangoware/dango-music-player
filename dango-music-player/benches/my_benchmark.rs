use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dango_core::{
    music_controller::config::Config,
    music_storage::music_db::{MusicLibrary, Tag},
};
use std::path::Path;
use std::sync::{Arc, RwLock};

fn criterion_benchmark(c: &mut Criterion) {
    let config = Arc::new(RwLock::new(Config::default()));
    let mut library = MusicLibrary::init(config.clone()).unwrap();

    if Path::new("music_database").metadata().unwrap().len() <= 8 {
        let now = std::time::Instant::now();
        //let total = library.scan_folder("/home/g2/Music/Random Songs/KICM-3158.cue", &config.clone().read().unwrap()).unwrap();
        let total = library.scan_folder("/home/g2/Downloads/Albums", &config.clone().read().unwrap()).unwrap();
        //let total = library.scan_folder("/home/g2/Music/Albums/", &config.clone().read().unwrap()).unwrap();
        let time = now.elapsed().as_micros() as f32 / 1000.0;
        println!("{} songs in {}ms", total, time);
    }

    c.bench_function("album collect", |b| b.iter(|| library.albums()));
    c.bench_function("query tracks", |b| b.iter(|| library
        .query_tracks(
            &String::from("yuru"),
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
        )));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
