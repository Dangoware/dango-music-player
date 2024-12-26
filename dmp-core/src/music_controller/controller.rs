//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions
#![allow(while_true)]

use kushi::{Queue, QueueItemType};
use kushi::{QueueError, QueueItem};
use prismriver::{Prismriver, Volume};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use thiserror::Error;
use uuid::Uuid;

use crate::config::ConfigError;
use crate::music_storage::library::Song;
use crate::music_storage::playlist::{ExternalPlaylist, Playlist, PlaylistFolderItem};
use crate::{config::Config, music_storage::library::MusicLibrary};

use super::queue::{QueueAlbum, QueueSong};

pub struct Controller();

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("{0:?}")]
    QueueError(#[from] QueueError),
    #[error("{0:?}")]
    PlayerError(#[from] prismriver::Error),
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
    SetVolume(f32),
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
    ExternalPlaylist(Uuid),
    Playlist(Uuid),
    ImportM3UPlayList(PathBuf),
    Save,
    Playlists,
}

#[derive(Debug, Clone)]
pub enum LibraryResponse {
    Ok,
    Song(Song, usize),
    AllSongs(Vec<Song>),
    Library(MusicLibrary),
    ExternalPlaylist(ExternalPlaylist),
    Playlist(Playlist),
    ImportM3UPlayList(Uuid, String),
    Playlists(Vec<(Uuid, String)>),
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
    lib_mail: (
        MailMan<LibraryCommand, LibraryResponse>,
        MailMan<LibraryResponse, LibraryCommand>
    ),
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
                lib_mail: lib_mail.0.clone(),
                player_mail: player_mail.0.clone(),
                queue_mail: queue_mail.0.clone()
            },
            ControllerInput {
                player_mail,
                lib_mail,
                queue_mail,
                library,
                config
            }
        )
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ControllerState {
    path: PathBuf,
    volume: f32,
    now_playing: Uuid,
}

impl ControllerState {
    fn new(path: PathBuf) -> Self {
        ControllerState {
            path,
            volume: 0.35,
            ..Default::default()
        }
    }

    fn write_file(&self) -> Result<(), std::io::Error> {
        OpenOptions::new()
        .truncate(true)
        .create(true)
        .write(true)
        .open(&self.path)
        .unwrap()
        .write_all(&to_string_pretty(self)?.into_bytes())?;
        Ok(())
    }

    fn read_file(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let state = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        Ok(state)
    }
}

#[allow(unused_variables)]
impl Controller {
    pub async fn start(
        ControllerInput {
            player_mail,
            lib_mail,
            queue_mail,
            mut library,
            config
        }: ControllerInput
    ) -> Result<(), Box<dyn Error>> {
        let queue: Queue<QueueSong, QueueAlbum> = Queue {
            items: Vec::new(),
            played: Vec::new(),
            loop_: false,
            shuffle: None,
        };

        let state = {
            let path = &config.read().unwrap().state_path;
            if let Ok(state) = ControllerState::read_file(path) {
                state
            } else {
                ControllerState::new(path.clone())
            }
        };

        std::thread::scope(|scope| {
            let queue_mail = queue_mail;
            let a = scope.spawn(|| {
                futures::executor::block_on(async {
                    moro::async_scope!(|scope| {
                        println!("async scope created");
                        let player = Arc::new(RwLock::new(Prismriver::new()));

                        let _player = player.clone();
                        let _lib_mail = lib_mail.0.clone();
                        scope
                            .spawn(async move {
                                Controller::player_command_loop(
                                    _player,
                                    player_mail.1,
                                    queue_mail.0,
                                    _lib_mail,
                                    state,
                                )
                                .await
                                .unwrap();
                            });
                        scope
                            .spawn(async move {
                                Controller::player_event_loop(
                                    player,
                                    player_mail.0
                                )
                                    .await
                                    .unwrap();
                            });
                        scope
                            .spawn(async {
                                Controller::library_loop(
                                    lib_mail.1,
                                    &mut library,
                                    config,
                                )
                                    .await
                                    .unwrap();
                            });
                    })
                    .await;
                })
            });

            let b = scope.spawn(|| {
                futures::executor::block_on(async {
                    Controller::queue_loop(queue, queue_mail.1).await;
                })
            });
            a.join().unwrap();
            b.join().unwrap();
        });

        Ok(())
    }

    async fn player_command_loop(
        player: Arc<RwLock<Prismriver>>,
        player_mail: MailMan<PlayerResponse, PlayerCommand>,
        queue_mail: MailMan<QueueCommand, QueueResponse>,
        lib_mail: MailMan<LibraryCommand, LibraryResponse>,
        mut state: ControllerState,
    ) -> Result<(), ()> {
        let mut first = true;
        {
            player.write().unwrap().set_volume(Volume::new(state.volume));
            println!("volume set to {}", state.volume);
        }
        while true {
            let _mail = player_mail.recv().await;
            if let Ok(mail) = _mail {
                match mail {
                    PlayerCommand::Play => {
                        if first {
                            queue_mail.send(QueueCommand::NowPlaying).await.unwrap();
                            let QueueResponse::Item(item) = queue_mail.recv().await.unwrap() else { unimplemented!() };
                            let QueueItemType::Single(song) = item.item else { unimplemented!("This is temporary, handle queueItemTypes at some point") };

                            let prism_uri = prismriver::utils::path_to_uri(&song.song.primary_uri().unwrap().0.as_path().unwrap()).unwrap();
                            player.write().unwrap().load_new(&prism_uri).unwrap();
                            player.write().unwrap().play();

                            player_mail.send(PlayerResponse::NowPlaying(song.song)).await.unwrap();
                            first = false
                        } else {
                            player.write().unwrap().play();
                            player_mail.send(PlayerResponse::Empty).await.unwrap();
                        }
                    }
                    PlayerCommand::Pause => {
                        player.write().unwrap().pause();
                        player_mail.send(PlayerResponse::Empty).await.unwrap();
                    }
                    PlayerCommand::SetVolume(volume) => {
                        player.write().unwrap().set_volume(Volume::new(volume));
                        println!("volume set to {volume}");
                        player_mail.send(PlayerResponse::Empty).await.unwrap();

                        state.volume = volume;
                        _ = state.write_file()
                    }
                    PlayerCommand::NextSong => {
                        queue_mail.send(QueueCommand::Next).await.unwrap();

                        if let QueueResponse::Item(item) = queue_mail.recv().await.unwrap() {
                            let uri = match &item.item {
                                QueueItemType::Single(song) => song.song.primary_uri().unwrap().0,
                                _ => unimplemented!(),
                            };

                            let prism_uri = prismriver::utils::path_to_uri(&uri.as_path().unwrap()).unwrap();
                            player.write().unwrap().load_new(&prism_uri).unwrap();
                            player.write().unwrap().play();

                            let QueueItemType::Single(np_song) = item.item else { panic!("This is temporary, handle queueItemTypes at some point")};

                            // Append next song in library
                            lib_mail.send(LibraryCommand::AllSongs).await.unwrap();

                            let LibraryResponse::AllSongs(songs) = lib_mail.recv().await.unwrap() else {
                                unreachable!()
                            };
                            lib_mail.send(LibraryCommand::Song(np_song.song.uuid)).await.unwrap();
                            let LibraryResponse::Song(_, i) = lib_mail.recv().await.unwrap() else {
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

                            let prism_uri = prismriver::utils::path_to_uri(&uri.as_path().unwrap()).unwrap();
                            player.write().unwrap().load_new(&prism_uri).unwrap();
                            player.write().unwrap().play();

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
                                    let prism_uri = prismriver::utils::path_to_uri(&song.song.primary_uri().unwrap().0.as_path().unwrap()).unwrap();
                                    player.write().unwrap().load_new(&prism_uri).unwrap();
                                    player.write().unwrap().play();
                                }
                                _ => unimplemented!(),
                            }
                            player_mail.send(PlayerResponse::Empty).await.unwrap();
                        }
                    }
                    PlayerCommand::PlayNow(uuid, location) => {
                        // TODO: This assumes the uuid doesn't point to an album. we've been over this.
                        lib_mail.send(LibraryCommand::Song(uuid)).await.unwrap();
                        let LibraryResponse::Song(song, index) = lib_mail.recv().await.unwrap() else {
                            unreachable!()
                        };
                        queue_mail.send(QueueCommand::Clear).await.unwrap();
                        let QueueResponse::Ok = queue_mail.recv().await.unwrap() else {
                            unreachable!()
                        };
                        queue_mail.send(QueueCommand::Append(QueueItem::from_item_type(QueueItemType::Single(QueueSong { song: song.clone(), location })), true)).await.unwrap();
                        let QueueResponse::Ok = queue_mail.recv().await.unwrap() else {
                            unreachable!()
                        };

                        // TODO: Handle non Local URIs here, and whenever `load_new()` or `load_gapless()` is called
                        let prism_uri = prismriver::utils::path_to_uri(&song.primary_uri().unwrap().0.as_path().unwrap()).unwrap();
                        player.write().unwrap().load_new(&prism_uri).unwrap();
                        player.write().unwrap().play();

                        // how grab all the songs in a certain subset of the library, I reckon?
                        // ...
                        // let's just pretend I figured that out already

                        let (songs, index) = match location {
                            PlayerLocation::Library => {
                                lib_mail.send(LibraryCommand::AllSongs).await.unwrap();
                                let LibraryResponse::AllSongs(songs) = lib_mail.recv().await.unwrap() else {
                                    unreachable!()
                                };
                                (songs, index)
                            }
                            PlayerLocation::Playlist(uuid) => {
                                lib_mail.send(LibraryCommand::ExternalPlaylist(uuid)).await.unwrap();
                                let LibraryResponse::ExternalPlaylist(list) = lib_mail.recv().await.unwrap() else {
                                    unreachable!()
                                };
                                let index = list.get_index(song.uuid).unwrap();
                                (list.tracks, index)
                            }
                            _ => todo!("Got Location other than Library or Playlist")
                        };


                        for i in index+1..(index+50) {
                            if let Some(song) = songs.get(i) {
                                queue_mail.send(QueueCommand::Append(QueueItem::from_item_type(QueueItemType::Single(QueueSong { song: song.clone(), location })), false)).await.unwrap();
                                let QueueResponse::Ok = queue_mail.recv().await.unwrap() else {
                                    unreachable!()
                                };
                            } else {
                                println!("End of Library / Playlist");
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

    async fn library_loop(
        lib_mail: MailMan<LibraryResponse, LibraryCommand>,
        library: &mut MusicLibrary,
        config: Arc<RwLock<Config>>,
    ) -> Result<(), ()> {
        while true {
            match lib_mail.recv().await.unwrap() {
                LibraryCommand::Song(uuid) => {
                    let (song, i) = library.query_uuid(&uuid).unwrap();
                    lib_mail.send(LibraryResponse::Song(song.clone(), i)).await.unwrap();
                }
                LibraryCommand::AllSongs => {
                    lib_mail.send(LibraryResponse::AllSongs(library.library.clone())).await.unwrap();
                },
                LibraryCommand::ExternalPlaylist(uuid) => {
                    let playlist = library.query_playlist_uuid(&uuid).unwrap();
                    lib_mail.send(LibraryResponse::ExternalPlaylist(ExternalPlaylist::from_playlist(playlist, library))).await.unwrap();
                }
                LibraryCommand::ImportM3UPlayList(path) => {
                    let playlist = Playlist::from_m3u(path, library).unwrap();
                    let uuid = playlist.uuid;
                    let name = playlist.title.clone();
                    library.playlists.items.push(PlaylistFolderItem::List(playlist));

                    lib_mail.send(LibraryResponse::ImportM3UPlayList(uuid, name)).await.unwrap();
                }
                LibraryCommand::Save => {
                    library.save(config.read().unwrap().libraries.get_library(&library.uuid).unwrap().path.clone()).unwrap();
                    lib_mail.send(LibraryResponse::Ok).await.unwrap();
                }
                LibraryCommand::Playlists => {
                    let mut lists = vec![];
                    library.playlists.lists_recursive().into_par_iter().map(|list| (list.uuid, list.title.clone())).collect_into_vec(&mut lists);

                    lib_mail.send(LibraryResponse::Playlists(lists)).await.unwrap();
                }
                _ => { todo!() }
            }
        }
        Ok(())
    }

    async fn player_event_loop(
        player: Arc<RwLock<Prismriver>>,
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
