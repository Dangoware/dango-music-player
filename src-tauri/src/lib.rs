use std::{fs, path::PathBuf, str::FromStr, thread::spawn};

use crossbeam::channel::{unbounded, Receiver, Sender};
use dmp_core::{config::{Config, ConfigLibrary}, music_controller::controller::{Controller, ControllerHandle}, music_player::gstreamer::GStreamer, music_storage::library::MusicLibrary};
use tauri::{Manager, State, WebviewWindowBuilder, Wry};
use uuid::Uuid;

use crate::wrappers::{get_library, play, pause, prev, set_volume, get_song, next};

pub mod wrappers;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (rx, tx) = unbounded::<Config>();
    let (lib_rx, lib_tx) = unbounded::<PathBuf>();
    let (handle_rx, handle_tx) = unbounded::<ControllerHandle>();

    let t1 = spawn(move || {
        let mut config = { tx.recv().unwrap() } ;
        let scan_path = { lib_tx.recv().unwrap() };
        let save_path = config.libraries.library_folder.join("library.dlib");

        let mut library = MusicLibrary::init(
            save_path.clone(),
            if let Ok(lib) = config.libraries.get_default() {
                lib.uuid
            } else {
                Uuid::new_v4()
            }
        ).unwrap();
        library.scan_folder(&scan_path).unwrap();

        config.push_library( ConfigLibrary::new(save_path, String::from("Library"), Some(vec![scan_path.clone()])));
        // config.write_file().unwrap();


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

    ]).manage(ConfigRx(rx))
    .manage(LibRx(lib_rx))
    .manage(HandleTx(handle_tx))
    .build(tauri::generate_context!())
    .expect("error while building tauri application");

    app
    .run(|_app_handle, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            api.prevent_exit();
        }
        _ => {}
    });
    t1.join().unwrap();
}

struct ConfigRx(Sender<Config>);

struct LibRx(Sender<PathBuf>);
struct HandleTx(Receiver<ControllerHandle>);

#[tauri::command]
async fn get_config(state: State<'_, ConfigRx>) -> Result<Config, String> {
    if let Some(dir) = directories::ProjectDirs::from("", "Dangoware", "dmp") {
        let path = dir.config_dir();
        fs::create_dir(path).or_else(|err| {
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
    let path = PathBuf::from(path);

    if !path.exists() {
        panic!("Path {} does not exist!", path.display())
    } else if !path.is_dir() {
        panic!("Path {} is not a directory!", path.display())
    }

    lib_rx.inner().0.send(path).unwrap();
    app.manage(handle_tx.inner().0.recv().unwrap());

    window.close().unwrap();
    Ok(())
}
