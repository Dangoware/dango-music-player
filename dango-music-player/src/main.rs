use std::{thread, path::{PathBuf, Path}};

use dango_core::{
    music_tracker::music_tracker::{DiscordRPC, DiscordRPCConfig, MusicTracker, LastFMConfig, LastFM},
    music_controller::music_controller::MusicController,
    music_storage::music_db::{MusicLibrary, URI, Song, Tag, normalize},
    music_player::music_player::{DecoderMessage, PlayerStatus}
};
use async_std::{fs::File, io, prelude::*, task};

use iced::{executor, widget::Button};
use iced::widget::{button, column, container, progress_bar, text, Column, text_input, slider, ProgressBar};
use iced::{
    Alignment, Application, Command, Element, Length, Settings, Subscription,
    Theme,
};
use once_cell::sync::Lazy;

static INPUT_ID: Lazy<text_input::Id> = Lazy::new(text_input::Id::unique);

fn main() {
    let config = dango_core::music_controller::config::Config::default();

    let mut library = MusicLibrary::init(&config).unwrap();
    /*
    library.find_all_music("/media/g2/Storage4/Media-Files/Music/Albums/").unwrap();
    library.save(&config).unwrap();
    */

    let query = String::from("のんびり三人娘");
    println!("{}", normalize(&query));
    let samples = 1;

    let mut overall = 0;
    for _ in 0..samples {
        let now = std::time::SystemTime::now();
        let songs = library.query(
            &query,
            &vec![
                Tag::Artist,
                Tag::Album,
            ],
            &vec![
                Tag::Artist,
                Tag::Album,
                Tag::Key("DiscNumber".to_string()),
                Tag::Track
            ]
        );
        let time = now.elapsed().unwrap();
        println!("The query \"{query}\" returned {} songs in {:?}", songs.clone().unwrap_or_default().len(), time);
        overall += time.as_micros();
    }
    println!("Average time: {}ms", (overall as f64 / samples as f64) / 1000.0);
    println!("Total tracks: {}", library.library.len());

    /*
    for song in songs.unwrap() {
        println!(
            "{: >3}, {: >3} | {}: {:?}",
            song.get_tag(&Tag::Key("DiscNumber".to_string())).unwrap_or(&"".to_string()),
            song.get_tag(&Tag::Track).unwrap_or(&"".to_string()),
            song.get_tag(&Tag::Album).unwrap_or(&"".to_string()),
            song.get_tag(&Tag::Title)
        );
    }
    */
    //println!("{:?}", library.library);

    //DMP::run(Settings::default());
}

#[derive(Debug, Clone)]
pub enum Message {
    PlayerMessage(DecoderMessage),
    SetVol(f32),
    Open(String),
    InputChanged(String),
}

struct DMP {
    controller: MusicController,
    inputval: String,
}

impl Application for DMP {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();
    
    fn new(_flags: ()) -> (DMP, Command<Message>) {
        (
            DMP {
                controller: MusicController::new(&PathBuf::from("config.toml")).unwrap(),
                inputval: String::from("Song"),
            },
            Command::none(),
        )
    }
    
    fn title(&self) -> String {
        String::from("Dango Music Player")
    }
    
    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Open(song) => {
                let song = Song {
                    location: URI::Local(song),
                    plays: 0,
                    skips: 0,
                    rating: None,
                    play_time: std::time::Duration::from_secs(200),
                    favorited: false,
                    format: None,
                    last_played: None,
                    date_added: None,
                    duration: std::time::Duration::from_secs(20),
                    date_modified: None,
                    album_art: Vec::new(),
                    tags: vec![(Tag::Title, "Miku".to_string()), (Tag::Artist, "Anamanaguchi".to_string())],
                };
                self.controller.song_control(DecoderMessage::OpenSong(song));
            }
            Message::InputChanged(song) => {
                self.inputval = song;
            }
            Message::PlayerMessage(message) => {
                self.controller.song_control(message)
            }
            Message::SetVol(vol) => {
                self.controller.set_vol(vol);
            }
        };
        Command::none()
    }
    
    fn view(&self) -> Element<Message> {
        column![
            text_input("Song goes here!", &self.inputval)
                    .id(INPUT_ID.clone())
                    .on_input(Message::InputChanged)
                    .padding(15)
                    .size(30),
            button("Open Song!").on_press(Message::Open(self.inputval.clone())),
            text(format!("Song volume: {}", self.controller.get_vol())).size(60),
            button("Play").on_press(Message::PlayerMessage(DecoderMessage::Play)),
            button("Pause").on_press(Message::PlayerMessage(DecoderMessage::Pause)),
            button("VOL+").on_press(Message::SetVol(self.controller.get_vol() + 0.1)),
            button("VOL-").on_press(Message::SetVol(self.controller.get_vol() - 0.1)),
            text(format!("Status: {:?}", self.controller.get_current_song())),
            progress_bar(0.0..=1.0, self.controller.get_vol())
        ]
        .padding(12)
        .align_items(Alignment::Center)
        .into()
    }
}
