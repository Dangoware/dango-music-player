use dmp_core::{
    music_controller::{
        connections::LastFMAuth,
        controller::{ControllerHandle, PlayerLocation},
    },
    music_storage::queue::{QueueItem, QueueItemType},
};
use std::{fs::OpenOptions, io::Write};
use tauri::{AppHandle, Emitter, State, Wry};
use tempfile::TempDir;
use uuid::Uuid;

use crate::{LAST_FM_API_KEY, LAST_FM_API_SECRET};

#[tauri::command]
pub async fn add_song_to_queue(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
    uuid: Uuid,
    location: PlayerLocation,
) -> Result<(), String> {
    let (song, _) = ctrl_handle.lib_get_song(uuid).await;
    match ctrl_handle
        .queue_append(QueueItem {
            item: QueueItemType::Song(song),
            location,
        })
        .await
    {
        Ok(()) => (),
        Err(e) => return Err(e.to_string()),
    }
    app.emit("queue_updated", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn play_now(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
    uuid: Uuid,
    location: PlayerLocation,
) -> Result<(), String> {
    let song = match ctrl_handle.play_now(uuid, location).await {
        Ok(song) => song,
        Err(e) => return Err(e.to_string()),
    };
    app.emit("queue_updated", ()).unwrap();
    app.emit("now_playing_change", &song).unwrap();
    app.emit("playing", true).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn display_album_art(
    ctrl_handle: State<'_, ControllerHandle>,
    temp_dir: State<'_, TempDir>,
    uuid: Uuid,
) -> Result<(), String> {
    match ctrl_handle.lib_get_song(uuid).await.0.album_art(0) {
        Ok(art) => {
            let art = art.unwrap();
            let path = temp_dir.path().join(format!(
                "CoverArt_{uuid}.{}",
                file_format::FileFormat::from_bytes(&art).extension()
            ));
            if !path.exists() {
                // TODO: This can be optimised later
                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .read(true)
                    .open(path.clone())
                    .unwrap();
                file.write_all(&art).unwrap();
            }
            opener::open(path).unwrap();
        }
        Err(e) => return Err(e.to_string()),
    };
    Ok(())
}

#[tauri::command]
pub async fn last_fm_init_auth(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.last_fm_scrobble_auth(
        LAST_FM_API_KEY.to_string(),
        LAST_FM_API_SECRET.to_string(),
        LastFMAuth::Session(None),
    );
    Ok(())
}

#[tauri::command]
pub async fn remove_from_lib_playlist(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
    song: Uuid,
    location: PlayerLocation,
) -> Result<(), String> {
    match location {
        PlayerLocation::Library => {
            ctrl_handle.lib_remove_song(song).await;
            app.emit("library_loaded", ()).unwrap();
        }
        PlayerLocation::Playlist(uuid) => {
            ctrl_handle.playlist_remove_song(song, uuid).await;
        }
        _ => unimplemented!(),
    }

    Ok(())
}
