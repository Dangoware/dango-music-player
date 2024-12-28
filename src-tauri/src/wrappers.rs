use std::{collections::BTreeMap, path::PathBuf};

use chrono::{DateTime, Utc, serde::ts_milliseconds_option};
use crossbeam::channel::Sender;
use dmp_core::{music_controller::controller::{ControllerHandle, LibraryCommand, LibraryResponse, PlayerResponse, QueueCommand, QueueResponse}, music_storage::library::{Song, Tag, URI}};
use itertools::Itertools;
use kushi::QueueItemType;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State, Wry};
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
    app.emit("playing", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn pause(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::Pause).await.unwrap();
    let PlayerResponse::Empty = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    app.emit("paused", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn set_volume(ctrl_handle: State<'_, ControllerHandle>, volume: String) -> Result<(), String> {
    let volume = volume.parse::<f32>().unwrap() / 1000.0;
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
        return Ok(())
    };
    println!("next");
    app.emit("now_playing_change", _Song::from(&song)).unwrap();
    app.emit("queue_updated", ()).unwrap();
    app.emit("playing", ()).unwrap();
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
            format: value.format.clone().map(|format| format.to_string()),
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
    println!("getting library");
    let LibraryResponse::AllSongs(songs) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };

    let _songs = songs.iter().map(|song| _Song::from(song)).collect::<Vec<_>>();

    Ok(_songs)
}

#[tauri::command]
pub async fn get_playlist(ctrl_handle: State<'_, ControllerHandle>, uuid: Uuid) -> Result<Vec<_Song>, String> {
    ctrl_handle.lib_mail.send(LibraryCommand::ExternalPlaylist(uuid)).await.unwrap();
    let LibraryResponse::ExternalPlaylist(playlist) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };

    let songs = playlist.tracks.iter().map(|song| _Song::from(song)).collect::<Vec<_>>();
    println!("Got Playlist {}, len {}", playlist.title, playlist.tracks.len());
    Ok(songs)
}

#[tauri::command]
pub async fn get_playlists(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    println!("getting Playlists");
    ctrl_handle.lib_mail.send(LibraryCommand::Playlists).await.unwrap();
    let LibraryResponse::Playlists(lists) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!() };
    println!("gotten playlists");

    app.emit("playlists_gotten", lists.into_iter().map(|(uuid, name)| PlaylistPayload { uuid, name }).collect_vec()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn import_playlist(ctrl_handle: State<'_, ControllerHandle>) -> Result<PlaylistPayload, String> {
    let file = rfd::AsyncFileDialog::new()
    .add_filter("m3u8 Playlist", &["m3u8", "m3u"])
    .set_title("Import a Playlist")
    .pick_file()
    .await
    .unwrap();

    ctrl_handle.lib_mail.send(LibraryCommand::ImportM3UPlayList(PathBuf::from(file.path()))).await.unwrap();
    let LibraryResponse::ImportM3UPlayList(uuid, name) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };
    ctrl_handle.lib_mail.send(LibraryCommand::Save).await.unwrap();
    let LibraryResponse::Ok = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!() };
    println!("Imported Playlist {name}");
    Ok(PlaylistPayload {uuid, name})
}

#[derive(Serialize, Clone)]
pub struct PlaylistPayload {
    uuid: Uuid,
    name: String
}

#[tauri::command]
pub async fn get_song(ctrl_handle: State<'_, ControllerHandle>) -> Result<_Song, String> {
    ctrl_handle.lib_mail.send(LibraryCommand::Song(Uuid::default())).await.unwrap();
    let LibraryResponse::Song(song, _) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };
    println!("got song {}", &song.tags.get(&Tag::Title).unwrap_or(&String::new()));
    Ok(_Song::from(&song))
}
