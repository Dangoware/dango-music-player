use std::collections::BTreeMap;

use chrono::{DateTime, Utc, serde::ts_milliseconds_option};
use crossbeam::channel::Sender;
use dmp_core::{music_controller::controller::{ControllerHandle, LibraryCommand, LibraryResponse, PlayerResponse}, music_storage::library::{BannedType, Song, URI}};
use serde::Serialize;
use tauri::{ipc::Response, AppHandle, Emitter, State, Wry};
use uuid::Uuid;

pub struct ArtworkRx(pub Sender<Vec<u8>>);

#[tauri::command]
pub async fn play(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::Play).await.unwrap();
    let PlayerResponse::Empty = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    Ok(())
}

#[tauri::command]
pub async fn pause(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::Pause).await.unwrap();
    let PlayerResponse::Empty = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    Ok(())
}

#[tauri::command]
pub async fn set_volume(ctrl_handle: State<'_, ControllerHandle>, volume: String) -> Result<(), String> {
    let volume = volume.parse::<f64>().unwrap() / 1000.0;
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::SetVolume(volume)).await.unwrap();
    let PlayerResponse::Empty = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    Ok(())
}

#[tauri::command]
pub async fn get_volume(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {

    Ok(())
}

#[tauri::command]
pub async fn next(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>, art_rx: State<'_, ArtworkRx>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::NextSong).await.unwrap();
    let PlayerResponse::NowPlaying(song) = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    let _song = _Song::from(&song);
    art_rx.0.send(song.album_art(0).unwrap()).unwrap();
    app.emit("now_playing_change", _song).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn prev(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::PrevSong).await.unwrap();
    let PlayerResponse::Empty = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    Ok(())
}

#[tauri::command]
pub async fn now_playing(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {

    Ok(())
}


//Grab Album art from custom protocol
#[derive(Serialize, Debug, Clone)]
pub struct _Song {
    pub location: Vec<URI>,
    pub uuid: Uuid,
    pub plays: i32,
    pub format: Option<String>,
    pub duration: String,
    #[serde(with = "ts_milliseconds_option")]
    pub last_played: Option<DateTime<Utc>>,
    #[serde(with = "ts_milliseconds_option")]
    pub date_added: Option<DateTime<Utc>>,
    #[serde(with = "ts_milliseconds_option")]
    pub date_modified: Option<DateTime<Utc>>,
    pub tags: BTreeMap<String, String>,
}

impl From<&Song> for _Song {
    fn from(value: &Song) -> Self {
        _Song {
            location: value.location.clone(),
            uuid: value.uuid.clone(),
            plays: value.plays.clone(),
            duration: value.duration.as_secs().to_string(),
            format: value.format.map(|format| format.to_string()),
            last_played: value.last_played,
            date_added: value.date_added,
            date_modified: value.date_modified,
            tags: value.tags.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
        }
    }
}

#[tauri::command]
pub async fn get_library(ctrl_handle: State<'_, ControllerHandle>) -> Result<Vec<_Song>, String> {
    ctrl_handle.lib_mail.send(LibraryCommand::AllSongs).await.unwrap();
    let LibraryResponse::AllSongs(songs) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };

    let _songs = songs.iter().map(|song| _Song::from(song)).collect::<Vec<_>>();

    Ok(_songs)
}

#[tauri::command]
pub async fn get_song(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.lib_mail.send(LibraryCommand::Song(Uuid::default())).await.unwrap();
    let LibraryResponse::Song(_) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };
    println!("got songs");
    Ok(())
}

#[derive(Serialize, Debug)]
pub struct NowPlaying {
    title: String,
    artist: String,
    album: String,
}