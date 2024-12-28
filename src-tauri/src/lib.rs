use std::{fs, path::PathBuf, str::FromStr, thread::spawn};

use commands::{add_song_to_queue, play_now};
use crossbeam::channel::{unbounded, Receiver, Sender};
use dmp_core::{config::{Config, ConfigLibrary}, music_controller::controller::{Controller, ControllerHandle, LibraryResponse}, music_storage::library::MusicLibrary};
use tauri::{http::Response, Emitter, Manager, State, WebviewWindowBuilder, Wry};
use uuid::Uuid;

use crate::wrappers::{get_library, play, pause, prev, set_volume, get_song, next, get_queue, import_playlist, get_playlist, get_playlists, remove_from_queue};

pub mod wrappers;
pub mod commands;

const DEFAULT_IMAGE: &[u8] = include_bytes!("../icons/icon.png");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (rx, tx) = unbounded::<Config>();
    let (lib_rx, lib_tx) = unbounded::<Option<PathBuf>>();
    let (handle_rx, handle_tx) = unbounded::<ControllerHandle>();

    let controller_thread = spawn(move || {
        let mut config = { tx.recv().unwrap() } ;
        let scan_path = { lib_tx.recv().unwrap() };
        let _temp_config = ConfigLibrary::default();
        let _lib = config.libraries.get_default().unwrap_or(&_temp_config);

        let save_path = if _lib.path == PathBuf::default() {
            scan_path.as_ref().unwrap().clone().canonicalize().unwrap().join("library.dlib")
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

        let (handle, input) = ControllerHandle::new(
            library,
            std::sync::Arc::new(std::sync::RwLock::new(config))
        );

        handle_rx.send(handle).unwrap();

        let _controller = futures::executor::block_on(Controller::start(input)).unwrap();
    });
    let app = tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .invoke_handler(tauri::generate_handler![
        get_config,
        new_library_window,
        create_library,
        get_library,
        play,
        pause,
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
        remove_from_queue
    ]).manage(ConfigRx(rx))
    .manage(LibRx(lib_rx))
    .manage(HandleTx(handle_tx))
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
            controller.lib_mail.send(dmp_core::music_controller::controller::LibraryCommand::Song(Uuid::parse_str(query.as_str()).unwrap())).await.unwrap();
            let LibraryResponse::Song(song, _) = controller.lib_mail.recv().await.unwrap() else {
                return None
            };
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
        tauri::RunEvent::ExitRequested { api, .. } => {
            api.prevent_exit();
        }
        _ => {}
    });
    std::mem::drop(controller_thread)
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
async fn new_library_window(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    WebviewWindowBuilder::new(
        &app,
        "library_create",
        tauri::WebviewUrl::App(PathBuf::from_str("/src/create_library_window/index.html").unwrap())
    ).title("Create a Library")
    .focused(true)
    .maximizable(false)
    .build()
    .unwrap();

    Ok(())
}


#[tauri::command]
async fn create_library(
    app: tauri::AppHandle<Wry>,
    lib_rx: State<'_, LibRx>,
    handle_tx: State<'_, HandleTx>,
    window: tauri::Window<Wry>,
    path: String
) -> Result<(), String> {
    println!("{path}");
    let path = PathBuf::from(path.trim().trim_matches('"'));

    if !path.exists() {
        panic!("Path {} does not exist!", path.display())
    } else if !path.is_dir() {
        panic!("Path {} is not a directory!", path.display())
    }

    lib_rx.inner().0.send(Some(path)).unwrap();
    app.manage(handle_tx.inner().0.recv().unwrap());
    app.emit("library_loaded", ()).unwrap();

    window.close().unwrap();
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
