use std::{fs::OpenOptions, io::Write};
use dmp_core::{
    music_controller::{
        connections::LastFMAuth,
        controller::{ControllerHandle, PlayerLocation},
        queue::QueueSong,
    },
    music_storage::queue::{QueueItem, QueueItemType},
};
use tauri::{AppHandle, Emitter, State, Wry};
use tempfile::TempDir;
use uuid::Uuid;

use crate::{wrappers::_Song, LAST_FM_API_KEY, LAST_FM_API_SECRET};

#[tauri::command]
pub async fn add_song_to_queue(
    app: AppHandle<Wry>,
    ctrl_handle: State<'_, ControllerHandle>,
    uuid: Uuid,
    location: PlayerLocation,
) -> Result<(), String> {
    dbg!(&location);
    let (song, _) = ctrl_handle.lib_get_song(uuid).await;
    match ctrl_handle
        .queue_append(QueueItem::from_item_type(QueueItemType::Single(
            QueueSong { song, location },
        )))
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
    app.emit("now_playing_change", _Song::from(&song)).unwrap();
    app.emit("playing", true).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn display_album_art(
    ctrl_handle: State<'_, ControllerHandle>,
    temp_dir: State<'_, TempDir>,
    uuid: Uuid,
) -> Result<(), String> {
    match ctrl_handle.lib_get_song(uuid.clone()).await.0.album_art(0) {
        Ok(art) => {
            let mut art = art.unwrap();
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
                file.write_all(&mut art).unwrap();
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