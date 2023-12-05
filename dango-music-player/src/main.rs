use std::sync::{Arc, RwLock};

use chrono::Duration;
use dango_core::{
    music_controller::config::Config,
    music_storage::music_db::{MusicLibrary, Tag, Field, Song},
    music_player::{Player, PlayerCmd},
};

fn main() {
    let config = Arc::new(RwLock::new(Config::default()));

    let mut library = MusicLibrary::init(config).unwrap();

    library.scan_folder("/media/g2/Storage4/Media-Files/Music/Albums/LiSA BEST -Day- & BEST -Way-/", &Config::default()).unwrap();

    let mut albums = library.query_albums(
        "みなみけ ただいま キャラクターソングアルバム みなきけのみなうた DISC1"
    ).unwrap();

    println!("{}", albums.len());

    // Create a new player
    let mut player = Player::new();
    player.set_volume(0.4);

    let mut all_songs: Vec<&Song> = Vec::new();
    albums.iter_mut().for_each(|album| all_songs.append(&mut album.tracks()));

    println!("{:#?}", all_songs[0]);

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
            player.seek_to(Duration::minutes(4) + Duration::seconds(15)).unwrap();
        }

        let location = match song.get_field("location").unwrap() {
            Field::Location(loc) => loc.path(),
            _ => std::path::PathBuf::new()
        };

        let loc_name = match location.file_name() {
            Some(name) => name.to_str().unwrap(),
            None => "",
        };

        println!("{} -- {}", song.get_tag(&Tag::Title).unwrap(), loc_name);

        loop {
            match player.message_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(msg) => match msg {
                    PlayerCmd::AboutToFinish => break,
                    PlayerCmd::Eos => break 'outer_loop,
                    _ => ()
                }
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
}
