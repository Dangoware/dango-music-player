pub mod music_tracker {
    pub mod music_tracker;
}

pub mod music_storage {
    pub mod music_db;
    pub mod playlist;
    mod utils;
}

pub mod music_player {
    pub mod music_output;
    pub mod music_player;
    pub mod music_resampler;
}

pub mod music_controller {
    pub mod config;
    pub mod init;
    pub mod music_controller;
}
