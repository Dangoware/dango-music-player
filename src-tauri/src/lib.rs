#![allow(while_true)]

use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    thread::{scope, spawn},
    time::Duration,
};

use config::{close_window, get_config, open_config_window, save_config};
use crossbeam::channel::bounded;
use dmp_core::{
    config::{Config, ConfigLibrary},
    music_controller::{
        connections::LastFMAuth,
        controller::{Controller, ControllerHandle, PlaybackInfo},
    },
    music_storage::library::{MusicLibrary, Song},
};
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter, Manager, http::Response};
use uuid::Uuid;
use wrappers::stop;

use crate::wrappers::{
    add_song_to_playlist, clear_queue, delete_playlist, get_library, get_playlist, get_playlists,
    get_queue, get_song, import_playlist, next, pause, play, play_next_queue, prev, queue_move_to,
    remove_from_queue, seek, set_volume,
};
use commands::{
    add_song_to_queue, display_album_art, last_fm_init_auth, play_now, remove_from_lib_playlist,
};

pub mod commands;
pub mod config;
pub mod wrappers;

const DEFAULT_IMAGE: &[u8] = include_bytes!("../icons/icon.png");

const DISCORD_CLIENT_ID: u64 = 1198868728243290152;
const LAST_FM_API_KEY: &str = env!("LAST_FM_API_KEY", "None");
const LAST_FM_API_SECRET: &str = env!("LAST_FM_API_SECRET", "None");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (sync_rx, sync_tx) = bounded::<()>(1);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_library,
            play,
            pause,
            stop,
            set_volume,
            next,
            prev,
            get_song,
            get_queue,
            add_song_to_queue,
            play_now,
            import_playlist,
            get_playlist,
            get_playlists,
            remove_from_queue,
            display_album_art,
            seek,
            last_fm_init_auth,
            open_config_window,
            get_config,
            save_config,
            close_window,
            start_controller,
            add_song_to_playlist,
            delete_playlist,
            play_next_queue,
            clear_queue,
            remove_from_lib_playlist,
            queue_move_to,
            // test_menu,
        ])
        .manage(tempfile::TempDir::new().unwrap())
        .manage(sync_rx)
        .register_asynchronous_uri_scheme_protocol("asset", move |ctx, req, res| {
            let query = req
                .clone()
                .uri()
                .clone()
                .into_parts()
                .path_and_query
                .unwrap()
                .query()
                .unwrap()
                .to_string();

            let bytes = if query.as_str() == "default" {
                Some(DEFAULT_IMAGE.to_vec())
            } else {
                futures::executor::block_on(async move {
                    let controller = ctx.app_handle().state::<ControllerHandle>();
                    let song = controller
                        .lib_get_song(Uuid::parse_str(query.as_str()).unwrap())
                        .await
                        .0;
                    Some(
                        song.album_art(0)
                            .unwrap_or_else(|_| None)
                            .unwrap_or(DEFAULT_IMAGE.to_vec()),
                    )
                })
            };

            res.respond(
                Response::builder()
                    .header("Origin", "*")
                    .header("Content-Length", bytes.as_ref().unwrap_or(&vec![]).len())
                    .status(200)
                    .body(bytes.unwrap_or_default())
                    .unwrap(),
            );
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            // api.prevent_exit();
            //panic!("does this kill the player?")
        }
        _ => {}
    });
}

#[tauri::command]
fn start_controller(app: AppHandle) -> Result<(), String> {
    spawn(move || {
        let mut config = init_get_config().unwrap();

        let (lib_path, lib_uuid) = match config.libraries.get_default() {
            Ok(library) => (library.path.clone(), library.uuid),
            Err(_) => (create_new_library().unwrap(), Uuid::new_v4()),
        };
        let scan_path = lib_path.parent().unwrap();

        println!("lib_path: {}\nscan_path:{scan_path:?}", lib_path.display());

        let mut library = MusicLibrary::init(lib_path.clone(), lib_uuid).unwrap();

        if config.libraries.get_default().is_err() {
            library.scan_folder(&scan_path).unwrap();
            config.push_library(ConfigLibrary::new(
                lib_path.clone(),
                String::from("Library"),
                Some(vec![scan_path.into()]),
                Some(library.uuid),
            ));
        }
        if library.library.is_empty() {
            println!("library is empty");
        } else {
            config.write_file().unwrap();
        }
        library.save(lib_path.to_path_buf()).unwrap();
        app.emit("library_loaded", ()).unwrap();

        let last_fm_session = config.connections.last_fm_session.clone();
        let listenbrainz_token = config.connections.listenbrainz_token.clone();

        let (handle, input, playback_info, next_song_notification) =
            ControllerHandle::new(library, std::sync::Arc::new(RwLock::new(config)));

        handle.discord_rpc(DISCORD_CLIENT_ID);
        if let Some(token) = listenbrainz_token {
            handle.listenbrainz_scrobble_auth(token);
        } else {
            println!("No ListenBrainz token found");
        }
        if let Some(session) = last_fm_session {
            handle.last_fm_scrobble_auth(
                LAST_FM_API_KEY.to_string(),
                LAST_FM_API_SECRET.to_string(),
                LastFMAuth::Session(Some(session)),
            );
        }

        app.manage(handle);

        std::thread::Builder::new()
            .name("PlaybackInfo handler".to_string())
            .spawn(move || {
                let mut _info: Arc<RwLock<PlaybackInfo>> =
                    Arc::new(RwLock::new(PlaybackInfo::default()));
                let mut _now_playing: Arc<RwLock<Option<Song>>> = Arc::new(RwLock::new(None));

                scope(|s| {
                    let info = _info.clone();
                    s.spawn(|| {
                        let info = info;
                        let playback_info = playback_info;
                        while true {
                            let i = playback_info.take();
                            app.emit("playback_info", i.clone()).unwrap();
                            *info.write() = i;
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    });

                    let now_playing = _now_playing.clone();
                    s.spawn(|| {
                        let now_playing = now_playing;
                        let next_song_notification = next_song_notification;
                        while true {
                            let song = next_song_notification.recv().unwrap();
                            app.emit("now_playing_change", &song).unwrap();
                            app.emit("queue_updated", ()).unwrap();
                            app.emit("playing", true).unwrap();
                            _ = now_playing.write().insert(song);
                        }
                    });
                });
            })
            .unwrap();

        let _controller = futures::executor::block_on(Controller::start(input)).unwrap();
    });
    Ok(())
}

fn init_get_config() -> Result<Config, String> {
    if let Some(dir) = directories::ProjectDirs::from("", "Dangoware", "dmp") {
        let path = dir.config_dir();
        fs::create_dir_all(path)
            .or_else(|err| {
                if err.kind() == std::io::ErrorKind::AlreadyExists {
                    Ok(())
                } else {
                    Err(err)
                }
            })
            .unwrap();

        let config = if let Ok(mut c) = Config::read_file(PathBuf::from(path).join("config")) {
            if c.state_path == PathBuf::default() {
                c.state_path = PathBuf::from(path).join("state");
            }
            c
        } else {
            let c = Config {
                path: PathBuf::from(path).join("config"),
                state_path: PathBuf::from(path).join("state"),
                ..Default::default()
            };
            c.write_file().unwrap();
            c
        };

        println!("got config");
        Ok(config)
    } else {
        panic!("No config dir for DMP")
    }
}

fn create_new_library() -> Result<PathBuf, String> {
    let dir = rfd::FileDialog::new()
        .set_title("Pick a library path")
        .pick_folder()
        .unwrap();

    let path = dir.as_path().canonicalize().unwrap();
    println!("{}", path.display());

    if !path.exists() {
        panic!("Path {} does not exist!", path.display())
    } else if !path.is_dir() {
        panic!("Path {} is not a directory!", path.display())
    }

    let path = if cfg!(windows) {
        path.join("library_windows.dlib")
    } else if cfg!(unix) {
        path.join("library_unix.dlib")
    } else {
        path.join("library.dlib")
    };

    Ok(path)
}
