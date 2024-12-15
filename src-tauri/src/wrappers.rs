use std::collections::BTreeMap;

use chrono::{DateTime, Utc, serde::ts_milliseconds_option};
use crossbeam::channel::Sender;
use dmp_core::{music_controller::controller::{ControllerHandle, LibraryCommand, LibraryResponse, PlayerResponse, QueueCommand, QueueResponse}, music_storage::library::{BannedType, Song, URI}};
use itertools::Itertools;
use kushi::QueueItemType;
use serde::Serialize;
use tauri::{ipc::Response, AppHandle, Emitter, State, Wry};
use uuid::Uuid;

pub struct ArtworkRx(pub Sender<Vec<u8>>);

#[tauri::command]
pub async fn play(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::Play).await.unwrap();
    let res = ctrl_handle.player_mail.recv().await.unwrap();
    if let PlayerResponse::Empty = res {}
    else if let PlayerResponse::NowPlaying(song) = res {
        app.emit("now_playing_change", _Song::from(&song)).unwrap();
    } else {
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
pub async fn next(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::NextSong).await.unwrap();
    let PlayerResponse::NowPlaying(song) = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    println!("next");
    app.emit("now_playing_change", _Song::from(&song)).unwrap();
    app.emit("queue_updated", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn prev(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::PrevSong).await.unwrap();
    let PlayerResponse::NowPlaying(song) = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    println!("prev");
    app.emit("now_playing_change", _Song::from(&song)).unwrap();
    app.emit("queue_updated", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn now_playing(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {

    Ok(())
}

#[tauri::command]
pub async fn get_queue(ctrl_handle: State<'_, ControllerHandle>) -> Result<Vec<_Song>, String> {
    ctrl_handle.queue_mail.send(QueueCommand::Get).await.unwrap();
    let QueueResponse::GetAll(queue) = ctrl_handle.queue_mail.recv().await.unwrap() else {
        unreachable!()
    };
    Ok(queue.into_iter().map(|item| {
        let QueueItemType::Single(song) = item.item else { unreachable!("There should be no albums in the queue right now") };
        _Song::from(&song.song)
    }).collect_vec())
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