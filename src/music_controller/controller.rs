//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions
#![allow(while_true)]

use kushi::{Queue, QueueItemType};
use kushi::{QueueError, QueueItem};
use std::error::Error;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use uuid::Uuid;

use crate::config::ConfigError;
use crate::music_player::player::{Player, PlayerError};
use crate::music_storage::library::Song;
use crate::{config::Config, music_storage::library::MusicLibrary};

use super::queue::{QueueAlbum, QueueSong};

pub struct Controller<'a, P>(&'a PhantomData<P>);

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("{0:?}")]
    QueueError(#[from] QueueError),
    #[error("{0:?}")]
    PlayerError(#[from] PlayerError),
    #[error("{0:?}")]
    ConfigError(#[from] ConfigError),
}

// TODO: move this to a different location to be used elsewhere
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum PlayerLocation {
    Test,
    Library,
    Playlist(Uuid),
    File,
    Custom,
}

#[derive(Debug, Clone)]
pub struct MailMan<Tx: Send, Rx: Send> {
    tx: async_channel::Sender<Tx>,
    rx: async_channel::Receiver<Rx>,
}

impl<Tx: Send, Rx: Send> MailMan<Tx, Rx> {
    pub fn double() -> (MailMan<Tx, Rx>, MailMan<Rx, Tx>) {
        let (tx, rx) = async_channel::unbounded::<Tx>();
        let (tx1, rx1) = async_channel::unbounded::<Rx>();

        (MailMan { tx, rx: rx1 }, MailMan { tx: tx1, rx })
    }

    pub async fn send(&self, mail: Tx) -> Result<(), async_channel::SendError<Tx>> {
        self.tx.send(mail).await
    }

    pub async fn recv(&self) -> Result<Rx, async_channel::RecvError> {
        self.rx.recv().await
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum PlayerCommand {
    NextSong,
    PrevSong,
    Pause,
    Play,
    Enqueue(usize),
    SetVolume(f64),
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum PlayerResponse {
    Empty,
}

pub enum LibraryCommand {
    Song(Uuid),
}

pub enum LibraryResponse {
    Songs(Song),
}

enum InnerLibraryCommand {
    Song(Uuid),
}

enum InnerLibraryResponse<'a> {
    Song(&'a Song),
}

pub enum QueueCommand {
    Append(QueueItem<QueueSong, QueueAlbum>),
    Next,
    Prev,
    GetIndex(usize),
    NowPlaying,
}

pub enum QueueResponse {
    Ok,
    Item(QueueItem<QueueSong, QueueAlbum>),
}


pub struct ControllerInput<'a> {
    player_mail: (
        MailMan<PlayerCommand, PlayerResponse>,
        MailMan<PlayerResponse, PlayerCommand>,
    ),
    lib_mail: MailMan<LibraryResponse, LibraryCommand>,
    library: &'a mut MusicLibrary,
    config: Arc<RwLock<Config>>,
}

pub struct ControllerHandle {
    lib_mail: MailMan<LibraryCommand, LibraryResponse>,
    player_mail: MailMan<PlayerCommand, PlayerResponse>,
}

#[allow(unused_variables)]
impl<'c, P: Player + Send + Sync> Controller<'c, P> {
    pub async fn start(
        ControllerInput {
            player_mail,
            lib_mail,
            library,
            config
        }: ControllerInput<'c>
    ) -> Result<(), Box<dyn Error>>
    where
        P: Player,
    {
        //TODO: make a separate event loop for sccessing library that clones borrowed values from inner library loop?
        let mut queue: Queue<QueueSong, QueueAlbum> = Queue {
            items: Vec::new(),
            played: Vec::new(),
            loop_: false,
            shuffle: None,
        };

        for song in &library.library {
            queue.add_item(
                QueueSong {
                    song: song.clone(),
                    location: PlayerLocation::Test,
                },
                true,
            );
        }
        let inner_lib_mail = MailMan::double();
        let queue = queue;

        std::thread::scope(|scope| {
            let queue_mail = MailMan::double();
            let a = scope.spawn(|| {
                futures::executor::block_on(async {
                    moro::async_scope!(|scope| {
                        println!("async scope created");
                        let player = Arc::new(RwLock::new(P::new().unwrap()));

                        let _player = player.clone();
                        scope
                            .spawn(async move {
                                Controller::<P>::player_command_loop(
                                    _player,
                                    player_mail.1,
                                    queue_mail.0,
                                )
                                .await
                                .unwrap();
                            })
                            .await;
                        scope
                            .spawn(async move {
                                Controller::<P>::player_event_loop(player, player_mail.0)
                                    .await
                                    .unwrap();
                            })
                            .await;
                        scope
                            .spawn(async {
                                Controller::<P>::inner_library_loop(inner_lib_mail.1, library)
                                    .await
                                    .unwrap()
                            })
                            .await;
                        scope
                            .spawn(async {
                                Controller::<P>::outer_library_loop(lib_mail, inner_lib_mail.0)
                                    .await
                                    .unwrap();
                            })
                            .await
                    })
                    .await;
                })
            });

            let b = scope.spawn(|| {
                futures::executor::block_on(async {
                    Controller::<P>::queue_loop(queue, queue_mail.1).await;
                })
            });
            a.join().unwrap();
            b.join().unwrap();
        });

        Ok(())
    }

    async fn player_command_loop(
        player: Arc<RwLock<P>>,
        player_mail: MailMan<PlayerResponse, PlayerCommand>,
        queue_mail: MailMan<QueueCommand, QueueResponse>,
    ) -> Result<(), ()> {
        {
            player.write().unwrap().set_volume(0.05);
        }
        while true {
            let _mail = player_mail.recv().await;
            if let Ok(mail) = _mail {
                match mail {
                    PlayerCommand::Play => {
                        player.write().unwrap().play().unwrap();
                        player_mail.send(PlayerResponse::Empty).await.unwrap();
                    }
                    PlayerCommand::Pause => {
                        player.write().unwrap().pause().unwrap();
                        player_mail.send(PlayerResponse::Empty).await.unwrap();
                    }
                    PlayerCommand::SetVolume(volume) => {
                        player.write().unwrap().set_volume(volume);
                        println!("volume set to {volume}");
                        player_mail.send(PlayerResponse::Empty).await.unwrap();
                    }
                    PlayerCommand::NextSong => {
                        queue_mail.send(QueueCommand::Next).await.unwrap();

                        if let QueueResponse::Item(item) = queue_mail.recv().await.unwrap() {
                            let uri = match &item.item {
                                QueueItemType::Single(song) => song.song.primary_uri().unwrap().0,
                                _ => unimplemented!(),
                            };
                            player.write().unwrap().enqueue_next(uri).unwrap();
                            player_mail.send(PlayerResponse::Empty).await.unwrap();
                        }
                    }
                    PlayerCommand::PrevSong => {
                        queue_mail.send(QueueCommand::Prev).await.unwrap();

                        if let QueueResponse::Item(item) = queue_mail.recv().await.unwrap() {
                            let uri = match &item.item {
                                QueueItemType::Single(song) => song.song.primary_uri().unwrap().0,
                                _ => unimplemented!(),
                            };
                            player.write().unwrap().enqueue_next(uri).unwrap();
                            player_mail.send(PlayerResponse::Empty).await.unwrap();
                        }
                    }
                    PlayerCommand::Enqueue(index) => {
                        queue_mail
                            .send(QueueCommand::GetIndex(index))
                            .await
                            .unwrap();
                        if let QueueResponse::Item(item) = queue_mail.recv().await.unwrap() {
                            match item.item {
                                QueueItemType::Single(song) => {
                                    player
                                        .write()
                                        .unwrap()
                                        .enqueue_next(song.song.primary_uri().unwrap().0)
                                        .unwrap();
                                }
                                _ => unimplemented!(),
                            }
                            player_mail.send(PlayerResponse::Empty).await.unwrap();
                        }
                    }
                }
            } else {
                return Err(());
            }
        }
        Ok(())
    }

    async fn outer_library_loop(
        lib_mail: MailMan<LibraryResponse, LibraryCommand>,
        inner_lib_mail: MailMan<InnerLibraryCommand, InnerLibraryResponse<'c>>,
    ) -> Result<(), ()> {
        while true {
            match lib_mail.recv().await.unwrap() {
                LibraryCommand::Song(uuid) => {
                    inner_lib_mail
                        .send(InnerLibraryCommand::Song(uuid))
                        .await
                        .unwrap();
                    let x = inner_lib_mail.recv().await.unwrap();
                }
            }
        }
        Ok(())
    }

    async fn inner_library_loop(
        lib_mail: MailMan<InnerLibraryResponse<'c>, InnerLibraryCommand>,
        library: &'c mut MusicLibrary,
    ) -> Result<(), ()> {
        while true {
            match lib_mail.recv().await.unwrap() {
                InnerLibraryCommand::Song(uuid) => {
                    let song: &'c Song = library.query_uuid(&uuid).unwrap().0;
                    lib_mail
                        .send(InnerLibraryResponse::Song(song))
                        .await
                        .unwrap();
                }
            }
        }
        Ok(())
    }

    async fn player_event_loop(
        player: Arc<RwLock<P>>,
        player_mail: MailMan<PlayerCommand, PlayerResponse>,
    ) -> Result<(), ()> {
        // just pretend this does something
        Ok(())
    }

    async fn queue_loop(
        mut queue: Queue<QueueSong, QueueAlbum>,
        queue_mail: MailMan<QueueResponse, QueueCommand>,
    ) {
        while true {
            match queue_mail.recv().await.unwrap() {
                QueueCommand::Append(item) => match item.item {
                    QueueItemType::Single(song) => queue.add_item(song, true),
                    _ => unimplemented!(),
                },
                QueueCommand::Next => {
                    let next = queue.next().unwrap();
                    queue_mail
                        .send(QueueResponse::Item(next.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::Prev => {
                    let next = queue.prev().unwrap();
                    queue_mail
                        .send(QueueResponse::Item(next.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::GetIndex(index) => {
                    let item = queue.items[index].clone();
                    queue_mail.send(QueueResponse::Item(item)).await.unwrap();
                }
                QueueCommand::NowPlaying => {
                    let item = queue.current().unwrap();
                    queue_mail
                        .send(QueueResponse::Item(item.clone()))
                        .await
                        .unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod test_super {
    use std::{
        path::PathBuf,
        sync::{Arc, RwLock},
        thread::spawn,
    };

    use crate::{
        config::{tests::new_config_lib, Config},
        music_controller::controller::{
            LibraryCommand, LibraryResponse, MailMan, PlayerCommand, PlayerResponse,
        },
        music_player::gstreamer::GStreamer,
        music_storage::library::MusicLibrary,
    };

    use super::Controller;

    #[tokio::test]
    async fn construct_controller() {
        // use if you don't have a config setup and add music to the music folder
        new_config_lib();

        let lib_mail: (MailMan<LibraryCommand, LibraryResponse>, MailMan<_, _>) = MailMan::double();
        let player_mail: (MailMan<PlayerCommand, PlayerResponse>, MailMan<_, _>) =
            MailMan::double();

        let _player_mail = player_mail.0.clone();
        let b = spawn(move || {
            futures::executor::block_on(async {
                _player_mail
                    .send(PlayerCommand::SetVolume(0.01))
                    .await
                    .unwrap();
                loop {
                    let buf: String = text_io::read!();
                    dbg!(&buf);
                    _player_mail
                        .send(match buf.to_lowercase().as_str() {
                            "next" => PlayerCommand::NextSong,
                            "prev" => PlayerCommand::PrevSong,
                            "pause" => PlayerCommand::Pause,
                            "play" => PlayerCommand::Play,
                            x if x.parse::<usize>().is_ok() => {
                                PlayerCommand::Enqueue(x.parse::<usize>().unwrap())
                            }
                            _ => continue,
                        })
                        .await
                        .unwrap();
                    println!("sent it");
                    println!("{:?}", _player_mail.recv().await.unwrap())
                }
            })
        });

        let a = spawn(move || {
            futures::executor::block_on(async {
                let config = Config::read_file(PathBuf::from(std::env!("CONFIG-PATH"))).unwrap();
                let mut library = {
                    MusicLibrary::init(
                        config.libraries.get_default().unwrap().path.clone(),
                        config.libraries.get_default().unwrap().uuid,
                    )
                    .unwrap()
                };

                Controller::<GStreamer>::start(
                    crate::music_controller::controller::ControllerInput {
                        player_mail,
                        lib_mail: lib_mail.1,
                        library: &mut library,
                        config: Arc::new(RwLock::new(config)),
                    }
                )
                .await
                .unwrap();
            });
        });

        b.join().unwrap();
        a.join().unwrap();
    }
}
