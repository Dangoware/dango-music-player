//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions

use crossbeam_channel;
use crossbeam_channel::{Receiver, Sender};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{sleep, spawn};
use std::time::Duration;
use thiserror::Error;

use crossbeam_channel::unbounded;
use std::error::Error;
use uuid::Uuid;

use crate::config::ConfigError;
use crate::music_player::player::{Player, PlayerCommand, PlayerError};
use crate::{
    config::Config, music_controller::queue::Queue, music_storage::library::MusicLibrary,
};

use super::queue::QueueError;

pub struct Controller<P: Player + Send + Sync> {
    pub queue: Arc<RwLock<Queue>>,
    pub config: Arc<RwLock<Config>>,
    pub library: MusicLibrary,
    pub player: Arc<Mutex<P>>,
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("{0:?}")]
    QueueError(#[from] QueueError),
    #[error("{0:?}")]
    PlayerError(#[from] PlayerError),
    #[error("{0:?}")]
    ConfigError(#[from] ConfigError),
}

#[derive(Debug)]
pub(super) struct MailMan<T: Send, U: Send> {
    pub tx: Sender<T>,
    rx: Receiver<U>,
}

impl<T: Send> MailMan<T, T> {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<T>();
        MailMan { tx, rx }
    }
}
impl<T: Send, U: Send> MailMan<T, U> {
    pub fn double() -> (MailMan<T, U>, MailMan<U, T>) {
        let (tx, rx) = unbounded::<T>();
        let (tx1, rx1) = unbounded::<U>();

        (MailMan { tx, rx: rx1 }, MailMan { tx: tx1, rx })
    }

    pub fn send(&self, mail: T) -> Result<(), Box<dyn Error>> {
        self.tx.send(mail).unwrap();
        Ok(())
    }

    pub fn recv(&self) -> Result<U, Box<dyn Error>> {
        let u = self.rx.recv()?;
        Ok(u)
    }
}

#[allow(unused_variables)]
impl<P: Player + Send + Sync + Sized + 'static> Controller<P> {
    pub fn start<T>(config_path: T) -> Result<Self, Box<dyn Error>>
    where
        std::path::PathBuf: std::convert::From<T>,
        P: Player,
    {
        let config_path = PathBuf::from(config_path);

        let config = Config::read_file(config_path)?;
        let uuid = config.libraries.get_default()?.uuid;

        let config_ = Arc::new(RwLock::from(config));
        let library = MusicLibrary::init(config_.clone(), uuid)?;

        let controller = Controller {
            queue: Arc::new(RwLock::from(Queue::default())),
            config: config_.clone(),
            library,
            player: Arc::new(Mutex::new(P::new()?)),
        };


        let player = controller.player.clone();
        let queue = controller.queue.clone();
        let controller_thread = spawn(move || {
            loop {
                let signal = { player.lock().unwrap().message_channel().recv().unwrap() };
                match signal {
                    PlayerCommand::AboutToFinish => {
                        println!("Switching songs!");

                        let mut queue = queue.write().unwrap();

                        let uri = queue
                                .next()
                                .unwrap()
                                .clone();

                        player
                            .lock()
                            .unwrap()
                            .enqueue_next(uri.item
                                .primary_uri()
                                .unwrap()
                                .0)
                            .unwrap();
                    },
                    PlayerCommand::EndOfStream => {dbg!()}
                    _ => {}
                }
            }

        });


        Ok(controller)
    }

    pub fn q_add(&mut self, item: &Uuid, source: super::queue::PlayerLocation, by_human: bool) {
        let item = self.library.query_uuid(item).unwrap().0.to_owned();
        self.queue.write().unwrap().add_item(item, source, by_human)
    }
}

#[cfg(test)]
mod test_super {
    use std::{thread::sleep, time::Duration};

    use crate::{config::tests::read_config_lib, music_controller::queue::PlayerLocation, music_player::{gstreamer::GStreamer, player::Player}};

    use super::Controller;

    #[test]
    fn construct_controller() {
        println!("starto!");
        let config = read_config_lib();

        let next = config.1.library[2].clone();
        {
            let controller = Controller::<GStreamer>::start("test-config/config_test.json").unwrap();
            {
                let mut queue = controller.queue.write().unwrap();
                for x in config.1.library {
                    queue.add_item(x, PlayerLocation::Library, true);
                }
            }
            {
                controller.player.lock().unwrap().enqueue_next(next.primary_uri().unwrap().0).unwrap();
            }
            {
                controller.player.lock().unwrap().set_volume(0.2);
            }
            {
                controller.player.lock().unwrap().play().unwrap();
            }
            println!("I'm a tire");
        }
        sleep(Duration::from_secs(600))

    }
}
