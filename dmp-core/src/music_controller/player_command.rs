use chrono::TimeDelta;
use crossbeam_channel::Sender;
use prismriver::{Prismriver, Volume};

use crate::{
    music_controller::controller::{LibraryCommand, LibraryResponse},
    music_storage::queue::{QueueItem, QueueItemType},
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
            if let Ok(PlayerCommandInput { res_rx, command }) = _mail {
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
                        res_rx
                            .send(PlayerResponse::Empty(res.map_err(|e| e.into())))
                            .await
                            .unwrap();
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
                                    QueueItemType::Song(song) => song.primary_uri().unwrap().0,
                                    _ => unimplemented!(),
                                };

                                let prism_uri =
                                    prismriver::utils::path_to_uri(&uri.as_path().unwrap())
                                        .unwrap();
                                println!("Playing song at path: {:?}", prism_uri);

                                // handle error here for unknown formats
                                player.load_new(&prism_uri).unwrap();
                                player.play();

                                let QueueItemType::Song(np_song) = item.item else {
                                    panic!("This is temporary, handle queueItemTypes at some point")
                                };

                                match item.location {
                                    PlayerLocation::Library => {
                                        let (command, tx) =
                                            LibraryCommandInput::command(LibraryCommand::AllSongs);
                                        // Append next song in library
                                        lib_mail.send(command).await.unwrap();
                                        let LibraryResponse::AllSongs(songs) =
                                            tx.recv().await.unwrap()
                                        else {
                                            continue;
                                        };
                                        let (command, tx) = LibraryCommandInput::command(
                                            LibraryCommand::Song(np_song.uuid),
                                        );
                                        lib_mail.send(command).await.unwrap();
                                        let LibraryResponse::Song(_, i) = tx.recv().await.unwrap()
                                        else {
                                            unreachable!()
                                        };
                                        if let Some(song) = songs.get(i + 49) {
                                            let (command, tx) =
                                                QueueCommandInput::command(QueueCommand::Append(
                                                    QueueItem {
                                                        item: QueueItemType::Song(song.clone()),
                                                        location: item.location,
                                                    },
                                                    false,
                                                ));
                                            queue_mail.send(command).await.unwrap();
                                            let QueueResponse::Empty(Ok(())) =
                                                tx.recv().await.unwrap()
                                            else {
                                                unreachable!()
                                            };
                                        } else {
                                            println!("Library Empty");
                                        }
                                    }
                                    PlayerLocation::Playlist(uuid) => {
                                        let (command, tx) = LibraryCommandInput::command(
                                            LibraryCommand::ExternalPlaylist(uuid),
                                        );
                                        lib_mail.send(command).await.unwrap();
                                        let LibraryResponse::ExternalPlaylist(playlist) =
                                            tx.recv().await.unwrap()
                                        else {
                                            unreachable!()
                                        };
                                        let (command, tx) = LibraryCommandInput::command(
                                            LibraryCommand::PlaylistSong {
                                                list_uuid: playlist.uuid,
                                                item_uuid: np_song.uuid,
                                            },
                                        );
                                        lib_mail.send(command).await.unwrap();
                                        let LibraryResponse::PlaylistSong(_, i) =
                                            tx.recv().await.unwrap()
                                        else {
                                            unreachable!()
                                        };
                                        if let Some(song) = playlist.tracks.get(i + 49) {
                                            let (command, tx) =
                                                QueueCommandInput::command(QueueCommand::Append(
                                                    QueueItem {
                                                        item: QueueItemType::Song(song.clone()),
                                                        location: PlayerLocation::Test,
                                                    },
                                                    false,
                                                ));
                                            queue_mail.send(command).await.unwrap();
                                            let QueueResponse::Empty(Ok(())) =
                                                tx.recv().await.unwrap()
                                            else {
                                                unreachable!()
                                            };
                                        } else {
                                            println!("Playlist Empty");
                                        }
                                    }
                                    _ => todo!(),
                                }
                                res_rx
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
                                res_rx
                                    .send(PlayerResponse::NowPlaying(Err(e.into())))
                                    .await
                                    .unwrap();
                            }
                            _ => continue,
                        }
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
                                res_rx
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
                                res_rx
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
                                res_rx
                                    .send(PlayerResponse::NowPlaying(Ok(song)))
                                    .await
                                    .unwrap();
                            }
                            QueueResponse::Item(Err(e)) => {
                                res_rx
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
                                res_rx
                                    .send(PlayerResponse::NowPlaying(Err(e.into())))
                                    .await
                                    .unwrap();
                                continue;
                            }
                            _ => unreachable!(),
                        }

                        let (command, tx) = QueueCommandInput::command(QueueCommand::Append(
                            QueueItem {
                                item: QueueItemType::Song(np_song.clone()),
                                location: PlayerLocation::Test,
                            },
                            true,
                        ));

                        queue_mail.send(command).await.unwrap();
                        match tx.recv().await.unwrap() {
                            QueueResponse::Empty(Ok(())) => (),
                            QueueResponse::Empty(Err(e)) => {
                                res_rx
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

                        let (songs, index) = match location {
                            PlayerLocation::Library => {
                                let (command, tx) =
                                    LibraryCommandInput::command(LibraryCommand::AllSongs);
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::AllSongs(songs) = tx.recv().await.unwrap()
                                else {
                                    unreachable!()
                                };
                                (songs, index)
                            }
                            PlayerLocation::Playlist(uuid) => {
                                let (command, tx) = LibraryCommandInput::command(
                                    LibraryCommand::ExternalPlaylist(uuid),
                                );
                                lib_mail.send(command).await.unwrap();
                                let LibraryResponse::ExternalPlaylist(list) =
                                    tx.recv().await.unwrap()
                                else {
                                    unreachable!()
                                };
                                let index = list.get_index(np_song.uuid).unwrap();
                                (list.tracks, index)
                            }
                            _ => todo!("Got Location other than Library or Playlist"),
                        };

                        for i in index + 1..(index + 50) {
                            if let Some(song) = songs.get(i) {
                                let (command, tx) =
                                    QueueCommandInput::command(QueueCommand::Append(
                                        QueueItem {
                                            item: QueueItemType::Song(song.clone()),
                                            location: PlayerLocation::Test,
                                        },
                                        false,
                                    ));
                                queue_mail.send(command).await.unwrap();
                                match tx.recv().await.unwrap() {
                                    QueueResponse::Empty(Ok(())) => (),
                                    QueueResponse::Empty(Err(e)) => {
                                        res_rx
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
                        // ^ This be my solution for now ^
                        res_rx
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
