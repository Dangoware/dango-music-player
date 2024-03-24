//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use crossbeam_channel::{Sender, Receiver};
use crossbeam_channel;
use std::thread::spawn;

use std::error::Error;
use crossbeam_channel::unbounded;
use uuid::Uuid;

use crate::music_storage::library::{Tag, URI};
use crate::{
    music_storage::library::{MusicLibrary, Song},
    config::config::Config,
    music_controller::queue::Queue,
};

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
    SaveLibrary,
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
    Library(MusicLibrary),
}

#[derive(Debug)]
enum QueueCmd {
    Default,
    Test,
    Play,
    Pause,
    // SetSongs(Vec<QueueItem<QueueState>>),
    // SetLocation(URI),
    Enqueue(URI),
    SetVolume(f64),
}

#[derive(Debug)]
enum QueueResponse {
    Default,
    Test,
    Index(i32),
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

        let config_ = Arc::new(RwLock::from(config));
        let mut lib = MusicLibrary::init(config_.clone(), uuid)?;

        let config = config_.clone();
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

        let config = config_.clone();
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
                    SaveLibrary => {
                        //TODO: make this send lib ref to the function to save instead
                        lib.save(config.read().unwrap().to_owned()).unwrap();
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
                config: config_.clone(),
                controller_mail: out_thread_controller,
                db_mail: out_thread_db,
                queue_mail: Vec::new(),
            }
        )
    }

    fn lib_get_songs(&self) -> Vec<Song> {
        self.db_mail.send(DatabaseCmd::GetSongs);
        match self.db_mail.recv().unwrap() {
            DatabaseResponse::Songs(songs) => songs,
            _ => Vec::new()
        }
    }

    fn lib_scan_folder(&self, folder: String) -> Result<(), Box<dyn Error>> {
        let mail = &self.db_mail;
        mail.send(DatabaseCmd::ReadFolder(folder))?;
        dbg!(mail.recv()?);
        Ok(())
    }

    pub fn lib_save(&self) -> Result<(), Box<dyn Error>> {
        self.db_mail.send(DatabaseCmd::SaveLibrary);
        Ok(())
    }

    pub fn q_new(&mut self) -> Result<usize, Box<dyn Error>> {
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
                            Err(_) => todo!()
                        };

                    },
                    Pause => {
                        match queue.player.pause() {
                            Ok(_) => in_thread.send(QueueResponse::Default).unwrap(),
                            Err(_) => todo!()
                        }
                    },
                    // SetSongs(songs) => {
                    //     queue.set_tracks(songs);
                    //     in_thread.send(QueueResponse::Default).unwrap();
                    // },
                    Enqueue(uri) => {
                        queue.player.enqueue_next(&uri).unwrap();

                        // in_thread.send(QueueResponse::Default).unwrap();
                    },
                    SetVolume(vol) => {
                        queue.player.set_volume(vol);
                    }
                }
            }
        });
        self.queue_mail.push(out_thread_queue);
        Ok(self.queue_mail.len() - 1)
    }

    fn q_play(&self, index: usize) -> Result<(), Box<dyn Error>> {
        let mail = &self.queue_mail[index];
        mail.send(QueueCmd::Play)?;
        dbg!(mail.recv()?);
        Ok(())
    }

    fn q_pause(&self, index: usize) -> Result<(), Box<dyn Error>> {
        let mail = &self.queue_mail[index];
        mail.send(QueueCmd::Pause)?;
        dbg!(mail.recv()?);
        Ok(())
    }

    pub fn q_set_volume(&self, index: usize, volume: f64) -> Result<(), Box<dyn Error>> {
        let mail = &self.queue_mail[index];
        mail.send(QueueCmd::SetVolume(volume))?;
        Ok(())
    }

    // fn q_set_songs(&self, index: usize, songs: Vec<QueueItem<QueueState>>) -> Result<(), Box<dyn Error>> {
    //     let mail = &self.queue_mail[index];
    //     mail.send(QueueCmd::SetSongs(songs))?;
    //     dbg!(mail.recv()?);
    //     Ok(())
    // }

    fn q_enqueue(&self, index: usize, uri: URI) -> Result<(), Box<dyn Error>> {
        let mail = &self.queue_mail[index];
        mail.send(QueueCmd::Enqueue(uri))?;
        // dbg!(mail.recv()?);
        Ok(())
    }


}

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use super::Controller;

    #[test]
    fn play_test() {
        let mut a = match Controller::start("test-config/config_test.json".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("{e}")
        };
        sleep(Duration::from_millis(500));

        let i = a.q_new().unwrap();
        a.q_set_volume(i, 0.04);
        // a.new_queue();
        let songs = a.lib_get_songs();
        a.q_enqueue(i, songs[2].location.clone());
        // a.enqueue(1, songs[2].location.clone());
        a.q_play(i).unwrap();
        // a.play(1).unwrap();

        sleep(Duration::from_secs(10));
        a.q_pause(i);
        sleep(Duration::from_secs(10));
        a.q_play(i);
        sleep(Duration::from_secs(1000));
    }

    #[test]
    fn test_() {
        let a = match Controller::start("test-config/config_test.json".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("{e}")
        };
        a.lib_scan_folder("F:/Music/Mp3".to_string());
        a.lib_save();
    }
}
