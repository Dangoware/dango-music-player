use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, thread::sleep, time::{Duration, SystemTime, UNIX_EPOCH}};

use chrono::TimeDelta;
use crossbeam::{scope, select};
use crossbeam_channel::{bounded, Receiver};
use discord_presence::Client;
use listenbrainz::ListenBrainz;
use parking_lot::RwLock;
use prismriver::State as PrismState;

use crate::{config::Config, music_storage::library::{Song, Tag}};

use super::controller::Controller;

#[derive(Debug, Clone)]
pub(super) enum ConnectionsNotification {
    Playback {
        position: Option<TimeDelta>,
        duration: Option<TimeDelta>
    },
    StateChange(PrismState),
    SongChange(Song),
    EOS,
}

#[derive(Debug)]
pub struct ConnectionsInput {
    pub discord_rpc_client_id: Option<u64>,
}

pub(super) struct ControllerConnections {
    pub notifications_tx: Receiver<ConnectionsNotification>,
    pub inner: ConnectionsInput
}


static DC_ACTIVE: AtomicBool = AtomicBool::new(false);
static LB_ACTIVE: AtomicBool = AtomicBool::new(false);

impl Controller {
    pub(super) fn handle_connections(
        config: Arc<RwLock<Config>>,
        ControllerConnections {
        notifications_tx,
        inner: ConnectionsInput {
               discord_rpc_client_id
        },
    }: ControllerConnections
    ) {
        let (dc_state_rx, dc_state_tx) = bounded::<PrismState>(1);
        let (dc_song_rx, dc_song_tx) = bounded::<Song>(1);
        let (lb_song_rx, lb_song_tx) = bounded::<Song>(1);
        let (lb_eos_rx, lb_eos_tx) = bounded::<()>(1);

        scope(|s| {
            s.builder().name("Notifications Sorter".to_string()).spawn(|_| {
                use ConnectionsNotification::*;
                while true {
                    match notifications_tx.recv().unwrap() {
                        Playback { .. } => {}
                        StateChange(state) => {
                            if DC_ACTIVE.load(Ordering::Relaxed) { dc_state_rx.send(state.clone()).unwrap(); }
                        }
                        SongChange(song) => {
                            if DC_ACTIVE.load(Ordering::Relaxed) { dc_song_rx.send(song.clone()).unwrap(); }
                            if LB_ACTIVE.load(Ordering::Relaxed) { lb_song_rx.send(song).unwrap(); }
                        }
                        EOS => {
                            if LB_ACTIVE.load(Ordering::Relaxed) { lb_eos_rx.send(()).unwrap(); }
                        }
                    }
                }
            }).unwrap();

            if let Some(client_id) = discord_rpc_client_id {
                s.builder().name("Discord RPC Handler".to_string()).spawn(move |_| {
                    Controller::discord_rpc(client_id, dc_song_tx, dc_state_tx);
                }).unwrap();
            };

            if let Some(token) = config.read().connections.listenbrainz_token.clone() {
                s.builder().name("ListenBrainz Handler".to_string()).spawn(move |_| {
                    Controller::listenbrainz_scrobble(&token, lb_song_tx, lb_eos_tx);
                }).unwrap();
            }
        }).unwrap();
    }

    fn discord_rpc(client_id: u64, song_tx: Receiver<Song>, state_tx: Receiver<PrismState>) {
        // TODO: Handle seeking position change and pause
        let mut client = discord_presence::Client::with_error_config(client_id, Duration::from_secs(5), None);
        client.start();
        while !Client::is_ready() { sleep(Duration::from_millis(100)); }
        println!("discord connected");

        let mut state = "Started".to_string();
        let mut song: Option<Song> = None;
        let mut now = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards?").as_secs();
        DC_ACTIVE.store(true, Ordering::Relaxed);

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
                },
                default(Duration::from_millis(99)) => ()
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
                }).instance(true)
            }).unwrap();
        }
        DC_ACTIVE.store(false, Ordering::Relaxed);
    }

    fn listenbrainz_scrobble(token: &str, song_tx: Receiver<Song>, eos_tx: Receiver<()>) {
        let mut client = ListenBrainz::new();
        client.authenticate(token).unwrap();
        if !client.is_authenticated() {
            return;
        }

        let mut song: Option<Song> = None;
        LB_ACTIVE.store(true, Ordering::Relaxed);

        while true {
            let song = &mut song;
            let client = &client;
            select! {
                recv(song_tx) -> res => {
                    if let Ok(_song) = res {
                        let artist = if let Some(tag) = _song.get_tag(&Tag::Artist) {
                            tag.as_str()
                        } else {
                            continue
                        };
                        let title = if let Some(tag) = _song.get_tag(&Tag::Title) {
                            tag.as_str()
                        } else {
                            continue
                        };
                        client.playing_now(artist, title, None).unwrap();
                        *song = Some(_song);
                        println!("Song Listening")
                    }
                },
                recv(eos_tx) -> _ => {
                    if let Some(song) = song {
                        let artist = if let Some(tag) = song.get_tag(&Tag::Artist) {
                            tag.as_str()
                        } else {
                            continue
                        };
                        let title = if let Some(tag) = song.get_tag(&Tag::Title) {
                            tag.as_str()
                        } else {
                            continue
                        };
                        client.listen(artist, title, None).unwrap();
                        println!("Song Scrobbled");
                    }
                }
            }
        }
        LB_ACTIVE.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod test_super {
    use std::thread::{sleep, spawn};

    use crossbeam_channel::unbounded;

    use crate::config::tests::read_config_lib;

    use super::*;

    #[test]
    fn lb_test() {
        let (song_rx, song_tx) = unbounded();
        let (eos_rx, eos_tx) = unbounded();

        let (config, lib ) = read_config_lib();
        song_rx.send(lib.library[0].clone()).unwrap();
        spawn(|| {
            Controller::listenbrainz_scrobble(config.connections.listenbrainz_token.unwrap().as_str(), song_tx, eos_tx);
        });
        sleep(Duration::from_secs(10));
        eos_rx.send(()).unwrap();
        sleep(Duration::from_secs(10));
    }
}