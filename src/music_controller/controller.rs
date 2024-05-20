//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions

use crossbeam_channel;
use crossbeam_channel::{Receiver, Sender};
use listenbrainz::ListenBrainz;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::thread::spawn;

use crossbeam_channel::unbounded;
use std::error::Error;
use uuid::Uuid;

use crate::music_controller::queue::QueueItem;
use crate::music_player::gstreamer::GStreamer;
use crate::music_storage::library::{Tag, URI};
use crate::{
    config::config::Config,
    music_controller::queue::Queue,
    music_storage::library::{MusicLibrary, Song},
};

pub struct Controller {
    pub queue: Queue,
    pub config: Arc<RwLock<Config>>,
    pub library: MusicLibrary,
    player_mail: MailMan<PlayerCmd, PlayerRes>,
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

enum PlayerCmd {
    Test(URI),
}

enum PlayerRes {
    Test,
}

#[allow(unused_variables)]
impl Controller {
    pub fn start<P>(config_path: P) -> Result<Self, Box<dyn Error>>
    where
        std::path::PathBuf: std::convert::From<P>,
    {
        let config_path = PathBuf::from(config_path);

        let config = Config::read_file(config_path)?;
        let uuid = config.libraries.get_default()?.uuid;

        let config_ = Arc::new(RwLock::from(config));
        let library = MusicLibrary::init(config_.clone(), uuid)?;

        let (player_mail, in_thread) = MailMan::<PlayerCmd, PlayerRes>::double();

        spawn(move || {
            let mut player = GStreamer::new().unwrap();

            while true {
                match in_thread.recv().unwrap() {
                    PlayerCmd::Test(uri) => {
                        &player.set_volume(0.04);
                        _ = &player.enqueue_next(&uri).unwrap();
                        _ = &player.play();
                        in_thread.send(PlayerRes::Test).unwrap();
                    }
                }
            }
        });

        Ok(Controller {
            queue: Queue::new(),
            config: config_.clone(),
            library,
            player_mail,
        })
    }

    pub fn q_add(&self, item: Uuid, source: super::queue::PlayerLocation, by_human: bool) {
        self.queue.add_item(item, source, by_human)
    }
}

#[cfg(test)]
mod test_super {
    use std::{thread::sleep, time::Duration};

    use super::Controller;

    #[test]
    fn play_test() {
        let mut a = match Controller::start("test-config/config_test.json".to_string()) {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        };
        sleep(Duration::from_millis(500));
    }

    #[test]
    fn test_() {
        let c = Controller::start(
            "F:\\Dangoware\\Dango Music Player\\dmp-core\\test-config\\config_test.json",
        )
        .unwrap();

        sleep(Duration::from_secs(60));
    }
}
