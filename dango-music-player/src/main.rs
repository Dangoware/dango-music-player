use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use chrono::Duration;
use dango_core::{
    music_controller::config::Config,
    music_player::{Player, PlayerCmd, PlayerState},
    music_storage::{
        db_reader::extern_library::ExternalLibrary,
        db_reader::xml::reader::*,
        library::{Field, MusicLibrary, Service, Song, Tag, URI},
        music_collection::MusicCollection,
        playlist::{self, Playlist},
    },
};
use gstreamer::prelude::ElementExtManual;
use urlencoding::decode;

fn main() {
    let config = Arc::new(RwLock::new(Config::default()));

    let mut library = MusicLibrary::init(config).unwrap();

    let xml = XmlLibrary::from_file(&PathBuf::from(
        "F:\\Music\\Mp3\\Music Main\\iTunes Music Library.xml",
    ))
    .to_songs();

    for song in xml {
        _ = library.add_song(song);
    }

    let mut playlist: Playlist = Playlist::new();
    let songs = library
        .query_tracks(
            &String::from("pancake"),
            &vec![Tag::Title],
            &vec![
                Tag::Field("location".to_string()),
                Tag::Album,
                Tag::Disk,
                Tag::Track,
            ],
        )
        .unwrap();
    // Create a new player
    let mut player = Player::new();
    player.set_volume(0.04);
    _ = playlist.set_tracks(songs);
    playlist.get_index("You Make My Life 1UP");

    'outer_loop: for song in playlist.tracks() {
        // Add a stream to be played
        player.enqueue_next(&song.location);
        if player.is_paused() {
            player.play().unwrap();
        }

        let location = match song.get_field("location").unwrap() {
            Field::Location(loc) => loc.path(),
            _ => std::path::PathBuf::new(),
        };

        let loc_name = match location.file_name() {
            Some(name) => name.to_str().unwrap(),
            None => "",
        };

        println!(
            "{}: {} -- {}",
            song.get_tag(&Tag::Track).unwrap_or(&"".to_string()),
            song.get_tag(&Tag::Title).unwrap_or(&"".to_string()),
            loc_name
        );

        loop {
            match player
                .message_rx
                .recv_timeout(std::time::Duration::from_millis(100))
            {
                Ok(msg) => match msg {
                    PlayerCmd::AboutToFinish => break,
                    PlayerCmd::Eos => break 'outer_loop,
                    _ => (),
                },
                Err(_) => (),
            }

            match player.position() {
                Some(pos) => print!(
                    "{:2}:{:02}/{}:{:02} - {:?}\r",
                    pos.num_minutes() % 60,
                    pos.num_seconds() % 60,
                    player
                        .duration()
                        .unwrap_or(Duration::seconds(0))
                        .num_minutes()
                        % 60,
                    player
                        .duration()
                        .unwrap_or(Duration::seconds(0))
                        .num_seconds()
                        % 60,
                    player.state()
                ),
                None => (),
            }
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        println!();
    }

    /*
    let mut all_songs: Vec<&Song> = Vec::new();
    albums.iter_mut().for_each(|album| all_songs.append(&mut album.tracks()));

    let mut skip = 1;
    'outer_loop:
    for song in all_songs {
        if skip > 0 {
            skip -= 1;
            continue;
        }

        // Add a stream to be played
        player.enqueue_next(&song.location);
        if player.is_paused() {
            player.play().unwrap();
        }

        if skip == 0 {
            skip -= 1;
            player.seek_to(Duration::minutes(4) + Duration::seconds(0)).unwrap();
        }

        let location = match song.get_field("location").unwrap() {
            Field::Location(loc) => loc.path(),
            _ => std::path::PathBuf::new()
        };

        let loc_name = match location.file_name() {
            Some(name) => name.to_str().unwrap(),
            None => "",
        };

        println!(
            "{}: {} -- {}",
            song.get_tag(&Tag::Track).unwrap_or(&"".to_string()),
            song.get_tag(&Tag::Title).unwrap_or(&"".to_string()),
            loc_name
        );

        loop {
            match player.message_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(msg) => {
                    match msg {
                        PlayerCmd::AboutToFinish => break,
                        PlayerCmd::Eos => break 'outer_loop,
                        _ => ()
                    }
                },
                Err(_) => ()
            }

            match player.position() {
                Some(pos) => print!(
                    "{:2}:{:02}/{}:{:02} - {:?}\r",
                    pos.num_minutes() % 60,
                    pos.num_seconds() % 60,

                    player.duration().unwrap_or(Duration::seconds(0)).num_minutes() % 60,
                    player.duration().unwrap_or(Duration::seconds(0)).num_seconds() % 60,

                    player.state()
                ),
                None => ()
            }
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        println!();
    }
    */
}
