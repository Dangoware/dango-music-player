use std::{sync::Arc, time::Duration};

use chrono::TimeDelta;
use crossbeam::atomic::AtomicCell;
use crossbeam_channel::{Receiver, Sender};
use prismriver::State as PrismState;

use crate::{
    music_controller::controller::{PlayerCommand, PlayerResponse},
    music_storage::library::Song,
};

use super::{
    connections::ConnectionsNotification,
    controller::{Controller, PlaybackInfo},
    controller_handle::PlayerCommandInput,
};

impl Controller {
    pub(super) fn player_monitor_loop(
        playback_state: Arc<std::sync::RwLock<PrismState>>,
        playback_time_tx: Receiver<(Option<TimeDelta>, Option<TimeDelta>)>,
        about_to_finish_tx: Receiver<()>,
        finished_tx: Receiver<()>,
        player_mail: async_channel::Sender<PlayerCommandInput>,
        notify_next_song: Sender<Song>,
        notify_connections_: Sender<ConnectionsNotification>,
        playback_info: Arc<AtomicCell<PlaybackInfo>>,
    ) -> Result<(), ()> {
        std::thread::scope(|s| {
            // Thread for timing and metadata
            let notify_connections = notify_connections_.clone();
            s.spawn({
                move || {
                    println!("playback monitor started");
                    while true {
                        let (position, duration) = playback_time_tx.recv().unwrap();
                        notify_connections
                            .send(ConnectionsNotification::Playback {
                                position: position.clone(),
                                duration: duration.clone(),
                            })
                            .unwrap();
                        playback_info.store(PlaybackInfo { position, duration });
                    }
                }
            });

            let notify_connections = notify_connections_.clone();
            s.spawn(move || {
                println!("AboutToFinish monitor started");
                futures::executor::block_on(async {
                    while true {
                        _ = about_to_finish_tx.recv();
                        notify_connections.send(ConnectionsNotification::AboutToFinish).unwrap();
                        println!("About to Finish");
                    }
                })
            });

            // Thread for End of Track
            let notify_connections = notify_connections_.clone();
            s.spawn(move || {
                println!("EOS monitor started");
                futures::executor::block_on(async {
                    while true {
                        _ = finished_tx.recv();

                        let (command, tx) = PlayerCommandInput::command(PlayerCommand::NextSong);
                        player_mail.send(command).await.unwrap();
                        let PlayerResponse::NowPlaying(res) = tx.recv().await.unwrap() else {
                            unreachable!()
                        };
                        if let Ok(song) = res {
                            notify_next_song.send(song.clone()).unwrap();
                            notify_connections
                                .send(ConnectionsNotification::SongChange(song))
                                .unwrap();
                        }

                        notify_connections
                                .send(ConnectionsNotification::EOS)
                                .unwrap();
                        println!("End of song");
                    }
                });
            });

            let notify_connections = notify_connections_.clone();
            s.spawn(move || {
                let mut state = PrismState::Stopped;
                while true {
                    let _state = playback_state.read().unwrap().to_owned();
                    if _state != state {
                        state = _state;
                        println!("State Changed to {state:?}");
                        notify_connections
                            .send(ConnectionsNotification::StateChange(state.clone()))
                            .unwrap();
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        });
        println!("Monitor Loops Ended");
        Ok(())
    }
}
