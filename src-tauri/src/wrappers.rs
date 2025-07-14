use std::{path::PathBuf, thread::spawn};

use crossbeam::channel::Sender;
use dmp_core::{
    music_controller::controller::{ControllerHandle, PlayerLocation},
    music_storage::{
        library::{Song, Tag},
        queue::QueueItemType,
    },
};
use itertools::Itertools;
use serde::Serialize;

use tauri::{AppHandle, Emitter, State, Wry};
use uuid::Uuid;

pub struct ArtworkRx(pub Sender<Vec<u8>>);

#[tauri::command]
pub async fn play(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<(), String> {
    match ctrl_handle.play().await {
        Ok(()) => {
            app.emit("playing", true).unwrap();
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn pause(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<(), String> {
    match ctrl_handle.pause().await {
        Ok(()) => {
            app.emit("playing", false).unwrap();
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn stop(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<(), String> {
    match ctrl_handle.stop().await {
        Ok(()) => {
            app.emit("stop", ()).unwrap();
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn set_volume(
    ctrl_handle: State<'_, ControllerHandle>,
    volume: String,
) -> Result<(), String> {
    let volume = volume.parse::<f32>().unwrap() / 100.0;
    ctrl_handle.set_volume(volume).await;
    Ok(())
}

#[tauri::command]
pub async fn get_volume(_ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn next(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<(), String> {
    let song = match ctrl_handle.next().await {
        Ok(s) => s,
        Err(e) => return Err(e.to_string()),
    };
    app.emit("now_playing_change", song).unwrap();
    app.emit("queue_updated", ()).unwrap();
    app.emit("playing", true).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn prev(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<(), String> {
    let song = match ctrl_handle.prev().await {
        Ok(s) => s,
        Err(e) => return Err(e.to_string()),
    };
    println!("prev");
    app.emit("now_playing_change", song).unwrap();
    app.emit("queue_updated", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn now_playing(_ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn get_queue(
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<Vec<(Song, PlayerLocation)>, String> {
    Ok(ctrl_handle
        .queue_get_all()
        .await
        .into_iter()
        .map(|item| {
            let QueueItemType::Song(song) = item.item else {
                unreachable!("There should be no albums in the queue right now")
            };
            (song.song, song.location)
        })
        .collect_vec())
}

#[tauri::command]
pub async fn remove_from_queue(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
    index: usize,
) -> Result<(), String> {
    match ctrl_handle.queue_remove(index).await {
        Ok(_) => {
            app.emit("queue_updated", ()).unwrap();
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn get_library(ctrl_handle: State<'_, ControllerHandle>) -> Result<Vec<Song>, String> {
    let songs = ctrl_handle.lib_get_all().await;
    Ok(songs)
}

#[tauri::command]
pub async fn get_playlist(
    ctrl_handle: State<'_, ControllerHandle>,
    uuid: Uuid,
) -> Result<Vec<Song>, String> {
    let playlist = match ctrl_handle.playlist_get(uuid).await {
        Ok(list) => list,
        Err(_) => todo!(),
    };
    println!(
        "Got Playlist {}, len {}",
        playlist.title,
        playlist.tracks.len()
    );
    Ok(playlist.tracks)
}

#[tauri::command]
pub async fn get_playlists(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<(), String> {
    let lists = ctrl_handle.playlist_get_all().await;
    spawn(move || {
        futures::executor::block_on(async {
            app.emit(
                "playlists_gotten",
                lists
                    .into_iter()
                    .map(|(uuid, name)| PlaylistPayload { uuid, name })
                    .collect_vec(),
            )
            .unwrap();
        })
    });
    Ok(())
}

#[tauri::command]
pub async fn import_playlist(
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<PlaylistPayload, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("m3u8 Playlist", &["m3u8", "m3u"])
        .set_title("Import a Playlist")
        .pick_file()
        .await
        .unwrap();

    let (uuid, name) = ctrl_handle
        .playlist_import_m3u(PathBuf::from(file.path()))
        .await
        .unwrap();
    ctrl_handle.lib_save().await;

    println!("Imported Playlist {name}");
    Ok(PlaylistPayload { uuid, name })
}

#[derive(Serialize, Clone)]
pub struct PlaylistPayload {
    uuid: Uuid,
    name: String,
}

#[tauri::command]
pub async fn get_song(
    ctrl_handle: State<'_, ControllerHandle>,
    uuid: Uuid,
) -> Result<Song, String> {
    let song = ctrl_handle.lib_get_song(uuid).await.0;
    println!(
        "got song {}",
        &song.tags.get(&Tag::Title).unwrap_or(&String::new())
    );
    Ok(song)
}

#[tauri::command]
pub async fn seek(ctrl_handle: State<'_, ControllerHandle>, time: i64) -> Result<(), String> {
    ctrl_handle.seek(time).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_song_to_playlist(
    ctrl_handle: State<'_, ControllerHandle>,
    song: Uuid,
    playlist: Uuid,
) -> Result<(), String> {
    Ok(ctrl_handle.playlist_add_song(playlist, song).await)
}

#[tauri::command]
pub async fn delete_playlist(
    ctrl_handle: State<'_, ControllerHandle>,
    uuid: Uuid,
) -> Result<(), String> {
    Ok(ctrl_handle.playlist_delete(uuid).await)
}

#[tauri::command]
pub async fn play_next_queue(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
    uuid: Uuid,
    location: PlayerLocation,
) -> Result<(), String> {
    let res = ctrl_handle
        .queue_play_next(uuid, location)
        .await
        .map_err(|e| e.to_string());
    app.emit("queue_updated", ()).unwrap();
    res
}

#[tauri::command]
pub async fn clear_queue(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
) -> Result<(), String> {
    let res = ctrl_handle.queue_clear().await.map_err(|e| e.to_string());
    _ = app.emit("queue_updated", ());
    res
}

#[tauri::command]
pub async fn queue_move_to(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
    index: usize,
) -> Result<(), String> {
    ctrl_handle
        .queue_move_to(index)
        .await
        .map_err(|e| e.to_string())?;

    match ctrl_handle.enqueue(0).await.map_err(|e| e.to_string()) {
        Ok(song) => {
            app.emit("queue_updated", ()).unwrap();
            app.emit("now_playing_change", song).unwrap();
            app.emit("playing", true).unwrap();
            Ok(())
        }
        Err(e) => Err(e),
    }
}
