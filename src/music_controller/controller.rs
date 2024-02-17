//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions

use std::path::{Path, PathBuf};
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
use crate::music_storage::library::{Tag, URI};
use crate::music_storage::playlist::Playlist;
use crate::{
    music_player::Player,
    music_storage::library::{MusicLibrary, Song},
    config::config::Config,
};

struct Queue {
    player: Player,
    name: String,
    songs: Vec<Song>,
}
impl Queue {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(
            Queue {
            player: Player::new()?,
            name: String::new(),
            songs: Vec::new()
            }
        )
    }
    fn set_tracks(&mut self, tracks: Vec<Song>) {
        let mut tracks = tracks;
        self.songs.clear();
        self.songs.append(&mut tracks);
    }
}

pub struct Controller {
    // queues: Vec<Queue>,
    config: Arc<RwLock<Config>>,
    // library: MusicLibrary,
    controller_mail: MailMan<ControllerCmd, ControllerResponse>,
    db_mail: MailMan<DatabaseCmd, DatabaseResponse>,
    queue_mail: Vec<MailMan<QueueCmd, QueueResponse>>,
}
#[derive(Debug)]
pub enum ControllerCmd {
    Default,
    Test
}

#[derive(Debug)]
enum ControllerResponse {
    Empty,
    QueueMailMan(MailMan<QueueCmd, QueueResponse>),

}

#[derive(Debug)]
pub enum DatabaseCmd {
    Default,
    Test,
    GetSongs,
    QueryUuid(Uuid),
    QueryUuids(Vec<Uuid>),
    ReadFolder(String),

}

#[derive(Debug)]
enum DatabaseResponse {
    Empty,
    Song(Song),
    Songs(Vec<Song>),
}

#[derive(Debug)]
enum QueueCmd {
    Default,
    Test,
    Play,
    Pause,
    SetSongs(Vec<Song>),
    // SetLocation(URI),
    Enqueue(URI),
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
        self.tx.send(mail).unwrap();
        Ok(())
    }

    pub fn recv(&self) -> Result<U, Box<dyn Error>> {
        let u = self.rx.recv().unwrap();
        Ok(u)
    }
}

#[allow(unused_variables)]
impl Controller {
    pub fn start(config_path: String) -> Result<Self, Box<dyn Error>> {
        let config_path = PathBuf::from(config_path);
        let config = Config::read_file(config_path)?;
        let uuid = config.libraries.get_default()?.uuid;

        let config = Arc::new(RwLock::from(config));
        let mut lib = MusicLibrary::init(config.clone(), uuid)?;

        let (out_thread_controller, in_thread) = MailMan::double();
        let monitor_thread = spawn(move || {
            use ControllerCmd::*;
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
            use DatabaseCmd::*;
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
                    QueryUuid(uuid) => {
                        match lib.query_uuid(&uuid) {
                            Some(song) => in_thread.send(DatabaseResponse::Song(song.0.clone())).unwrap(),
                            None => in_thread.send(DatabaseResponse::Empty).unwrap(),
                        }
                    },
                    QueryUuids(uuids) => {
                        let mut vec = Vec::new();
                        for uuid in uuids {
                            match lib.query_uuid(&uuid) {
                                Some(song) => vec.push(song.0.clone()),
                                None => unimplemented!()
                            }
                        }
                        in_thread.send(DatabaseResponse::Songs(vec)).unwrap();
                    },
                    ReadFolder(folder) => {
                        lib.scan_folder(&folder).unwrap();
                        in_thread.send(DatabaseResponse::Empty).unwrap();
                    }

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
        self.db_mail.send(DatabaseCmd::GetSongs);
        match self.db_mail.recv().unwrap() {
            DatabaseResponse::Songs(songs) => songs,
            _ => Vec::new()
        }

    }

    pub fn new_queue(&mut self) {
        let (out_thread_queue, in_thread) = MailMan::<QueueCmd, QueueResponse>::double();
        let queues_monitor =  spawn(move || {
            use QueueCmd::*;
            let mut queue = Queue::new().unwrap();
            loop {
                let command = in_thread.recv().unwrap();
                match command {
                    Default => {},
                    Test => { in_thread.send(QueueResponse::Test).unwrap() },
                    Play => {
                        match queue.player.play() {
                            Ok(_) => in_thread.send(QueueResponse::Default).unwrap(),
                            Err(_) => unimplemented!()
                        };

                    },
                    Pause => {},
                    SetSongs(songs) => {
                        queue.set_tracks(songs);
                        in_thread.send(QueueResponse::Default).unwrap();
                    },
                    Enqueue(uri) => {
                        if uri.exists().unwrap() {
                            queue.player.enqueue_next(&uri);
                        }

                        // in_thread.send(QueueResponse::Default).unwrap();
                    }
                }
            }
        });
        self.queue_mail.push(out_thread_queue);
    }

    fn play(&self, index: usize) -> Result<(), Box<dyn Error>> {
        let mail = &self.queue_mail[index];
        mail.send(QueueCmd::Play)?;
        dbg!(mail.recv()?);
        Ok(())
    }

    fn set_songs(&self, index: usize, songs: Vec<Song>) -> Result<(), Box<dyn Error>> {
        let mail = &self.queue_mail[index];
        mail.send(QueueCmd::SetSongs(songs))?;
        dbg!(mail.recv()?);
        Ok(())
    }

    fn enqueue(&self, index: usize, uri: URI) -> Result<(), Box<dyn Error>> {
        let mail = &self.queue_mail[index];
        mail.send(QueueCmd::Enqueue(uri))?;
        // dbg!(mail.recv()?);
        Ok(())
    }
    fn scan_folder(&self, folder: String) -> Result<(), Box<dyn Error>> {
        let mail = &self.db_mail;
        mail.send(DatabaseCmd::ReadFolder(folder))?;
        dbg!(mail.recv()?);
        Ok(())
    }

}

#[test]
fn name() {
    let mut a = match Controller::start("test-config/config_test.json".to_string()) {
        Ok(c) => c,
        Err(e) => panic!("{e}")
    };
    sleep(Duration::from_millis(500));
    a.scan_folder("test-config/music/".to_string());
    a.new_queue();
    // a.new_queue();
    let songs = a.get_db_songs();
    a.enqueue(0, songs[4].location.clone());
    // a.enqueue(1, songs[2].location.clone());
    a.play(0).unwrap();
    // a.play(1).unwrap();

    sleep(Duration::from_secs(10));
}
