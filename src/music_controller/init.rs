use std::fs::File;
use std::path::Path;

pub fn init() {}

fn init_config() {
    let config_path = "./config.toml";

    if !Path::new(config_path).try_exists().unwrap() {
        File::create("./config.toml").unwrap();
    }
}

fn init_db() {}
