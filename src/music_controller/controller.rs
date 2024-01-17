//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions

use crate::{
    music_player::Player,
    music_storage::library::Song,
    config::config::Config
};

struct Queue {
    player: Player,
    name: String,
    songs: Vec<Song>,
}

pub struct Controller {
    queues: Vec<Queue>,
    config: Config,
}

impl Controller {
    // more stuff to come
}
