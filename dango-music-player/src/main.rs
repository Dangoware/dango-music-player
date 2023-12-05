use std::sync::{Arc, RwLock};

use chrono::Duration;
use dango_core::{
    music_controller::config::Config,
    music_storage::music_db::{MusicLibrary, Tag, Field},
    music_player::{Player, PlayerCmd},
};

fn main() {
    let config = Arc::new(RwLock::new(Config::default()));

    let mut library = MusicLibrary::init(config).unwrap();

    library.scan_folder("/media/g2/Storage4/Media-Files/Music/Albums/LiSA BEST -Day- & BEST -Way-/", &Config::default()).unwrap();

    let albums = library.query_albums(
        "LiVE is Smile Always ~LiTTLE DEVIL PARADE~ 2017.06.24 @SAITAMA SUPER ARENA"
    ).unwrap();

    // Create a new player
    let mut player = Player::new();
    player.set_volume(0.4);

    let mut skip = 0;
    for song in albums[0].tracks() {
        if skip != 0 {
            skip -= 1;
            continue;
        }

        // Add a stream to be played
        player.enqueue_next(&song.location);
        if player.is_paused() {
            player.play();
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
            match player.message_rx.try_recv() {
                Ok(msg) => match msg {
                    PlayerCmd::AboutToFinish => {
                        break
                    },
                    _ => ()
                },
                Err(_) => ()
            }

            match player.position() {
                Some(pos) => print!(
                    "{:2}:{:02}/{:2}:{:02} - {:?}\r",
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
