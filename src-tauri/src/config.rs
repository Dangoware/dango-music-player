use std::path::PathBuf;

use dmp_core::{config::Config, music_controller::controller::ControllerHandle};
use tauri::{State, WebviewWindowBuilder, Window, Wry};

#[tauri::command]
pub async fn open_config_window(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    WebviewWindowBuilder::new(&app, "editdmpconfig", tauri::WebviewUrl::App(PathBuf::from("src/config/index.html")))
    .title("Edit Dango Music Player")
    .build()
    .unwrap();
    Ok(())
}

#[tauri::command]
pub async fn get_config(ctrl_handle: State<'_, ControllerHandle>) -> Result<Config, String> {
    Ok(ctrl_handle.config.read().clone())
}

#[tauri::command]
pub async fn save_config(ctrl_handle: State<'_, ControllerHandle>, config: Config) -> Result<(), String> {
    let config_original = ctrl_handle.config.read().clone();

    if config.connections.listenbrainz_token.as_ref().is_some_and(|t| Some(t) != config_original.connections.listenbrainz_token.as_ref()) {
        let token = config.connections.listenbrainz_token.clone().unwrap();
        ctrl_handle.listenbrainz_scrobble_auth(dbg!(token));
    }

    *ctrl_handle.config.write() = config;
    ctrl_handle.config.read().write_file().unwrap();
    Ok(())
}

#[tauri::command]
pub async fn close_window(window: Window<Wry>) -> Result<(), String> {
    window.close().unwrap();
    Ok(())
}