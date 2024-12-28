use dmp_core::music_controller::{controller::{ControllerHandle, LibraryResponse, PlayerCommand, PlayerLocation, PlayerResponse, QueueResponse}, queue::QueueSong};
use kushi::QueueItem;
use tauri::{AppHandle, Emitter, State, Wry};
use uuid::Uuid;

use crate::wrappers::_Song;



#[tauri::command]
pub async fn add_song_to_queue(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>, uuid: Uuid, location: PlayerLocation) -> Result<(), String> {
    dbg!(&location);
    let (song, _) = ctrl_handle.lib_get_song(uuid).await;
    match ctrl_handle.queue_append(QueueItem::from_item_type(kushi::QueueItemType::Single(QueueSong { song, location }))).await {
        Ok(()) => (),
        Err(e) => return Err(e.to_string())
    }
    app.emit("queue_updated", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn play_now(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>, uuid: Uuid, location: PlayerLocation) -> Result<(), String> {
    let song = match ctrl_handle.play_now(uuid, location).await {
        Ok(song) => song,
        Err(e) => return Err(e.to_string())
    };
    app.emit("queue_updated", ()).unwrap();
    app.emit("now_playing_change", _Song::from(&song)).unwrap();
    app.emit("playing", ()).unwrap();
    Ok(())
}