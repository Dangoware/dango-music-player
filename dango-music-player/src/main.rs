use std::{thread, path::{PathBuf, Path}};

use dango_core::{
    music_tracker::music_tracker::{DiscordRPC, DiscordRPCConfig, MusicTracker, LastFMConfig, LastFM},
    music_controller::music_controller::MusicController,
    music_storage::music_db::{MusicLibrary, URI, Song, Tag, normalize},
    music_player::music_player::{DecoderMessage, PlayerStatus}
};
use async_std::{fs::File, io, prelude::*, task};

use iced::{executor, widget::{Button, scrollable, row}, Length, window::{self, icon::from_file}};
use iced::widget::{button, column, container, progress_bar, text, Column, text_input, slider, ProgressBar};
use iced::{
    Alignment, Application, Command, Element, Settings,
    Theme, Font
};
use once_cell::sync::Lazy;

static INPUT_ID: Lazy<text_input::Id> = Lazy::new(text_input::Id::unique);

fn main() {
    let default_font = Font {
        family: iced::font::Family::Name("Noto Sans CJK JP"),
        monospaced: true,
        ..Default::default()
    };
    let _ = DMP::run(Settings {
            antialiasing: true,
            default_font,
            ..Default::default()
    });
}

#[derive(Debug, Clone)]
pub enum Message {
    PlayerMessage(DecoderMessage),
    SetVol(f32),
    Open(String),
    InputChanged(String),
    SliderChanged(i32),
}

struct DMP {
    controller: MusicController,
    inputval: String,
    result: String,
    play_time: f32,
    slider_value: i32
}

pub struct Slider {
    slider_value: i32,
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
                result: String::new(),
                play_time: 0.0,
                slider_value: 100
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Dango Music Player")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Open(_) => {
                let query = &self.inputval;
                let songs = self.controller.query_library(
                    &query,
                    vec![
                        Tag::Artist,
                        Tag::Album,
                        Tag::Key("AlbumArtist".to_string()),
                        Tag::Title,
                    ],
                    true,
                    vec![
                        Tag::Field("location".to_string()),
                        Tag::Artist,
                        Tag::Album,
                        Tag::Key("DiscNumber".to_string()),
                        Tag::Track
                    ]
                );
                let song = songs.unwrap()[0];
                self.controller.song_control(DecoderMessage::OpenSong(song.clone()));
            }
            Message::InputChanged(song) => {
                self.inputval = song;

                if self.inputval.len() == 0 {
                    return Command::none()
                }

                let query = &self.inputval;
                let now = std::time::Instant::now();
                let songs = self.controller.query_library(
                    &query,
                    vec![
                        Tag::Artist,
                        Tag::Album,
                        Tag::Title,
                    ],
                    true,
                    vec![
                        Tag::Field("location".to_string()),
                        Tag::Artist,
                        Tag::Album,
                        Tag::Key("DiscNumber".to_string()),
                    ]
                );
                let time = now.elapsed();
                let full_songs = songs.unwrap_or_default();
                println!("{: >4}: {:0<5}ms, {query}", full_songs.len(), time.as_micros() as f64 / 1000.0);
                if full_songs.len() >= 20 {
                    self.result = full_songs[0..20].into_iter().map(|x| {
                        let temp = String::new();
                        let mut temp = match x.get_tag(&Tag::Title) {
                            Some(string) => string.to_string(),
                            None => return temp
                        };
                        temp.push_str("\n");
                        temp
                    }).collect::<String>();
                } else {
                    self.result = full_songs[0..full_songs.len()].into_iter().map(|x| {
                        let temp = String::new();
                        let mut temp = match x.get_tag(&Tag::Title) {
                            Some(string) => string.to_string(),
                            None => return temp
                        };
                        temp.push_str("\n");
                        temp
                    }).collect::<String>();
                }
            }
            Message::PlayerMessage(message) => {
                self.controller.song_control(message)
            }
            Message::SetVol(vol) => {
                self.controller.set_vol(vol);
            }
            Message::SliderChanged(value) => {
                self.controller.set_vol(value as f32 / 100.0);
                self.slider_value = value;
            }
        };
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let value = self.slider_value;
        let h_slider = slider(0..=100, value, Message::SliderChanged);

        column![
            row![
                text_input("Song goes here!", &self.inputval)
                    .id(INPUT_ID.clone())
                    .on_input(Message::InputChanged)
                    .padding(15)
                    .size(30),
                button("Open Song!").height(70).on_press(Message::Open(self.inputval.clone()))
            ].spacing(12).padding(12),
            scrollable(text(format!("{}", &self.result)).size(20)).height(Length::Fill).width(Length::Fill),
            row![
                button("Play").on_press(Message::PlayerMessage(DecoderMessage::Play)),
                button("Pause").on_press(Message::PlayerMessage(DecoderMessage::Pause)),
                text(format!("Volume: {: >3}", (self.controller.get_vol() * 100.0) as i32)).size(24),
                container(h_slider).width(200).center_x().center_y(),
            ].spacing(12).padding(12),
            //text(format!("Status: {:?}", self.controller.get_current_song())),
            progress_bar(0.0..=1.0, self.play_time),
        ]
        .padding(12)
        .align_items(Alignment::Center)
        .into()
    }
}
