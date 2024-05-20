//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions

use crossbeam_channel;
use crossbeam_channel::{Receiver, Sender};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crossbeam_channel::unbounded;
use std::error::Error;
use uuid::Uuid;

use crate::music_player::player::Player;
use crate::{
    config::config::Config, music_controller::queue::Queue, music_storage::library::MusicLibrary,
};

pub struct Controller<P: Player> {
    pub queue: Queue,
    pub config: Arc<RwLock<Config>>,
    pub library: MusicLibrary,
    pub player: Box<P>,
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
impl<P: Player> Controller<P> {
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

        Ok(Controller {
            queue: Queue::default(),
            config: config_.clone(),
            library,
            player: Box::new(P::new()),
        })
    }

    pub fn q_add(&mut self, item: &Uuid, source: super::queue::PlayerLocation, by_human: bool) {
        let item = self.library.query_uuid(item).unwrap().0.to_owned();
        self.queue.add_item(item, source, by_human)
    }
}

#[cfg(test)]
mod test_super {}
