pub mod music_storage {
    pub mod library;
    pub mod playlist;
    mod utils;
    pub mod  music_collection;
    pub mod db_reader {
        pub mod foobar {
            pub mod reader;
        }
        pub mod musicbee {
            pub mod utils;
            pub mod reader;
        }
        pub mod xml {
            pub mod reader;
        }
        pub mod common;
        pub mod extern_library;
    }
}

pub mod music_controller {
    pub mod config;
    pub mod controller;
}

pub mod music_player;
