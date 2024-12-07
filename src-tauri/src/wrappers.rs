use dmp_core::{music_controller::controller::{ControllerHandle, LibraryCommand, LibraryResponse, PlayerResponse}, music_storage::library::Song};
use tauri::{ipc::Response, State};
use uuid::Uuid;

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
pub async fn set_volume(ctrl_handle: State<'_, ControllerHandle>, volume: f64) -> Result<(), String> {
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
pub async fn next(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.player_mail.send(dmp_core::music_controller::controller::PlayerCommand::NextSong).await.unwrap();
    let PlayerResponse::Empty = ctrl_handle.player_mail.recv().await.unwrap() else {
        unreachable!()
    };
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

#[tauri::command]
pub async fn get_library(ctrl_handle: State<'_, ControllerHandle>) -> Result<Response, String> {
    println!("getting songs");
    ctrl_handle.lib_mail.send(LibraryCommand::AllSongs).await.unwrap();
    let LibraryResponse::AllSongs(songs) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };
    println!("got songs");

    let mut buf = vec![];
    ciborium::into_writer(&songs, &mut buf);
    Ok(Response::new(buf))
}

#[tauri::command]
pub async fn get_song(ctrl_handle: State<'_, ControllerHandle>) -> Result<(), String> {
    ctrl_handle.lib_mail.send(LibraryCommand::Song(Uuid::default())).await.unwrap();
    let LibraryResponse::Song(_) = ctrl_handle.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };
    println!("got songs");
    Ok(())
}