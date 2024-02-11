//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use crossbeam_channel::{Sender, Receiver};
// use std::sync::mpsc;
use crossbeam_channel;
use gstreamer::format::Default;
use gstreamer::query::Uri;
use std::thread::{self, sleep, spawn};

use std::error::Error;
use crossbeam_channel::unbounded;
use rayon::iter::Rev;
use uuid::Uuid;

use crate::config;
use crate::music_storage::library::Tag;
use crate::music_storage::playlist::Playlist;
use crate::{
    music_player::Player,
    music_storage::{
        library::{MusicLibrary, Song}
    },
    config::config::Config,
};

struct Queue {
    player: Player,
    name: String,
    songs: Playlist,
}
impl Queue {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(
            Queue {
            player: Player::new()?,
            name: String::new(),
            songs: Playlist::new()
            }
        )
    }
}

pub struct Controller {
    // queues: Vec<Queue>,
    config: Arc<RwLock<Config>>,
    // library: MusicLibrary,
    controller_mail: MailMan<ControllerCommand, ControllerResponse>,
    db_mail: MailMan<DatabaseCommand, DatabaseResponse>,
    queue_mail: Vec<MailMan<QueueCommand, QueueResponse>>,
}
#[derive(Debug)]

pub enum ControllerCommand {
    Default,
    Test
}
#[derive(Debug)]

enum ControllerResponse {
    Empty,
    QueueMailMan(MailMan<QueueCommand, QueueResponse>),

}
#[derive(Debug)]

pub enum DatabaseCommand {
    Default,
    Test,
    GetSongs,

}
#[derive(Debug)]

enum DatabaseResponse {
    Empty,
    Songs(Vec<Song>),
}
#[derive(Debug)]
enum QueueCommand {
    Default,
    Test,
    Play,
    Pause,
}
#[derive(Debug)]
enum QueueResponse {
    Default,
    Test,
}

#[derive(Debug)]

struct MailMan<T, U> {
    pub tx: Sender<T>,
    rx: Receiver<U>
}
impl<T> MailMan<T, T> {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<T>();
        MailMan { tx, rx }
    }
}
impl<T, U> MailMan<T, U> {
    pub fn double() -> (MailMan<T, U>, MailMan<U, T>) {
        let (tx, rx) = unbounded::<T>();
        let (tx1, rx1) = unbounded::<U>();

        (
            MailMan { tx, rx: rx1 },
            MailMan { tx: tx1, rx }
        )
    }

    pub fn send(&self, mail: T) -> Result<(), Box<dyn Error>> {
        &self.tx.send(mail).unwrap();
        Ok(())
    }

    pub fn recv(&self) -> Result<U, Box<dyn Error>> {
        let u = self.rx.recv().unwrap();
        Ok(u)
    }
}

#[allow(unused_variables)]
impl Controller {
    pub fn start(config: PathBuf) -> Result<Self, Box<dyn Error>> {
        let config = Config::read_file(config)?;
        let uuid = config.libraries.get_default()?.uuid;

        let config = Arc::new(RwLock::from(config));
        let lib = MusicLibrary::init(config.clone(), uuid)?;

        let (out_thread_controller, in_thread) = MailMan::double();
        let monitor_thread = spawn(move || {
            use ControllerCommand::*;
            loop {
                let command = in_thread.recv().unwrap();

                match command {
                    Default => (),
                    Test => {
                        in_thread.send(ControllerResponse::Empty).unwrap();
                    },
                }
            }
        });


        let (out_thread_db, in_thread) = MailMan::double();
        let db_monitor = spawn(move || {
            use DatabaseCommand::*;
            loop {
                let command = in_thread.recv().unwrap();

                match command {
                    Default => {},
                    Test => {
                        in_thread.send(DatabaseResponse::Empty).unwrap();
                    },
                    GetSongs => {
                        let songs = lib.query_tracks(&String::from(""), &(vec![Tag::Title]), &(vec![Tag::Title])).unwrap().iter().cloned().cloned().collect();
                        in_thread.send(DatabaseResponse::Songs(songs)).unwrap();
                    },

                }
            }
        });



        Ok(
            Controller {
                // queues: Vec::new(),
                config,
                controller_mail: out_thread_controller,
                db_mail: out_thread_db,
                queue_mail: Vec::new(),
            }
        )
    }
    fn get_db_songs(&self) -> Vec<Song> {
        self.db_mail.send(DatabaseCommand::GetSongs);
        match self.db_mail.recv().unwrap() {
            DatabaseResponse::Songs(songs) => songs,
            _ => Vec::new()
        }

    }
    pub fn new_queue(&mut self) {
        let (out_thread_queue, in_thread) = MailMan::<QueueCommand, QueueResponse>::double();
        let queues_monitor =  spawn(move || {
            use QueueCommand::*;
            loop {
                let command = in_thread.recv().unwrap();
                match command {
                    Default => {},
                    Test => {},
                    Play => {},
                    Pause => {},
                }
            }
        });
        self.queue_mail.push(out_thread_queue);
    }
}

#[test]
fn name() {
    let a = Controller::start(PathBuf::from("test-config/config_test.json")).unwrap();
    // sleep(Duration::from_millis(5000));
    _ = a.controller_mail.send(ControllerCommand::Test);
    // dbg!(a.get_db_songs());
    // sleep(Duration::from_secs(6));
}