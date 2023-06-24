
    use std::path::Path;
    use std::fs::File;

    pub fn init() {

    }

    fn init_config() {
        let config_path = "./config.toml";

        if !Path::new(config_path).try_exists().unwrap() {
            File::create("./config.toml");
        }
    }

    fn init_db() {

    }

