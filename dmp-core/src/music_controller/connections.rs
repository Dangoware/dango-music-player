#![allow(while_true)]
use std::{thread::sleep, time::{Duration, SystemTime, UNIX_EPOCH}};

use chrono::TimeDelta;
use crossbeam::{scope, select};
use crossbeam_channel::{bounded, Receiver};
use discord_presence::Client;
use prismriver::State as PrismState;

use crate::music_storage::library::{Song, Tag};

use super::controller::Controller;

#[derive(Debug, Clone)]
pub(super) enum ConnectionsNotification {
    Playback {
        position: Option<TimeDelta>,
        duration: Option<TimeDelta>
    },
    StateChange(PrismState),
    SongChange(Song),
}

#[derive(Debug)]
pub struct ConnectionsInput {
    pub discord_rpc_client_id: Option<u64>,
}

pub(super) struct ControllerConnections {
    pub notifications_tx: Receiver<ConnectionsNotification>,
    pub inner: ConnectionsInput
}

impl Controller {
    pub(super) fn handle_connections(ControllerConnections {
        notifications_tx,
        inner: ConnectionsInput {
               discord_rpc_client_id
        },
    }: ControllerConnections
    ) {
        let (dc_state_rx, dc_state_tx) = bounded::<PrismState>(1);
        let (dc_song_rx, dc_song_tx) = bounded::<Song>(1);
        scope(|s| {
            s.builder().name("Notifications Sorter".to_string()).spawn(|_| {
                use ConnectionsNotification::*;
                while true {
                    match notifications_tx.recv().unwrap() {
                        Playback { position, duration } => { continue; }
                        StateChange(state) => {
                            dc_state_rx.send(state.clone()).unwrap();
                        }
                        SongChange(song) => {
                            dc_song_rx.send(song).unwrap();
                        }
                    }
                }
            }).unwrap();

            if let Some(client_id) = discord_rpc_client_id {
                println!("Discord thingy detected");
                s.builder().name("Discord RPC Handler".to_string()).spawn(move |_| {
                    Controller::discord_rpc(client_id, dc_song_tx, dc_state_tx);
                }).unwrap();
            };
        }).unwrap();
    }

    fn discord_rpc(client_id: u64, song_tx: Receiver<Song>, state_tx: Receiver<PrismState>) {
        // TODO: Handle seeking position change
        std::thread::spawn(move || {
            let mut client = discord_presence::Client::new(client_id);
            client.start();
            while !Client::is_ready() { sleep(Duration::from_millis(100)); }
            println!("discord connected");

            let mut state = "Started".to_string();
            let mut song: Option<Song> = None;
            let mut now = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards?").as_secs();

            while true {
                let state = &mut state;
                let song = &mut song;
                select! {
                    recv(state_tx) -> res => {
                        if let Ok(state_) = res {
                            *state = match state_ {
                                PrismState::Playing => "Playing",
                                PrismState::Paused => "Paused",
                                PrismState::Stopped => "Stopped",
                                _ => "I'm Scared, Boss"
                            }.to_string();
                        }
                    },
                    recv(song_tx) -> res => {
                        if let Ok(song_) = res {
                            *song = Some(song_);
                            now = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards?").as_secs();
                        }
                    }
                    default(Duration::from_millis(4500)) => ()
                }

                client.set_activity(|activity| {
                    let a = activity.state(
                        song.as_ref().map_or(String::new(), |s| format!(
                            "{}{}{}",
                            s.get_tag(&Tag::Artist).map_or(String::new(), |album| album.clone()),
                            if s.get_tag(&Tag::Album).is_some() && s.get_tag(&Tag::Artist).is_some() { " - " } else { "" },
                            s.get_tag(&Tag::Album).map_or(String::new(), |album| album.clone())
                        )
                    )
                    )._type(discord_presence::models::ActivityType::Listening)
                    .details(
                        if let Some(song) = song {
                            song.get_tag(&Tag::Title).map_or(String::from("Unknown Title"), |title| title.clone())
                        } else {
                            String::new()
                        }
                    );
                    if let Some(s) = song {
                        if state.as_str() == "Playing" {
                            a.timestamps(|timestamps| {
                                timestamps.start(now)
                                .end(now + s.duration.as_secs())
                            })
                        } else {
                            a
                        }
                    } else {
                        a
                    }.assets(|a| {
                        a.large_text(state.clone())
                    })
                }).unwrap();
                println!("Updated Discord Status");
            }
        });
    }
}

#[cfg(test)]
mod test_super {
    use std::thread::sleep;

    use crossbeam_channel::unbounded;

    use crate::config::tests::read_config_lib;

    use super::*;

    #[test]
    fn discord_test() {
        let client_id = std::env!("DISCORD_CLIENT_ID").parse::<u64>().unwrap();
        let (song_rx, song_tx) = unbounded();
        let (_, state_tx) = unbounded();

        let (_, lib ) = read_config_lib();
        song_rx.send(lib.library[0].clone()).unwrap();

        Controller::discord_rpc(client_id, song_tx, state_tx);
        sleep(Duration::from_secs(150));
    }
}