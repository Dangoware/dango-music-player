use dango_core::{
    music_storage::music_db::{URI, Service},
    music_player::Player,
};

fn main() {
    // Create a new player
    let mut player = Player::new();

    // Add a stream to be queued
    player.enqueue_next(
        URI::Remote(
            Service::InternetRadio,
            "https://stream.gensokyoradio.net/3".to_string()
        )
    );

    player.play();
    player.set_volume(0.4);

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
