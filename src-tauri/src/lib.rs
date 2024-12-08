use std::{fs, io::Read, path::PathBuf, str::FromStr, thread::spawn, time::Duration};

use crossbeam::channel::{unbounded, Receiver, Sender};
use dmp_core::{config::{Config, ConfigLibrary}, music_controller::controller::{Controller, ControllerHandle}, music_player::gstreamer::GStreamer, music_storage::library::{AlbumArt, MusicLibrary}};
use tauri::{http::Response, Manager, State, Url, WebviewWindowBuilder, Wry};
use uuid::Uuid;
use wrappers::ArtworkRx;

use crate::wrappers::{get_library, play, pause, prev, set_volume, get_song, next};

pub mod wrappers;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (rx, tx) = unbounded::<Config>();
    let (lib_rx, lib_tx) = unbounded::<Option<PathBuf>>();
    let (handle_rx, handle_tx) = unbounded::<ControllerHandle>();
    let (art_rx, art_tx) = unbounded::<Vec<u8>>();

    let controller_thread = spawn(move || {
        let mut config = { tx.recv().unwrap() } ;
        let scan_path = { lib_tx.recv().unwrap() };
        let save_path = dbg!(config.libraries.library_folder.join("library.dlib"));

        let mut library = MusicLibrary::init(
            save_path.clone(),
            if let Ok(lib) = config.libraries.get_default() {
                lib.uuid
            } else {
                Uuid::new_v4()
            }
        ).unwrap();

        let scan_path = scan_path.unwrap_or_else(|| config.libraries.get_default().unwrap().scan_folders.as_ref().unwrap()[0].clone());
        library.scan_folder(&scan_path).unwrap();

        if config.libraries.get_default().is_err() {
            config.push_library( ConfigLibrary::new(save_path, String::from("Library"), Some(vec![scan_path.clone()])));
        }
        if library.library.is_empty() {
            println!("library is empty");
        } else {
            config.write_file().unwrap();
        }
        println!("scan_path: {}", scan_path.display());

        let (handle, input) = ControllerHandle::new(
            library,
            std::sync::Arc::new(std::sync::RwLock::new(config))
        );

        handle_rx.send(handle).unwrap();

        let controller = futures::executor::block_on(Controller::<GStreamer>::start(input)).unwrap();
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

    ]).manage(ConfigRx(rx))
    .manage(LibRx(lib_rx))
    .manage(HandleTx(handle_tx))
    .manage(ArtworkRx(art_rx))
    .register_asynchronous_uri_scheme_protocol("asset", move |_, req, res| {
        dbg!(req);
        let buf = art_tx.recv().unwrap_or_else(|_| Vec::new());
        res.respond(
            Response::builder()
        .header("Origin", "*")
        .header("Content-Length", buf.len())
        .status(200)
        .body(buf)
        .unwrap()
        );
        println!("res sent")
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
    // controller_thread.join().unwrap();
}

struct ConfigRx(Sender<Config>);

struct LibRx(Sender<Option<PathBuf>>);
struct HandleTx(Receiver<ControllerHandle>);


#[tauri::command]
async fn get_config(state: State<'_, ConfigRx>) -> Result<Config, String> {
    if let Some(dir) = directories::ProjectDirs::from("", "Dangoware", "dmp") {
        let path = dir.config_dir();
        // dbg!(&path);
        fs::create_dir_all(path).or_else(|err| {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(err)
            }
        }).unwrap();

        // dbg!(&dir);

        let config = if let Ok(c) = Config::read_file(PathBuf::from(path).join("config")) {
            c
        } else {
            let c = Config {
                path: PathBuf::from(path).join("config"),
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

    window.close().unwrap();
    Ok(())
}

#[tauri::command]
    async fn lib_already_created(app: tauri::AppHandle<Wry>, lib_rx: State<'_, LibRx>, handle_tx: State<'_, HandleTx>) -> Result<(), String> {
    println!("lib already created");
    lib_rx.inner().0.send(None);
    app.manage(handle_tx.inner().0.recv().unwrap());
    Ok(())
}
