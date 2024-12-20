//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions
#![allow(while_true)]

use kushi::{Queue, QueueItemType};
use kushi::{QueueError, QueueItem};
use serde::{Deserialize, Serialize};
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
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
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
    PlayNow(Uuid, PlayerLocation),
}

#[derive(Debug, PartialEq, Clone)]
pub enum PlayerResponse {
    Empty,
    NowPlaying(Song)
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum LibraryCommand {
    Song(Uuid),
    AllSongs,
    GetLibrary,
}

#[derive(Debug)]
pub enum LibraryResponse {
    Song(Song),
    AllSongs(Vec<Song>),
    Library(MusicLibrary),
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
enum InnerLibraryCommand {
    Song(Uuid),
    AllSongs,
}

#[derive(Debug, PartialEq, Clone)]
enum InnerLibraryResponse<'a> {
    Song(&'a Song, usize),
    AllSongs(&'a Vec<Song>),
}


#[derive(Debug, PartialEq, Clone)]
pub enum QueueCommand {
    Append(QueueItem<QueueSong, QueueAlbum>, bool),
    Next,
    Prev,
    GetIndex(usize),
    NowPlaying,
    Get,
    Clear
}

#[derive(Debug, PartialEq, Clone)]
pub enum QueueResponse {
    Ok,
    Item(QueueItem<QueueSong, QueueAlbum>),
    GetAll(Vec<QueueItem<QueueSong, QueueAlbum>>),
    Err(QueueError),
}


pub struct ControllerInput {
    player_mail: (
        MailMan<PlayerCommand, PlayerResponse>,
        MailMan<PlayerResponse, PlayerCommand>,
    ),
    lib_mail: MailMan<LibraryResponse, LibraryCommand>,
    queue_mail: (
        MailMan<QueueCommand, QueueResponse>,
        MailMan<QueueResponse, QueueCommand>
    ),
    library: MusicLibrary,
    config: Arc<RwLock<Config>>,
}

pub struct ControllerHandle {
    pub lib_mail: MailMan<LibraryCommand, LibraryResponse>,
    pub player_mail: MailMan<PlayerCommand, PlayerResponse>,
    pub queue_mail: MailMan<QueueCommand, QueueResponse>,
}

impl ControllerHandle {
    pub fn new(library: MusicLibrary, config: Arc<RwLock<Config>>) -> (Self, ControllerInput) {
        let lib_mail = MailMan::double();
        let player_mail = MailMan::double();
        let queue_mail = MailMan::double();

        (
            ControllerHandle {
                lib_mail: lib_mail.0,
                player_mail: player_mail.0.clone(),
                queue_mail: queue_mail.0.clone()
            },
            ControllerInput {
                player_mail,
                lib_mail: lib_mail.1,
                queue_mail: queue_mail,
                library,
                config
            }
        )
    }
}

#[allow(unused_variables)]
impl<'c, P: Player + Send + Sync> Controller<'c, P> {
    pub async fn start(
        ControllerInput {
            player_mail,
            lib_mail,
            queue_mail,
            mut library,
            config
        }: ControllerInput
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
        // for testing porpuses
        // for song in &library.library {
        //     queue.add_item(
        //         QueueSong {
        //             song: song.clone(),
        //             location: PlayerLocation::Test,
        //         },
        //         true,
        //     );
        // }
        let inner_lib_mail = MailMan::double();
        let queue = queue;

        std::thread::scope(|scope| {
            let queue_mail = queue_mail;
            let a = scope.spawn(|| {
                futures::executor::block_on(async {
                    moro::async_scope!(|scope| {
                        println!("async scope created");
                        let player = Arc::new(RwLock::new(P::new().unwrap()));

                        let _player = player.clone();
                        let _inner_lib_mail = inner_lib_mail.0.clone();
                        scope
                            .spawn(async move {
                                Controller::<P>::player_command_loop(
                                    _player,
                                    player_mail.1,
                                    queue_mail.0,
                                    _inner_lib_mail
                                )
                                .await
                                .unwrap();
                            });
                        scope
                            .spawn(async move {
                                Controller::<P>::player_event_loop(player, player_mail.0)
                                    .await
                                    .unwrap();
                            });
                        scope
                            .spawn(async {
                                Controller::<P>::inner_library_loop(inner_lib_mail.1, &mut library).await
                                    .unwrap()
                            });
                        scope
                            .spawn(async {
                                Controller::<P>::outer_library_loop(lib_mail, inner_lib_mail.0)
                                    .await
                                    .unwrap();
                            });
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
        inner_lib_mail: MailMan<InnerLibraryCommand, InnerLibraryResponse<'c>>
    ) -> Result<(), ()> {
        let mut first = true;
        while true {
            let _mail = player_mail.recv().await;
            if let Ok(mail) = _mail {
                match mail {
                    PlayerCommand::Play => {
                        if first {
                            queue_mail.send(QueueCommand::NowPlaying).await.unwrap();
                            let QueueResponse::Item(item) = queue_mail.recv().await.unwrap() else { unimplemented!() };
                            let QueueItemType::Single(song) = item.item else { unimplemented!("This is temporary, handle queueItemTypes at some point") };
                            player.write().unwrap().enqueue_next(song.song.primary_uri().unwrap().0).unwrap();
                            player_mail.send(PlayerResponse::NowPlaying(song.song)).await.unwrap();
                            first = false
                        } else {
                            player.write().unwrap().play().unwrap();
                            player_mail.send(PlayerResponse::Empty).await.unwrap();
                        }
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
                            let QueueItemType::Single(np_song) = item.item else { panic!("This is temporary, handle queueItemTypes at some point")};

                            // Append next song in library
                            inner_lib_mail.send(InnerLibraryCommand::AllSongs).await.unwrap();

                            let InnerLibraryResponse::AllSongs(songs) = inner_lib_mail.recv().await.unwrap() else {
                                unreachable!()
                            };
                            inner_lib_mail.send(InnerLibraryCommand::Song(np_song.song.uuid.clone())).await.unwrap();
                            let InnerLibraryResponse::Song(_, i) = inner_lib_mail.recv().await.unwrap() else {
                                unreachable!()
                            };
                            if let Some(song) = songs.get(i + 49) {
                                queue_mail.send(
                                    QueueCommand::Append(
                                        QueueItem::from_item_type(
                                            QueueItemType::Single(
                                                QueueSong {
                                                    song: song.clone(),
                                                    location: np_song.location
                                                }
                                            )
                                        ),
                                        false
                                    )
                                ).await
                                .unwrap();
                                let QueueResponse::Ok = queue_mail.recv().await.unwrap() else {
                                    unreachable!()
                                };
                            } else {
                                println!("Library Empty");
                            }

                            player_mail.send(PlayerResponse::NowPlaying(np_song.song.clone())).await.unwrap();
                        }
                    }
                    PlayerCommand::PrevSong => {
                        queue_mail.send(QueueCommand::Prev).await.unwrap();

                        if let QueueResponse::Item(item) = queue_mail.recv().await.unwrap() {
                            let uri = match &item.item {
                                QueueItemType::Single(song) => song.song.primary_uri().unwrap().0,
                                _ => unimplemented!(),
                            };
                            let QueueItemType::Single(np_song) = item.item else { panic!("This is temporary, handle queueItemTypes at some point")};
                            player_mail.send(PlayerResponse::NowPlaying(np_song.song.clone())).await.unwrap();
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
                    PlayerCommand::PlayNow(uuid, location) => {
                        // TODO: This assumes the uuid doesn't point to an album. we've been over this.
                        inner_lib_mail.send(InnerLibraryCommand::Song(uuid)).await.unwrap();
                        let InnerLibraryResponse::Song(song, index) = inner_lib_mail.recv().await.unwrap() else {
                            unreachable!()
                        };
                        queue_mail.send(QueueCommand::Clear).await.unwrap();
                        let QueueResponse::Ok = queue_mail.recv().await.unwrap() else {
                            unreachable!()
                        };
                        queue_mail.send(QueueCommand::Append(QueueItem::from_item_type(QueueItemType::Single(QueueSong { song: song.clone(), location: location })), true)).await.unwrap();
                        let QueueResponse::Ok = queue_mail.recv().await.unwrap() else {
                            unreachable!()
                        };

                        player.write().unwrap().enqueue_next(song.primary_uri().unwrap().0).unwrap();

                        // how grab all the songs in a certain subset of the library, I reckon?
                        // ...
                        // let's just pretend I figured that out already

                        inner_lib_mail.send(InnerLibraryCommand::AllSongs).await.unwrap();
                        let InnerLibraryResponse::AllSongs(songs) = inner_lib_mail.recv().await.unwrap() else {
                            unreachable!()
                        };

                        for i in index+1..(index+50) {
                            if let Some(song) = songs.get(i) {
                                queue_mail.send(QueueCommand::Append(QueueItem::from_item_type(QueueItemType::Single(QueueSong { song: song.clone(), location })), false)).await.unwrap();
                                let QueueResponse::Ok = queue_mail.recv().await.unwrap() else {
                                    unreachable!()
                                };
                            } else {
                                println!("End of Library");
                                break;
                            }
                        }
                        // ^ This be my solution for now ^
                        player_mail.send(PlayerResponse::NowPlaying(song.clone())).await.unwrap();
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
                    let InnerLibraryResponse::Song(song, i) = inner_lib_mail.recv().await.unwrap() else {
                        unimplemented!();
                    };
                    lib_mail.send(LibraryResponse::Song(song.clone())).await.unwrap();
                }
                LibraryCommand::AllSongs => {
                    inner_lib_mail
                    .send(InnerLibraryCommand::AllSongs)
                    .await
                    .unwrap();
                    let x = inner_lib_mail.recv().await.unwrap();
                    if let InnerLibraryResponse::AllSongs(songs) = x {
                        lib_mail.send(LibraryResponse::AllSongs(songs.clone())).await.unwrap();
                    } else {
                        unreachable!()
                    }
                },
                _ => { todo!() }
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
                    let (song, i): (&'c Song, usize) = library.query_uuid(&uuid).unwrap();
                    lib_mail
                        .send(InnerLibraryResponse::Song(song, i))
                        .await
                        .unwrap();
                }
                InnerLibraryCommand::AllSongs => {
                    let songs: &'c Vec<Song> = &library.library;
                    lib_mail.send(InnerLibraryResponse::AllSongs(songs))
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
                QueueCommand::Append(item, by_human) => {
                    match item.item {
                        QueueItemType::Single(song) => queue.add_item(song, by_human),
                        _ => unimplemented!(),
                    }
                    queue_mail
                        .send(QueueResponse::Ok)
                        .await
                        .unwrap();
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
                    let item = queue.items.get(index).expect("No item in the queue at index {index}").clone();
                    queue_mail.send(QueueResponse::Item(item)).await.unwrap();
                }
                QueueCommand::NowPlaying => {
                    let item = queue.current().unwrap();
                    queue_mail
                        .send(QueueResponse::Item(item.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::Get => {
                    queue_mail.send(QueueResponse::GetAll(queue.items.clone())).await.unwrap();
                }
                QueueCommand::Clear => {
                    queue.clear();
                    queue_mail.send(QueueResponse::Ok).await.unwrap();
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
            LibraryCommand, LibraryResponse, MailMan, PlayerCommand, PlayerResponse, ControllerHandle
        },
        music_player::gstreamer::GStreamer,
        music_storage::library::MusicLibrary,
    };

    use super::Controller;

    #[tokio::test]
    async fn construct_controller() {
        // use if you don't have a config setup and add music to the music folder
        new_config_lib();

        let config = Config::read_file(PathBuf::from(std::env!("CONFIG-PATH"))).unwrap();
        let library = {
            MusicLibrary::init(
                config.libraries.get_default().unwrap().path.clone(),
                config.libraries.get_default().unwrap().uuid,
            )
            .unwrap()
        };

        let (handle, input) = ControllerHandle::new(library, Arc::new(RwLock::new(config)));

        let b = spawn(move || {
            futures::executor::block_on(async {
                handle.player_mail
                    .send(PlayerCommand::SetVolume(0.01))
                    .await
                    .unwrap();
                loop {
                    let buf: String = text_io::read!();
                    dbg!(&buf);
                    handle.player_mail
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
                    println!("{:?}", handle.player_mail.recv().await.unwrap())
                }
            })
        });

        let a = spawn(move || {
            futures::executor::block_on(async {


                Controller::<GStreamer>::start(input)
                .await
                .unwrap();
            });
        });

        b.join().unwrap();
        a.join().unwrap();
    }
}

