use std::{thread, path::PathBuf};

use dango_core::{music_tracker::music_tracker::{DiscordRPC, DiscordRPCConfig, MusicTracker, LastFMConfig, LastFM}, music_controller::music_controller::MusicController};
use async_std::{fs::File, io, prelude::*, task};

fn main() {
    let ctl = MusicController::new(&PathBuf::from("config.toml")).unwrap();

    let conf = DiscordRPCConfig {
        enabled: true,
        dango_client_id: 1144475145864499240,
        dango_icon: String::from("flat"),
    };
    let mut disc: Box<dyn MusicTracker> = Box::new(DiscordRPC::new(&conf));
    let song = String::from("Listening to ASM");
    let good = task::block_on(async {
        return disc.track_now(&song).await
    });
    println!("good: {:?}", good);
    
    let fm = LastFMConfig {
        enabled: true,
                dango_api_key: String::from("29a071e3113ab8ed36f069a2d3e20593"),
                auth_token: None,
                shared_secret: Some(String::from("5400c554430de5c5002d5e4bcc295b3d")),
                session_key: None,
    };
    
    let mut last = LastFM::new(&fm);
    
    let url = task::block_on(async {
        last.get_auth_url().await.unwrap()
    });
    
    println!("URL: {url}");
    
    thread::sleep_ms(10000); 
    
    let sk = task::block_on(async {
        last.set_session().await
    });
    
    let song = String::from("song");
    let track = task::block_on(async {
        last.track_now(&song).await
    });
    
    thread::sleep_ms(5000000);
}
