//! The [Controller] is the input and output for the entire
//! player. It manages queues, playback, library access, and
//! other functions
#![allow(while_true)]

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
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::config::ConfigError;
use crate::music_storage::library::Song;
use crate::music_storage::playlist::{ExternalPlaylist, Playlist, PlaylistFolderItem};
use crate::{config::Config, music_storage::library::MusicLibrary};

use super::connections::{ConnectionsInput, ConnectionsNotification, ControllerConnections};
use super::controller_handle::{LibraryCommandInput, PlayerCommandInput, QueueCommandInput};
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
        async_channel::Sender<PlayerCommandInput>,
        async_channel::Receiver<PlayerCommandInput>
    ),
    lib_mail: (
        async_channel::Sender<LibraryCommandInput>,
        async_channel::Receiver<LibraryCommandInput>
    ),
    queue_mail: (
        async_channel::Sender<QueueCommandInput>,
        async_channel::Receiver<QueueCommandInput>
    ),
    library: MusicLibrary,
    config: Arc<RwLock<Config>>,
    playback_info: Arc<AtomicCell<PlaybackInfo>>,
    notify_next_song: Sender<Song>,
    connections: Option<ConnectionsInput>
}

pub struct ControllerHandle {
    pub(super) lib_mail_rx: async_channel::Sender<LibraryCommandInput>,
    pub(super) player_mail_rx: async_channel::Sender<PlayerCommandInput>,
    pub(super)  queue_mail_rx: async_channel::Sender<QueueCommandInput>,
}

impl ControllerHandle {
    pub fn new(library: MusicLibrary, config: Arc<RwLock<Config>>, connections: Option<ConnectionsInput>) -> (Self, ControllerInput, Arc<AtomicCell<PlaybackInfo>>, Receiver<Song>) {
        let (lib_mail_rx, lib_mail_tx) = async_channel::unbounded();
        let (player_mail_rx, player_mail_tx) = async_channel::unbounded();
        let (queue_mail_rx, queue_mail_tx)= async_channel::unbounded();
        let playback_info = Arc::new(AtomicCell::new(PlaybackInfo::default()));
        let notify_next_song = crossbeam::channel::unbounded::<Song>();
        (
            ControllerHandle {
                lib_mail_rx: lib_mail_rx.clone(),
                player_mail_rx: player_mail_rx.clone(),
                queue_mail_rx: queue_mail_rx.clone()
            },
            ControllerInput {
                player_mail: (player_mail_rx, player_mail_tx),
                lib_mail: (lib_mail_rx, lib_mail_tx),
                queue_mail: (queue_mail_rx, queue_mail_tx),
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
                let _notifications_rx = notifications_rx.clone();
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
                                        _notifications_rx,
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
        player_mail: async_channel::Receiver<PlayerCommandInput>,
        queue_mail: async_channel::Sender<QueueCommandInput>,
        lib_mail: async_channel::Sender<LibraryCommandInput>,
        notify_connections_: Sender<ConnectionsNotification>,
        mut state: ControllerState,
    ) -> Result<(), ()> {
        player.set_volume(Volume::new(state.volume));
        'outer: while true {
            let _mail = player_mail.recv().await;
            if let Ok(PlayerCommandInput {res_rx, command}) = _mail {
                match command {
                    PlayerCommand::Play => {
                        player.play();
                        res_rx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Pause => {
                        player.pause();
                        res_rx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Stop => {
                        player.stop();
                        res_rx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Seek(time) => {
                        let res = player.seek_to(TimeDelta::milliseconds(time));
                        res_rx.send(PlayerResponse::Empty(res.map_err(|e| e.into()))).await.unwrap();
                    }

                    PlayerCommand::SetVolume(volume) => {
                        player.set_volume(Volume::new(volume));
                        res_rx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();

                        // make this async or something
                        state.volume = volume;
                        _ = state.write_file()
                    }

                    PlayerCommand::NextSong => {
                        let (command, tx) = QueueCommandInput::command(QueueCommand::Next);
                        queue_mail.send(command).await.unwrap();

                        match tx.recv().await.unwrap() {
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

                                let (command, tx) = LibraryCommandInput::command(LibraryCommand::AllSongs);
                                // Append next song in library
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::AllSongs(songs) = tx.recv().await.unwrap() else {
                                    continue;
                                };

                                let (command, tx) = LibraryCommandInput::command(LibraryCommand::Song(np_song.song.uuid));
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::Song(_, i) = tx.recv().await.unwrap() else {
                                    unreachable!()
                                };
                                if let Some(song) = songs.get(i + 49) {
                                    let (command, tx) = QueueCommandInput::command(
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
                                    );
                                    queue_mail.send(command).await
                                    .unwrap();
                                    let QueueResponse::Empty(Ok(())) = tx.recv().await.unwrap() else {
                                        unreachable!()
                                    };
                                } else {
                                    println!("Library Empty");
                                }

                                res_rx.send(PlayerResponse::NowPlaying(Ok(np_song.song.clone()))).await.unwrap();

                                state.now_playing = np_song.song.uuid;
                                _ = state.write_file();
                                notify_connections_.send(ConnectionsNotification::SongChange(np_song.song)).unwrap();
                            } QueueResponse::Item(Err(e)) => {
                                res_rx.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                            }
                            _ => continue
                        }
                    }

                    PlayerCommand::PrevSong => {
                        let (command, tx) = QueueCommandInput::command(QueueCommand::Prev);
                        queue_mail.send(command).await.unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Item(Ok(item)) => {
                                let uri = match &item.item {
                                    QueueItemType::Single(song) => song.song.primary_uri().unwrap().0,
                                    _ => unimplemented!(),
                                };

                                let prism_uri = prismriver::utils::path_to_uri(&uri.as_path().unwrap()).unwrap();
                                player.load_new(&prism_uri).unwrap();
                                player.play();

                                let QueueItemType::Single(np_song) = item.item else { panic!("This is temporary, handle queueItemTypes at some point")};
                                res_rx.send(PlayerResponse::NowPlaying(Ok(np_song.song.clone()))).await.unwrap();

                                state.now_playing = np_song.song.uuid;
                                _ = state.write_file();
                                notify_connections_.send(ConnectionsNotification::SongChange(np_song.song)).unwrap();
                            }
                            QueueResponse::Item(Err(e)) => {
                                res_rx.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                            }
                            _ => continue
                        }
                    }

                    PlayerCommand::Enqueue(index) => {
                        let (command, tx) = QueueCommandInput::command(QueueCommand::GetIndex(index));
                        queue_mail
                            .send(command)
                            .await
                            .unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Item(Ok(item)) => {
                                match item.item {
                                    QueueItemType::Single(np_song) => {
                                        let prism_uri = prismriver::utils::path_to_uri(&np_song.song.primary_uri().unwrap().0.as_path().unwrap()).unwrap();
                                        player.load_new(&prism_uri).unwrap();
                                        player.play();

                                        state.now_playing = np_song.song.uuid;
                                        _ = state.write_file();
                                        notify_connections_.send(ConnectionsNotification::SongChange(np_song.song)).unwrap();
                                    }
                                    _ => unimplemented!(),
                                }
                                res_rx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                            }
                            QueueResponse::Item(Err(e)) => {
                                res_rx.send(PlayerResponse::Empty(Err(e.into()))).await.unwrap();
                            }
                            _ => continue
                        }
                    }

                    PlayerCommand::PlayNow(uuid, location) => {
                        // TODO: This assumes the uuid doesn't point to an album. we've been over this.
                        let (command, tx) = LibraryCommandInput::command(LibraryCommand::Song(uuid));
                        lib_mail.send(command).await.unwrap();
                        let LibraryResponse::Song(np_song, index) = tx.recv().await.unwrap() else {
                            unreachable!()
                        };

                        let (command, tx) = QueueCommandInput::command(QueueCommand::Clear);
                        queue_mail.send(command).await.unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Empty(Ok(())) => (),
                            QueueResponse::Empty(Err(e)) => {
                                res_rx.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                                continue;
                            }
                            _ => unreachable!()
                        }

                        let (command, tx) = QueueCommandInput::command(
                            QueueCommand::Append(QueueItem::from_item_type(QueueItemType::Single(QueueSong { song: np_song.clone(), location })), true)
                        );
                        queue_mail.send(command).await.unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Empty(Ok(())) => (),
                            QueueResponse::Empty(Err(e)) => {
                                res_rx.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
                                continue;
                            }
                            _ => unreachable!()
                        }

                        // TODO: Handle non Local URIs here, and whenever `load_new()` or `load_gapless()` is called
                        let prism_uri = prismriver::utils::path_to_uri(&np_song.primary_uri().unwrap().0.as_path().unwrap()).unwrap();
                        player.load_new(&prism_uri).unwrap();
                        player.play();

                        // how grab all the songs in a certain subset of the library, I reckon?
                        // ...
                        // let's just pretend I figured that out already

                        let (songs, index) = match location {
                            PlayerLocation::Library => {
                                let (command, tx) = LibraryCommandInput::command(LibraryCommand::AllSongs);
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::AllSongs(songs) = tx.recv().await.unwrap() else {
                                    unreachable!()
                                };
                                (songs, index)
                            }
                            PlayerLocation::Playlist(uuid) => {
                                let (command, tx) = LibraryCommandInput::command(LibraryCommand::ExternalPlaylist(uuid));
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::ExternalPlaylist(list) = tx.recv().await.unwrap() else {
                                    unreachable!()
                                };
                                let index = list.get_index(np_song.uuid).unwrap();
                                (list.tracks, index)
                            }
                            _ => todo!("Got Location other than Library or Playlist")
                        };


                        for i in index+1..(index+50) {
                            if let Some(song) = songs.get(i) {
                                let (command, tx) = QueueCommandInput::command(
                                    QueueCommand::Append(QueueItem::from_item_type(QueueItemType::Single(QueueSong { song: song.clone(), location })), false)
                                );
                                queue_mail.send(command).await.unwrap();
                                match tx.recv().await.unwrap() {
                                    QueueResponse::Empty(Ok(())) => (),
                                    QueueResponse::Empty(Err(e)) => {
                                        res_rx.send(PlayerResponse::NowPlaying(Err(e.into()))).await.unwrap();
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
                        res_rx.send(PlayerResponse::NowPlaying(Ok(np_song.clone()))).await.unwrap();

                        state.now_playing = np_song.uuid;
                        _ = state.write_file();
                        notify_connections_.send(ConnectionsNotification::SongChange(np_song)).unwrap();
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
        player_mail: async_channel::Sender<PlayerCommandInput>,
        queue_mail: async_channel::Sender<QueueCommandInput>,
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

                    let (command, tx) = PlayerCommandInput::command(PlayerCommand::NextSong);
                    player_mail.send(command).await.unwrap();
                    let PlayerResponse::NowPlaying(res) = tx.recv().await.unwrap() else {
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
        lib_mail: async_channel::Receiver<LibraryCommandInput>,
        library: &mut MusicLibrary,
        config: Arc<RwLock<Config>>,
    ) -> Result<(), ()> {
        while true {
            let LibraryCommandInput { res_rx, command } = lib_mail.recv().await.unwrap();
            match command {
                LibraryCommand::Song(uuid) => {
                    let (song, i) = library.query_uuid(&uuid).unwrap();
                    res_rx.send(LibraryResponse::Song(song.clone(), i)).await.unwrap();
                }
                LibraryCommand::AllSongs => {
                    res_rx.send(LibraryResponse::AllSongs(library.library.clone())).await.unwrap();
                },
                LibraryCommand::ExternalPlaylist(uuid) => {
                    let playlist = library.query_playlist_uuid(&uuid).unwrap();
                    res_rx.send(LibraryResponse::ExternalPlaylist(ExternalPlaylist::from_playlist(playlist, library))).await.unwrap();
                }
                LibraryCommand::ImportM3UPlayList(path) => {
                    let playlist = Playlist::from_m3u(path, library).unwrap();
                    let uuid = playlist.uuid;
                    let name = playlist.title.clone();
                    library.playlists.items.push(PlaylistFolderItem::List(playlist));

                    res_rx.send(LibraryResponse::ImportM3UPlayList(uuid, name)).await.unwrap();
                }
                LibraryCommand::Save => {
                    library.save(config.read().libraries.get_library(&library.uuid).unwrap().path.clone()).unwrap();
                    res_rx.send(LibraryResponse::Ok).await.unwrap();
                }
                LibraryCommand::Playlists => {
                    let mut lists = vec![];
                    library.playlists.lists_recursive().into_par_iter().map(|list| (list.uuid, list.title.clone())).collect_into_vec(&mut lists);

                    res_rx.send(LibraryResponse::Playlists(lists)).await.unwrap();
                }
                _ => { todo!() }
            }
        }
        Ok(())
    }

    async fn queue_loop(
        mut queue: Queue<QueueSong, QueueAlbum>,
        queue_mail: async_channel::Receiver<QueueCommandInput>,
    ) {
        while true {
            let QueueCommandInput { res_rx, command } = queue_mail.recv().await.unwrap();
            match command {
                QueueCommand::Append(item, by_human) => {
                    match item.item {
                        QueueItemType::Single(song) => queue.add_item(song, by_human),
                        _ => unimplemented!(),
                    }
                    res_rx
                        .send(QueueResponse::Empty(Ok(())))
                        .await
                        .unwrap();
                },
                QueueCommand::Next => {
                    let next = queue.next().map_or( Err(QueueError::NoNext), |s| Ok(s.clone()));
                    res_rx
                        .send(QueueResponse::Item(next.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::Prev => {
                    let prev = queue.prev().map_or( Err(QueueError::EmptyPlayed), |s| Ok(s.clone()));
                    res_rx
                        .send(QueueResponse::Item(prev.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::GetIndex(index) => {
                    let item = queue.items.get(index).map_or( Err(QueueError::OutOfBounds { index, len: queue.items.len() }), |s| Ok(s.clone()));
                    res_rx.send(QueueResponse::Item(item)).await.unwrap();
                }
                QueueCommand::NowPlaying => {
                    let item = queue.current().map(|t| t.clone());
                    res_rx
                        .send(QueueResponse::Item(item))
                        .await
                        .unwrap();
                }
                QueueCommand::Get => {
                    res_rx.send(QueueResponse::GetAll(queue.items.clone())).await.unwrap();
                }
                QueueCommand::Clear => {
                    queue.clear();
                    res_rx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::Remove(index) => {
                    res_rx.send(QueueResponse::Item(queue.remove_item(index))).await.unwrap();
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
