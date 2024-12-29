use std::{collections::BTreeMap, path::PathBuf};

use chrono::{DateTime, Utc, serde::ts_milliseconds_option};
use crossbeam::channel::Sender;
use dmp_core::{music_controller::controller::{ControllerHandle, LibraryCommand, LibraryResponse, PlayerLocation, PlayerResponse, QueueCommand, QueueResponse}, music_storage::library::{Song, Tag, URI}};
use itertools::Itertools;
use kushi::QueueItemType;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State, Wry};
use uuid::Uuid;

pub struct ArtworkRx(pub Sender<Vec<u8>>);

#[tauri::command]
pub async fn play(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    match ctrl_handle.play().await {
        Ok(()) => {
            app.emit("playing", ()).unwrap();
            Ok(())
        },
        Err(e) => Err(e.to_string())
    }
}

#[tauri::command]
pub async fn pause(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    match ctrl_handle.pause().await {
        Ok(()) => {
            app.emit("paused", ()).unwrap();
            Ok(())
        }
        Err(e) => Err(e.to_string())
    }

}

#[tauri::command]
pub async fn set_volume(ctrl_handle: State<'_, ControllerHandle>, volume: String) -> Result<(), String> {
    let volume = volume.parse::<f32>().unwrap() / 100.0;
    ctrl_handle.set_volume(volume).await;
    Ok(())
}

#[tauri::command]
pub async fn get_volume(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn next(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    let song = match ctrl_handle.next().await {
        Ok(s) => s,
        Err(e) => return Err(e.to_string())
    };
    app.emit("now_playing_change", _Song::from(&song)).unwrap();
    app.emit("queue_updated", ()).unwrap();
    app.emit("playing", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn prev(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    let song = match ctrl_handle.prev().await {
        Ok(s) => s,
        Err(e) => return Err(e.to_string())
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
pub async fn get_queue(ctrl_handle: State<'_, ControllerHandle>) -> Result<Vec<(_Song, PlayerLocation)>, String> {
    Ok(
        ctrl_handle
            .queue_get_all()
            .await
            .into_iter()
            .map(|item| {
                let QueueItemType::Single(song) = item.item else { unreachable!("There should be no albums in the queue right now") };
                (_Song::from(&song.song), song.location)
            }
        ).collect_vec()
    )
}

#[tauri::command]
pub async fn remove_from_queue(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>, index: usize) -> Result<(), String> {
    match ctrl_handle.queue_remove(index).await {
        Ok(_) => {
            app.emit("queue_updated", ()).unwrap();
            Ok(())
        }
        Err(e) => Err(e.to_string())
    }
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
    let songs = ctrl_handle
        .lib_get_all()
        .await
        .iter()
        .map(|song| _Song::from(song))
        .collect_vec();
    Ok(songs)
}

#[tauri::command]
pub async fn get_playlist(ctrl_handle: State<'_, ControllerHandle>, uuid: Uuid) -> Result<Vec<_Song>, String> {
    let playlist = match ctrl_handle.playlist_get(uuid).await {
        Ok(list) => list,
        Err(_) => todo!()
    };

    let songs = playlist.tracks.iter().map(|song| _Song::from(song)).collect::<Vec<_>>();
    println!("Got Playlist {}, len {}", playlist.title, playlist.tracks.len());
    Ok(songs)
}

#[tauri::command]
pub async fn get_playlists(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    let lists = ctrl_handle.playlist_get_all().await;
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

    let (uuid, name) = ctrl_handle.playlist_import_m3u(PathBuf::from(file.path())).await.unwrap();
    ctrl_handle.lib_save().await;

    println!("Imported Playlist {name}");
    Ok(PlaylistPayload {uuid, name})
}

#[derive(Serialize, Clone)]
pub struct PlaylistPayload {
    uuid: Uuid,
    name: String
}

#[tauri::command]
pub async fn get_song(ctrl_handle: State<'_, ControllerHandle>, uuid: Uuid) -> Result<_Song, String> {
    let song = ctrl_handle.lib_get_song(uuid).await.0;
    println!("got song {}", &song.tags.get(&Tag::Title).unwrap_or(&String::new()));
    Ok(_Song::from(&song))
}
