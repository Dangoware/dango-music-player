//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions
#![allow(while_true)]

use async_channel::{bounded, unbounded};
use chrono::TimeDelta;
use crossbeam::atomic::AtomicCell;
use crossbeam_channel::{Receiver, Sender};
use kushi::{Queue, QueueItemType};
use kushi::{QueueError, QueueItem};
use parking_lot::RwLock;
use prismriver::{Error as PrismError, Prismriver, State as PrismState, Volume};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::collections::HashMap;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::config::ConfigError;
use crate::music_storage::library::Song;
use crate::music_storage::playlist::{ExternalPlaylist, Playlist, PlaylistFolderItem};
use crate::{config::Config, music_storage::library::MusicLibrary};

use super::connections::{ConnectionsInput, ConnectionsNotification, ControllerConnections};
use super::queue::{QueueAlbum, QueueSong};

pub struct Controller();

type QueueItem_ = QueueItem<QueueSong, QueueAlbum>;

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
    Stop,
    Seek(i64),
    Enqueue(usize),
    SetVolume(f32),
    PlayNow(Uuid, PlayerLocation),
}

#[derive(Debug, PartialEq, Clone)]
pub enum PlayerResponse {
    Empty(Result<(), PlayerError>),
    NowPlaying(Result<Song, QueueError>)
}

#[derive(Error, Debug, PartialEq, Clone)]
pub enum PlayerError {
    #[error("{0}")]
    QueueError(#[from] QueueError),
    #[error("{0}")]
    Prismriver(#[from] PrismError),
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
    Append(QueueItem_, bool),
    Next,
    Prev,
    GetIndex(usize),
    NowPlaying,
    Get,
    Clear,
    Remove(usize),
}

#[derive(Debug, PartialEq, Clone)]
pub enum QueueResponse {
    Empty(Result<(), QueueError>),
    Item(Result<QueueItem_, QueueError>),
    GetAll(Vec<QueueItem_>),
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
    playback_info: Arc<AtomicCell<PlaybackInfo>>,
    notify_next_song: Sender<Song>,
    connections: Option<ConnectionsInput>
}

pub struct ControllerHandle {
    pub lib_mail: MailMan<LibraryCommand, LibraryResponse>,
    pub player_mail: MailMan<PlayerCommand, PlayerResponse>,
    pub queue_mail: MailMan<QueueCommand, QueueResponse>,
}

impl ControllerHandle {
    pub fn new(library: MusicLibrary, config: Arc<RwLock<Config>>, connections: Option<ConnectionsInput>) -> (Self, ControllerInput, Arc<AtomicCell<PlaybackInfo>>, Receiver<Song>) {
        let lib_mail = MailMan::double();
        let player_mail = MailMan::double();
        let queue_mail = MailMan::double();
        let playback_info = Arc::new(AtomicCell::new(PlaybackInfo::default()));
        let notify_next_song = crossbeam::channel::unbounded::<Song>();
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
                config,
                playback_info: Arc::clone(&playback_info),
                notify_next_song: notify_next_song.0,
                connections,
            },
            playback_info,
            notify_next_song.1
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
            config,
            playback_info,
            notify_next_song,
            connections,
        }: ControllerInput
    ) -> Result<(), Box<dyn Error>> {
        let queue: Queue<QueueSong, QueueAlbum> = Queue {
            items: Vec::new(),
            played: Vec::new(),
            loop_: false,
            shuffle: None,
        };

        let state = {
            let path = &config.read().state_path;
            if let Ok(state) = ControllerState::read_file(path) {
                state
            } else {
                ControllerState::new(path.clone())
            }
        };

        std::thread::scope(|scope| {
            let player = Prismriver::new();
            let player_state = player.state.clone();
            let player_timing = player.get_timing_recv();
            let finished_tx = player.get_finished_recv();
            let (notifications_rx, notifications_tx) = crossbeam_channel::unbounded::<ConnectionsNotification>();

            let a = scope.spawn({
                let queue_mail = queue_mail.clone();
                move || {
                    futures::executor::block_on(async {
                        moro::async_scope!(|scope| {
                            println!("async scope created");

                            let _lib_mail = lib_mail.0.clone();
                            let _queue_mail = queue_mail.0.clone();
                            scope
                                .spawn(async move {
                                    Controller::player_command_loop(
                                        player,
                                        player_mail.1,
                                        _queue_mail,
                                        _lib_mail,
                                        state,
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
                }
            });

            let b = scope.spawn(|| {
                futures::executor::block_on(async {
                    Controller::queue_loop(queue, queue_mail.1).await;
                })
            });

            let c = scope.spawn(|| {
                Controller::player_monitor_loop(
                    player_state,
                    player_timing,
                    finished_tx,
                    player_mail.0,
                    queue_mail.0,
                    notify_next_song,
                    notifications_rx,
                    playback_info,
                ).unwrap();
            });

            if let Some(inner) = connections {
                dbg!(&inner);
                let d = scope.spawn(|| {
                    Controller::handle_connections( ControllerConnections {
                        notifications_tx,
                        inner,
                    });
                });
            }
            a.join().unwrap();
            b.join().unwrap();
            c.join().unwrap();
        });

        Ok(())
    }

    async fn player_command_loop(
        mut player: Prismriver,
        player_mail: MailMan<PlayerResponse, PlayerCommand>,
        queue_mail: MailMan<QueueCommand, QueueResponse>,
        lib_mail: MailMan<LibraryCommand, LibraryResponse>,
        mut state: ControllerState,
    ) -> Result<(), ()> {
        player.set_volume(Volume::new(state.volume));
        'outer: while true {
            let _mail = player_mail.recv().await;
            if let Ok(mail) = _mail {
                match mail {
                    PlayerCommand::Play => {
                        player.play();
                        player_mail.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Pause => {
                        player.pause();
                        player_mail.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Stop => {
                        player.stop();
                        player_mail.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Seek(time) => {
                        let res = player.seek_to(TimeDelta::milliseconds(time));
                        player_mail.send(PlayerResponse::Empty(res.map_err(|e| e.into()))).await.unwrap();
                    }

                    PlayerCommand::SetVolume(volume) => {
                        player.set_volume(Volume::new(volume));
                        player_mail.send(PlayerResponse::Empty(Ok(()))).await.unwrap();

                        // make this async or something
                        state.volume = volume;
                        _ = state.write_file()
                    }

                    PlayerCommand::NextSong => {
                        queue_mail.send(QueueCommand::Next).await.unwrap();

                        match queue_mail.recv().await.unwrap() {
                            QueueResponse::Item(Ok(item)) => {
                                let uri = match &item.item {
                                    QueueItemType::Single(song) => song.song.primary_uri().unwrap().0,
                                    _ => unimplemented!(),
                                };

                                let prism_uri = prismriver::utils::path_to_uri(&uri.as_path().unwrap()).unwrap();
                                println!("Playing song at path: {:?}", prism_uri);

                                // handle error here for unknown formats
                                player.load_new(&prism_uri).unwrap();
                                player.play();

                                let QueueItemType::Single(np_song) = item.item else { panic!("This is temporary, handle queueItemTypes at some point")};

                                // Append next song in library
                                lib_mail.send(LibraryCommand::AllSongs).await.unwrap();
                                let LibraryResponse::AllSongs(songs) = lib_mail.recv().await.unwrap() else {
                                    continue;
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
                                    let QueueResponse::Empty(Ok(())) = queue_mail.recv().await.unwrap() else {
                                        unreachable!()
                                    };
                                } else {
                                    println!("Library Empty");
                                }

                                player_mail.send(PlayerResponse::NowPlaying(Ok(np_song.song.clone()))).await.unwrap();

                                state.now_playing = np_song.song.uuid;
                                _ = state.write_file();
                            } QueueResponse::Item(Err(e)) => {
                                player_mail.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                            }
                            _ => continue
                        }
                    }

                    PlayerCommand::PrevSong => {
                        queue_mail.send(QueueCommand::Prev).await.unwrap();
                        match queue_mail.recv().await.unwrap() {
                            QueueResponse::Item(Ok(item)) => {
                                let uri = match &item.item {
                                    QueueItemType::Single(song) => song.song.primary_uri().unwrap().0,
                                    _ => unimplemented!(),
                                };

                                let prism_uri = prismriver::utils::path_to_uri(&uri.as_path().unwrap()).unwrap();
                                player.load_new(&prism_uri).unwrap();
                                player.play();

                                let QueueItemType::Single(np_song) = item.item else { panic!("This is temporary, handle queueItemTypes at some point")};
                                player_mail.send(PlayerResponse::NowPlaying(Ok(np_song.song.clone()))).await.unwrap();

                                state.now_playing = np_song.song.uuid;
                                _ = state.write_file();
                            }
                            QueueResponse::Item(Err(e)) => {
                                player_mail.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                            }
                            _ => continue
                        }
                    }

                    PlayerCommand::Enqueue(index) => {
                        queue_mail
                            .send(QueueCommand::GetIndex(index))
                            .await
                            .unwrap();
                        match queue_mail.recv().await.unwrap() {
                            QueueResponse::Item(Ok(item)) => {
                                match item.item {
                                    QueueItemType::Single(song) => {
                                        let prism_uri = prismriver::utils::path_to_uri(&song.song.primary_uri().unwrap().0.as_path().unwrap()).unwrap();
                                        player.load_new(&prism_uri).unwrap();
                                        player.play();
                                    }
                                    _ => unimplemented!(),
                                }
                                player_mail.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                            }
                            QueueResponse::Item(Err(e)) => {
                                player_mail.send(PlayerResponse::Empty(Err(e.into()))).await.unwrap();
                            }
                            _ => continue
                        }
                    }

                    PlayerCommand::PlayNow(uuid, location) => {
                        // TODO: This assumes the uuid doesn't point to an album. we've been over this.
                        lib_mail.send(LibraryCommand::Song(uuid)).await.unwrap();
                        let LibraryResponse::Song(song, index) = lib_mail.recv().await.unwrap() else {
                            unreachable!()
                        };
                        queue_mail.send(QueueCommand::Clear).await.unwrap();
                        match queue_mail.recv().await.unwrap() {
                            QueueResponse::Empty(Ok(())) => (),
                            QueueResponse::Empty(Err(e)) => {
                                player_mail.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                                continue;
                            }
                            _ => unreachable!()
                        }
                        queue_mail.send(QueueCommand::Append(QueueItem::from_item_type(QueueItemType::Single(QueueSong { song: song.clone(), location })), true)).await.unwrap();
                        match queue_mail.recv().await.unwrap() {
                            QueueResponse::Empty(Ok(())) => (),
                            QueueResponse::Empty(Err(e)) => {
                                player_mail.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                                continue;
                            }
                            _ => unreachable!()
                        }

                        // TODO: Handle non Local URIs here, and whenever `load_new()` or `load_gapless()` is called
                        let prism_uri = prismriver::utils::path_to_uri(&song.primary_uri().unwrap().0.as_path().unwrap()).unwrap();
                        player.load_new(&prism_uri).unwrap();
                        player.play();

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
                                match queue_mail.recv().await.unwrap() {
                                    QueueResponse::Empty(Ok(())) => (),
                                    QueueResponse::Empty(Err(e)) => {
                                        player_mail.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                                        continue 'outer;
                                    }
                                    _ => unreachable!()
                                }
                            } else {
                                println!("End of Library / Playlist");
                                break;
                            }
                        }
                        // ^ This be my solution for now ^
                        player_mail.send(PlayerResponse::NowPlaying(Ok(song.clone()))).await.unwrap();
                    }
                }
            } else {
                return Err(());
            }
        }
        Ok(())
    }

    fn player_monitor_loop(
        playback_state: Arc<std::sync::RwLock<PrismState>>,
        playback_time_tx: Receiver<(Option<TimeDelta>, Option<TimeDelta>)>,
        finished_recv: Receiver<()>,
        player_mail: MailMan<PlayerCommand, PlayerResponse>,
        queue_mail: MailMan<QueueCommand, QueueResponse>,
        notify_next_song: Sender<Song>,
        notify_connections_: Sender<ConnectionsNotification>,
        playback_info: Arc<AtomicCell<PlaybackInfo>>
    ) -> Result<(), ()> {
        std::thread::scope(|s| {
            // Thread for timing and metadata
            let notify_connections = notify_connections_.clone();
            s.spawn({
                move || {
                    println!("playback monitor started");
                    while true {
                        let (position, duration) = playback_time_tx.recv().unwrap();
                        notify_connections.send(ConnectionsNotification::Playback { position: position.clone(), duration: duration.clone() }).unwrap();
                        playback_info.store(PlaybackInfo { position, duration });
                    }
                }
            });

            // Thread for End of Track
            let notify_connections = notify_connections_.clone();
            s.spawn(move || { futures::executor::block_on(async {
                println!("EOS monitor started");
                while true {
                    let _ = finished_recv.recv();
                    println!("End of song");

                    player_mail.send(PlayerCommand::NextSong).await.unwrap();
                    let PlayerResponse::NowPlaying(res) = player_mail.recv().await.unwrap() else {
                        unreachable!()
                    };
                    if let Ok(song) = res {
                        notify_next_song.send(song.clone()).unwrap();
                        notify_connections.send(ConnectionsNotification::SongChange(song)).unwrap();
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            });});

            let notify_connections = notify_connections_.clone();
            s.spawn(move || {
                let mut state = PrismState::Stopped;
                while true {
                    let _state = playback_state.read().unwrap().to_owned();
                    if  _state != state {
                        state = _state;
                        println!("State Changed to {state:?}");
                        notify_connections.send(ConnectionsNotification::StateChange(state.clone())).unwrap();
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        });



        println!("Monitor Loops Ended");
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
                    library.save(config.read().libraries.get_library(&library.uuid).unwrap().path.clone()).unwrap();
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
                        .send(QueueResponse::Empty(Ok(())))
                        .await
                        .unwrap();
                },
                QueueCommand::Next => {
                    let next = queue.next().map_or( Err(QueueError::NoNext), |s| Ok(s.clone()));
                    queue_mail
                        .send(QueueResponse::Item(next.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::Prev => {
                    let prev = queue.prev().map_or( Err(QueueError::EmptyPlayed), |s| Ok(s.clone()));
                    queue_mail
                        .send(QueueResponse::Item(prev.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::GetIndex(index) => {
                    let item = queue.items.get(index).map_or( Err(QueueError::OutOfBounds { index, len: queue.items.len() }), |s| Ok(s.clone()));
                    queue_mail.send(QueueResponse::Item(item)).await.unwrap();
                }
                QueueCommand::NowPlaying => {
                    let item = queue.current().map(|t| t.clone());
                    queue_mail
                        .send(QueueResponse::Item(item))
                        .await
                        .unwrap();
                }
                QueueCommand::Get => {
                    queue_mail.send(QueueResponse::GetAll(queue.items.clone())).await.unwrap();
                }
                QueueCommand::Clear => {
                    queue.clear();
                    queue_mail.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::Remove(index) => {
                    queue_mail.send(QueueResponse::Item(queue.remove_item(index))).await.unwrap();
                }
            }
        }
    }
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct PlaybackInfo {
    pub position: Option<TimeDelta>,
    pub duration: Option<TimeDelta>,
}
