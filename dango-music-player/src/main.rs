use std::{thread, path::{PathBuf, Path}};

use dango_core::{music_tracker::music_tracker::{DiscordRPC, DiscordRPCConfig, MusicTracker, LastFMConfig, LastFM}, music_controller::music_controller::MusicController, music_storage::music_db::{URI, Song}, music_player::music_player::{DecoderMessage, PlayerStatus}};
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
    DMP::run(Settings::default());
    
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
                    path: URI::Local(song),
                    title:  Some(String::from("Miku")),
                    album:  None,
                    tracknum: None,
                    artist: Some(String::from("Anamanaguchi")),
                    date: None,
                    genre: None,
                    plays: None,
                    favorited: None,
                    format: None, // TODO: Make this a proper FileFormat eventually
                    duration: None,
                    custom_tags: None,
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
            _ => {}
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