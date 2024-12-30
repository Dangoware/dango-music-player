#![allow(while_true)]
use std::time::Duration;

use chrono::TimeDelta;
use crossbeam::scope;
use crossbeam_channel::{bounded, Receiver};
use discord_presence::models::{Activity, ActivityTimestamps, ActivityType};
use prismriver::State as PrismState;
use rayon::spawn;

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
        spawn(move || {
            let mut client = discord_presence::Client::new(client_id);
            client.start();
            client.block_until_event(discord_presence::Event::Connected).unwrap();
            client.set_activity(|_|
                Activity::new()
            ).unwrap();
            println!("discord connected");

            let mut state = "Started".to_string();
            let mut song: Option<Song> = None;

            while true {
                let state_res = state_tx.recv_timeout(Duration::from_secs(5));
                let song_res = song_tx.recv_timeout(Duration::from_millis(100));

                let state = &mut state;
                let song = &mut song;

                if let Ok(state_) = state_res {
                    *state = match state_ {
                        PrismState::Playing => "Playing",
                        PrismState::Paused => "Paused",
                        PrismState::Stopped => "Stopped",
                        _ => "I'm Scared, Boss"
                    }.to_string()
                }
                if let Ok(song_) = song_res {
                    *song = Some(song_);
                }

                client.set_activity(|activity| {
                    activity.state(
                        state.clone()
                    )._type(discord_presence::models::ActivityType::Listening)
                    .details(
                        if let Some(song) = song {
                            format!(
                                "{} - {}\n{}",
                                song.get_tag(&Tag::Title).map_or(String::from("No Title"), |title| title.clone()),
                                song.get_tag(&Tag::Artist).map_or(String::from("No Artist"), |artist| artist.clone()),
                                song.get_tag(&Tag::Album).map_or(String::from("No Album"), |album| album.clone())
                            )
                        } else {
                            String::new()
                        }
                    )
                    // if let Some(song) = song {
                    //     a.timestamps(|timestamp| {
                    //         ActivityTimestamps::new()
                    //         .start(timestamp.start.unwrap_or_default())
                    //         .end(
                    //             song.duration.as_millis().clamp(u64::MIN as u128, u64::MAX as u128) as u64
                    //         )
                    //     })
                    // } else {
                    //     a
                    // }
                }).unwrap();
                println!("Changed Status");
            }
        });
    }
}