use dmp_core::music_controller::{controller::{ControllerHandle, LibraryResponse, PlayerCommand, PlayerLocation, PlayerResponse, QueueResponse}, queue::QueueSong};
use kushi::QueueItem;
use tauri::{AppHandle, Emitter, State, Wry};
use uuid::Uuid;

use crate::wrappers::_Song;



#[tauri::command]
pub async fn add_song_to_queue(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>, uuid: Uuid, location: PlayerLocation) -> Result<(), String> {
    ctrl_handle.lib_mail.send(dmp_core::music_controller::controller::LibraryCommand::Song(uuid)).await.unwrap();
    let LibraryResponse::Song(song) = ctrl_handle.lib_mail.recv().await.unwrap() else {
        unreachable!()
    };
    ctrl_handle.queue_mail.send(dmp_core::music_controller::controller::QueueCommand::Append(QueueItem::from_item_type(kushi::QueueItemType::Single(QueueSong { song, location })), true)).await.unwrap();
    let QueueResponse::Ok = ctrl_handle.queue_mail.recv().await.unwrap() else {
        panic!()
    };
    app.emit("queue_updated", ()).unwrap();
    Ok(())
}

#[tauri::command]
pub async fn play_now(app: AppHandle<Wry>, ctrl_handle: State<'_, ControllerHandle>, uuid: Uuid, location: PlayerLocation) -> Result<(), String> {
    ctrl_handle.player_mail.send(PlayerCommand::PlayNow(uuid, location)).await.unwrap();
    let PlayerResponse::NowPlaying(song) = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
    app.emit("queue_updated", ()).unwrap();
    app.emit("now_playing_change", _Song::from(&song)).unwrap();
    app.emit("playing", ()).unwrap();
    Ok(())
}