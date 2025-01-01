#![allow(while_true)]

use std::{fs, path::PathBuf, sync::Arc, thread::{scope, spawn}, time::Duration};

use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use dmp_core::{config::{Config, ConfigLibrary}, music_controller::{connections::ConnectionsInput, controller::{Controller, ControllerHandle, PlaybackInfo}}, music_storage::library::{MusicLibrary, Song}};
use futures::channel::oneshot;
use parking_lot::RwLock;
use tauri::{http::Response, Emitter, Manager, State, Wry};
use uuid::Uuid;
use wrappers::{_Song, stop};

use crate::wrappers::{get_library, play, pause, prev, set_volume, get_song, next, get_queue, import_playlist, get_playlist, get_playlists, remove_from_queue, seek};
use commands::{add_song_to_queue, play_now, display_album_art};


pub mod wrappers;
pub mod commands;

const DEFAULT_IMAGE: &[u8] = include_bytes!("../icons/icon.png");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (rx, tx) = unbounded::<Config>();
    let (lib_rx, lib_tx) = unbounded::<Option<PathBuf>>();
    let (handle_rx, handle_tx) = unbounded::<ControllerHandle>();
    let (playback_info_rx, playback_info_tx) = bounded(1);
    let (next_rx, next_tx) = bounded(1);

    let _controller_thread = spawn(move || {
        let mut config = { tx.recv().unwrap() } ;
        let scan_path = { lib_tx.recv().unwrap() };
        let _temp_config = ConfigLibrary::default();
        let _lib = config.libraries.get_default().unwrap_or(&_temp_config);

        let save_path = if _lib.path == PathBuf::default() {
            let p = scan_path.as_ref().unwrap().clone().canonicalize().unwrap();

            if cfg!(windows) {
                p.join("library_windows.dlib")
            } else if cfg!(unix) {
                p.join("library_unix.dlib")
            } else {
                p.join("library.dlib")
            }
        } else {
            _lib.path.clone()
        };
        println!("save_path: {}\nscan_path:{scan_path:?}", save_path.display());

        let mut library = MusicLibrary::init(
            save_path.clone(),
            _lib.uuid
        ).unwrap();

        let scan_path = scan_path.unwrap_or_else(|| config.libraries.get_default().unwrap().scan_folders.as_ref().unwrap()[0].clone());

        if config.libraries.get_default().is_err() {
            library.scan_folder(&scan_path).unwrap();
            config.push_library( ConfigLibrary::new(save_path.clone(), String::from("Library"), Some(vec![scan_path.clone()]), Some(library.uuid)));
        }
        if library.library.is_empty() {
            println!("library is empty");
        } else {
            config.write_file().unwrap();
        }
        println!("scan_path: {}", scan_path.display());

        library.save(save_path).unwrap();

        let (
            handle,
            input,
            playback_info,
            next_song_notification,
        ) = ControllerHandle::new(
            library,
            std::sync::Arc::new(RwLock::new(config)),
            Some(ConnectionsInput {
                discord_rpc_client_id: std::option_env!("DISCORD_CLIENT_ID").map(|id| id.parse::<u64>().unwrap()),
            }),
        );

        handle_rx.send(handle).unwrap();
        playback_info_rx.send(playback_info).unwrap();
        next_rx.send(next_song_notification).unwrap();

        let _controller = futures::executor::block_on(Controller::start(input)).unwrap();



    });
    let app = tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .invoke_handler(tauri::generate_handler![
        get_config,
        create_new_library,
        get_library,
        play,
        pause,
        stop,
        set_volume,
        next,
        prev,
        get_song,
        lib_already_created,
        get_queue,
        add_song_to_queue,
        play_now,
        import_playlist,
        get_playlist,
        get_playlists,
        remove_from_queue,
        display_album_art,
        seek,
    ]).manage(ConfigRx(rx))
    .manage(LibRx(lib_rx))
    .manage(HandleTx(handle_tx))
    .manage(tempfile::TempDir::new().unwrap())
    .setup(|app| {
        let _app = app.handle().clone();
        let app = _app.clone();

        std::thread::Builder::new()
        .name("PlaybackInfo handler".to_string())
        .spawn(move || {
            let mut _info: Arc<RwLock<PlaybackInfo>> = Arc::new(RwLock::new(PlaybackInfo::default()));
            let mut _now_playing: Arc<RwLock<Option<Song>>> = Arc::new(RwLock::new(None));

            scope(|s| {
                let info = _info.clone();
                s.spawn(|| {
                    let info = info;
                    let playback_info = playback_info_tx.recv().unwrap();
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
                    let next_song_notification = next_tx.recv().unwrap();
                    while true {
                        let song = next_song_notification.recv().unwrap();
                        app.emit("now_playing_change", _Song::from(&song)).unwrap();
                        app.emit("queue_updated", ()).unwrap();
                        app.emit("playing", ()).unwrap();
                        _ = now_playing.write().insert(song);
                    }
                });
            });
        }).unwrap();

        Ok(())
    })
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
        } else {futures::executor::block_on(async move {
            let controller = ctx.app_handle().state::<ControllerHandle>();
            let song = controller.lib_get_song(Uuid::parse_str(query.as_str()).unwrap()).await.0;
            Some(song.album_art(0).unwrap_or_else(|_| None).unwrap_or(DEFAULT_IMAGE.to_vec()))
        })};

        res.respond(
            Response::builder()
            .header("Origin", "*")
            .header("Content-Length", bytes.as_ref().unwrap_or(&vec![]).len())
            .status(200)
            .body(bytes.unwrap_or_default())
            .unwrap()
        );
    })
    .build(tauri::generate_context!())
    .expect("error while building tauri application");

    app
    .run(|_app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            // api.prevent_exit();
            //panic!("does this kill the player?")
        }
        _ => {}
    });
}

struct ConfigRx(Sender<Config>);

struct LibRx(Sender<Option<PathBuf>>);
struct HandleTx(Receiver<ControllerHandle>);


#[tauri::command]
async fn get_config(state: State<'_, ConfigRx>) -> Result<Config, String> {
    if let Some(dir) = directories::ProjectDirs::from("", "Dangoware", "dmp") {
        let path = dir.config_dir();
        fs::create_dir_all(path).or_else(|err| {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(err)
            }
        }).unwrap();

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

        state.inner().0.send(config.clone()).unwrap();

        Ok(config)
    } else {
        panic!("No config dir for DMP")
    }
}

#[tauri::command]
async fn create_new_library(
    app: tauri::AppHandle<Wry>,
    lib_rx: State<'_, LibRx>,
    handle_tx: State<'_, HandleTx>,
) -> Result<(), String> {
    let dir = rfd::AsyncFileDialog::new()
    .set_title("Pick a library path")
    .pick_folder()
    .await
    .unwrap();

    let path = dir.path().canonicalize().unwrap();
    println!("{}", path.display());

    if !path.exists() {
        panic!("Path {} does not exist!", path.display())
    } else if !path.is_dir() {
        panic!("Path {} is not a directory!", path.display())
    }

    lib_rx.inner().0.send(Some(path)).unwrap();
    app.manage(handle_tx.inner().0.recv().unwrap());
    app.emit("library_loaded", ()).unwrap();
    Ok(())
}

#[tauri::command]
    async fn lib_already_created(app: tauri::AppHandle<Wry>, lib_rx: State<'_, LibRx>, handle_tx: State<'_, HandleTx>) -> Result<(), String> {
    println!("lib already created");
    lib_rx.inner().0.send(None).unwrap();
    app.manage(handle_tx.inner().0.recv().unwrap());
    app.emit("library_loaded", ()).unwrap();
    Ok(())
}
