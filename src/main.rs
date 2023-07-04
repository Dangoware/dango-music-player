use std::path::PathBuf;

use music_controller::music_controller::MusicController;
use music_db::create_db;

mod music_db;
mod music_controller;

fn main() {
    create_db().unwrap();
    let config_path = PathBuf::from("config.toml");
    let controller = MusicController::new(&config_path).unwrap();
    let rows = controller.connection.execute("SELECT * FROM music_collection", ()).unwrap();
    
    println!("Rows affected: {}", rows);
}
