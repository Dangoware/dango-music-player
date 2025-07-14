#![allow(while_true)]
pub mod music_storage {
    pub mod library;
    pub mod music_collection;
    pub mod playlist;
    pub mod queue;
    mod utils;

    #[allow(dead_code)]
    pub mod db_reader;
}

pub mod music_controller {
    pub mod connections;
    pub mod controller;
    pub mod controller_handle;
    pub mod library_command;
    pub mod player_command;
    pub mod player_monitor;
    pub mod queue_command;
}

pub mod config;
