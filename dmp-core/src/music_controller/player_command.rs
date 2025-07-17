use chrono::TimeDelta;
use crossbeam_channel::Sender;
use prismriver::{Prismriver, Volume};

use crate::{
    music_controller::controller::{LibraryCommand, LibraryResponse, PlayerError},
    music_storage::{
        library::Song,
        queue::{QueueItem, QueueItemType, Shuffle, UpNextSong, UpNextSongInner},
    },
};

use super::{
    connections::ConnectionsNotification,
    controller::{
        Controller, ControllerState, PlayerCommand, PlayerLocation, PlayerResponse, QueueCommand,
        QueueResponse,
    },
    controller_handle::{LibraryCommandInput, PlayerCommandInput, QueueCommandInput},
};

impl Controller {
    pub(super) async fn player_command_loop(
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
            if let Ok(PlayerCommandInput {
                res_rx: res_tx,
                command,
            }) = _mail
            {
                match command {
                    PlayerCommand::Play => {
                        player.play();
                        res_tx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Pause => {
                        player.pause();
                        res_tx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Stop => {
                        player.stop();
                        res_tx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();
                    }

                    PlayerCommand::Seek(time) => {
                        let res = player.seek_to(TimeDelta::milliseconds(time));
                        res_tx
                            .send(PlayerResponse::Empty(res.map_err(|e| e.into())))
                            .await
                            .unwrap();
                    }

                    PlayerCommand::SetVolume(volume) => {
                        player.set_volume(Volume::new(volume));
                        res_tx.send(PlayerResponse::Empty(Ok(()))).await.unwrap();

                        // make this async or something
                        state.volume = volume;
                        _ = state.write_file()
                    }

                    PlayerCommand::NextSong => {
                        let (command, rx) = QueueCommandInput::command(QueueCommand::Next);
                        queue_mail.send(command).await.unwrap();

                        let QueueResponse::Next(res) = rx.recv().await.unwrap() else {
                            unimplemented!("QueueResponse should be Next")
                        };

                        let Ok(next) = res else {
                            println!("No Next song found");
                            res_tx
                                .send(PlayerResponse::NowPlaying(Err(
                                    crate::music_storage::queue::QueueError::NoNext,
                                )))
                                .await
                                .unwrap();
                            continue;
                        };

                        if let Some(up_next_item) = next.up_next_item {
                            let song = match up_next_item.inner {
                                UpNextSongInner::Library(uuid) => {
                                    let (command, rx) =
                                        LibraryCommandInput::command(LibraryCommand::Song(uuid));
                                    lib_mail.send(command).await.unwrap();

                                    let LibraryResponse::Song(song, _) = rx.recv().await.unwrap()
                                    else {
                                        unreachable!()
                                    };
                                    song
                                }
                                UpNextSongInner::File(ref path) => {
                                    Song::from_file(path).expect("failed to create song from file")
                                }
                            };
                            let (command, rx) =
                                QueueCommandInput::command(QueueCommand::AddUpNext(QueueItem {
                                    item: QueueItemType::Song(song),
                                    location: up_next_item.location,
                                }));
                            queue_mail.send(command).await.unwrap();
                            let QueueResponse::Empty(res) = rx.recv().await.unwrap() else {
                                unreachable!()
                            };
                            if let Err(e) = res {
                                println!("{e:?}");
                            }
                        }

                        let uri = match &next.item.item {
                            QueueItemType::Song(song) => song.primary_uri().unwrap().0,
                            _ => unimplemented!("Album lookup comes later"),
                        };

                        let prism_uri =
                            prismriver::utils::path_to_uri(&uri.as_path().unwrap()).unwrap();
                        println!("Playing song at path: {prism_uri:?}");

                        let np_song = match next.item.item {
                            QueueItemType::Song(s) => s,
                            _ => unimplemented!(),
                        };
                        // handle error here for unknown formats
                        player.load_new(&prism_uri).unwrap();
                        player.play();
                        res_tx
                            .send(PlayerResponse::NowPlaying(Ok(np_song.clone())))
                            .await
                            .unwrap();

                        notify_connections_
                            .send(ConnectionsNotification::SongChange(np_song))
                            .unwrap();
                    }

                    PlayerCommand::PrevSong => {
                        let (command, tx) = QueueCommandInput::command(QueueCommand::Prev);
                        queue_mail.send(command).await.unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Item(Ok(item)) => {
                                let uri = match &item.item {
                                    QueueItemType::Song(song) => song.primary_uri().unwrap().0,
                                    _ => unimplemented!(),
                                };

                                let prism_uri =
                                    prismriver::utils::path_to_uri(&uri.as_path().unwrap())
                                        .unwrap();
                                player.load_new(&prism_uri).unwrap();
                                player.play();

                                let QueueItemType::Song(np_song) = item.item else {
                                    panic!("This is temporary, handle queueItemTypes at some point")
                                };
                                res_tx
                                    .send(PlayerResponse::NowPlaying(Ok(np_song.clone())))
                                    .await
                                    .unwrap();

                                state.now_playing = np_song.uuid;
                                _ = state.write_file();
                                notify_connections_
                                    .send(ConnectionsNotification::SongChange(np_song))
                                    .unwrap();
                            }
                            QueueResponse::Item(Err(e)) => {
                                res_tx
                                    .send(PlayerResponse::NowPlaying(Err(e.into())))
                                    .await
                                    .unwrap();
                            }
                            _ => continue,
                        }
                    }

                    PlayerCommand::Enqueue(index) => {
                        let (command, tx) =
                            QueueCommandInput::command(QueueCommand::GetIndex(index));
                        queue_mail.send(command).await.unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Item(Ok(item)) => {
                                let song = match item.item {
                                    QueueItemType::Song(np_song) => {
                                        let prism_uri = prismriver::utils::path_to_uri(
                                            &np_song.primary_uri().unwrap().0.as_path().unwrap(),
                                        )
                                        .unwrap();
                                        player.load_new(&prism_uri).unwrap();
                                        player.play();

                                        state.now_playing = np_song.uuid;
                                        _ = state.write_file();

                                        notify_connections_
                                            .send(ConnectionsNotification::SongChange(
                                                np_song.clone(),
                                            ))
                                            .unwrap();
                                        np_song
                                    }
                                    _ => unimplemented!(),
                                };
                                res_tx
                                    .send(PlayerResponse::NowPlaying(Ok(song)))
                                    .await
                                    .unwrap();
                            }
                            QueueResponse::Item(Err(e)) => {
                                res_tx
                                    .send(PlayerResponse::NowPlaying(Err(e.into())))
                                    .await
                                    .unwrap();
                            }
                            _ => continue,
                        }
                    }

                    PlayerCommand::PlayNow(uuid, location) => {
                        // TODO: This assumes the uuid doesn't point to an album. we've been over this.
                        let (command, tx) =
                            LibraryCommandInput::command(LibraryCommand::Song(uuid));
                        lib_mail.send(command).await.unwrap();
                        let LibraryResponse::Song(np_song, index) = tx.recv().await.unwrap() else {
                            unreachable!()
                        };

                        let (command, tx) = QueueCommandInput::command(QueueCommand::Clear);
                        queue_mail.send(command).await.unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Empty(Ok(())) => (),
                            QueueResponse::Empty(Err(e)) => {
                                res_tx
                                    .send(PlayerResponse::NowPlaying(Err(e.into())))
                                    .await
                                    .unwrap();
                                continue;
                            }
                            _ => unreachable!(),
                        }

                        let (command, tx) =
                            QueueCommandInput::command(QueueCommand::Append(QueueItem {
                                item: QueueItemType::Song(np_song.clone()),
                                location: PlayerLocation::Test,
                            }));

                        queue_mail.send(command).await.unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Empty(Ok(())) => (),
                            QueueResponse::Empty(Err(e)) => {
                                res_tx
                                    .send(PlayerResponse::NowPlaying(Err(e.into())))
                                    .await
                                    .unwrap();
                                continue;
                            }
                            _ => unreachable!(),
                        }

                        // TODO: Handle non Local URIs here, and whenever `load_new()` or `load_gapless()` is called
                        let prism_uri = prismriver::utils::path_to_uri(
                            &np_song.primary_uri().unwrap().0.as_path().unwrap(),
                        )
                        .unwrap();
                        player.load_new(&prism_uri).unwrap();
                        player.play();

                        // how grab all the songs in a certain subset of the library, I reckon?

                        // ...
                        // let's just pretend I figured that out already

                        let (command, rx) = QueueCommandInput::command(QueueCommand::Info);
                        queue_mail.send(command).await.unwrap();
                        let QueueResponse::Info {
                            shuffle,
                            up_next_limit,
                            ..
                        } = rx.recv().await.unwrap()
                        else {
                            unreachable!()
                        };

                        let (mut uuids, index) = match location {
                            PlayerLocation::Library => {
                                let (command, tx) =
                                    LibraryCommandInput::command(LibraryCommand::AllUuids);
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::AllUuids(uuids) = tx.recv().await.unwrap()
                                else {
                                    unreachable!()
                                };
                                (uuids, index)
                            }
                            PlayerLocation::Playlist(uuid) => {
                                let (command, tx) = LibraryCommandInput::command(
                                    LibraryCommand::FilteredPlaylist(uuid),
                                );
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::FilteredPlaylist(list) =
                                    tx.recv().await.unwrap()
                                else {
                                    unreachable!()
                                };
                                let mut index = 0;
                                for (i, uuid) in list.iter().enumerate() {
                                    if &np_song.uuid == uuid {
                                        index = i;
                                    }
                                }
                                // It crashes if I don't do this. I don't know why
                                // somehow the returned playlist uuids don't match up unless I do
                                (list, index)
                            }
                            _ => todo!("Got Location other than Library or Playlist"),
                        };

                        if let Shuffle::NoShuffle = shuffle {
                            for _ in 0..(index + 1) {
                                _ = uuids.remove(0)
                            }
                        }
                        for _ in 0..up_next_limit {
                            if !uuids.is_empty() {
                                let uuid = uuids.remove(0);
                                let (command, rx) =
                                    LibraryCommandInput::command(LibraryCommand::Song(uuid));
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::Song(song, _) = rx.recv().await.unwrap()
                                else {
                                    unreachable!()
                                };

                                let (command, rx) = QueueCommandInput::command(
                                    QueueCommand::AddUpNext(QueueItem {
                                        item: QueueItemType::Song(song.clone()),
                                        location: PlayerLocation::Test,
                                    }),
                                );
                                queue_mail.send(command).await.unwrap();
                                match rx.recv().await.unwrap() {
                                    QueueResponse::Empty(Ok(())) => (),
                                    QueueResponse::Empty(Err(e)) => {
                                        res_tx
                                            .send(PlayerResponse::NowPlaying(Err(e.into())))
                                            .await
                                            .unwrap();
                                        continue 'outer;
                                    }
                                    _ => unreachable!(),
                                }
                            } else {
                                println!("End of Library / Playlist");
                                break;
                            }
                        }

                        let (command, rx) =
                            QueueCommandInput::command(QueueCommand::AddUpNextInvis(
                                uuids
                                    .into_iter()
                                    .map(|uuid| UpNextSong {
                                        inner: UpNextSongInner::Library(uuid),
                                        location,
                                    })
                                    .collect(),
                            ));
                        queue_mail.send(command).await.unwrap();
                        let QueueResponse::Empty(res) = rx.recv().await.unwrap() else {
                            unreachable!()
                        };

                        // ^ This be my solution for now ^
                        res_tx
                            .send(PlayerResponse::NowPlaying(Ok(np_song.clone())))
                            .await
                            .unwrap();

                        state.now_playing = np_song.uuid;
                        _ = state.write_file();
                        notify_connections_
                            .send(ConnectionsNotification::SongChange(np_song))
                            .unwrap();
                    }
                }
            } else {
                return Err(());
            }
        }
        Ok(())
    }
}
