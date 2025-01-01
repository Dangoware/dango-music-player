use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::TimeDelta;
use crossbeam::{scope, select};
use crossbeam_channel::{unbounded, Receiver};
use discord_presence::Client;
use listenbrainz::ListenBrainz;
use parking_lot::RwLock;
use prismriver::State as PrismState;

use crate::{
    config::Config,
    music_storage::library::{Song, Tag},
};

use super::controller::Controller;

#[derive(Debug, Clone)]
pub(super) enum ConnectionsNotification {
    Playback {
        position: Option<TimeDelta>,
        duration: Option<TimeDelta>,
    },
    StateChange(PrismState),
    SongChange(Song),
    AboutToFinish,
    EOS,
}

#[derive(Debug)]
pub struct ConnectionsInput {
    pub discord_rpc_client_id: Option<u64>,
}

pub(super) struct ControllerConnections {
    pub notifications_tx: Receiver<ConnectionsNotification>,
    pub inner: ConnectionsInput,
}

static DC_ACTIVE: AtomicBool = AtomicBool::new(false);
static LB_ACTIVE: AtomicBool = AtomicBool::new(false);

impl Controller {
    pub(super) fn handle_connections(
        config: Arc<RwLock<Config>>,
        ControllerConnections {
            notifications_tx,
            inner: ConnectionsInput {
                discord_rpc_client_id,
            },
        }: ControllerConnections,
    ) {
        let (dc_state_rx, dc_state_tx) = unbounded::<PrismState>();
        let (dc_song_rx, dc_song_tx) = unbounded::<Song>();
        let (lb_song_rx, lb_song_tx) = unbounded::<Song>();
        let (lb_abt_fin_rx, lb_abt_fn_tx) = unbounded::<()>();
        let (lb_eos_rx, lb_eos_tx) = unbounded::<()>();

        scope(|s| {
            s.builder()
                .name("Notifications Sorter".to_string())
                .spawn(|_| {
                    use ConnectionsNotification::*;
                    while true {
                        match notifications_tx.recv().unwrap() {
                            Playback { .. } => {}
                            StateChange(state) => {
                                if DC_ACTIVE.load(Ordering::Relaxed) {
                                    dc_state_rx.send(state.clone()).unwrap();
                                }
                            }
                            SongChange(song) => {
                                if DC_ACTIVE.load(Ordering::Relaxed) {
                                    dc_song_rx.send(song.clone()).unwrap();
                                }
                                if LB_ACTIVE.load(Ordering::Relaxed) {
                                    lb_song_rx.send(song).unwrap();
                                }
                            }
                            EOS => {
                                if LB_ACTIVE.load(Ordering::Relaxed) {
                                    lb_eos_rx.send(()).unwrap();
                                }
                            }
                            AboutToFinish => {
                                if LB_ACTIVE.load(Ordering::Relaxed) {
                                    lb_abt_fin_rx.send(()).unwrap();
                                }
                            }
                        }
                    }
                })
                .unwrap();

            if let Some(client_id) = discord_rpc_client_id {
                s.builder()
                    .name("Discord RPC Handler".to_string())
                    .spawn(move |_| {
                        Controller::discord_rpc(client_id, dc_song_tx, dc_state_tx);
                    })
                    .unwrap();
            };

            if let Some(token) = config.read().connections.listenbrainz_token.clone() {
                s.builder()
                    .name("ListenBrainz Handler".to_string())
                    .spawn(move |_| {
                        Controller::listenbrainz_scrobble(&token, lb_song_tx, lb_abt_fn_tx, lb_eos_tx);
                    })
                    .unwrap();
            }
        })
        .unwrap();
    }

    fn discord_rpc(client_id: u64, song_tx: Receiver<Song>, state_tx: Receiver<PrismState>) {
        // TODO: Handle seeking position change and pause
        let mut client =
            discord_presence::Client::with_error_config(client_id, Duration::from_secs(5), None);
        client.start();
        while !Client::is_ready() {
            sleep(Duration::from_millis(100));
        }
        println!("discord connected");

        let mut state = "Started".to_string();
        let mut song: Option<Song> = None;
        let mut now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards?")
            .as_secs();
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

            client
                .set_activity(|activity| {
                    let a = activity
                        .state(song.as_ref().map_or(String::new(), |s| {
                            format!(
                                "{}{}{}",
                                s.get_tag(&Tag::Artist)
                                    .map_or(String::new(), |album| album.clone()),
                                if s.get_tag(&Tag::Album).is_some()
                                    && s.get_tag(&Tag::Artist).is_some()
                                {
                                    " - "
                                } else {
                                    ""
                                },
                                s.get_tag(&Tag::Album)
                                    .map_or(String::new(), |album| album.clone())
                            )
                        }))
                        ._type(discord_presence::models::ActivityType::Listening)
                        .details(if let Some(song) = song {
                            song.get_tag(&Tag::Title)
                                .map_or(String::from("Unknown Title"), |title| title.clone())
                        } else {
                            String::new()
                        });
                    if let Some(s) = song {
                        if state.as_str() == "Playing" {
                            a.timestamps(|timestamps| {
                                timestamps.start(now).end(now + s.duration.as_secs())
                            })
                        } else {
                            a
                        }
                    } else {
                        a
                    }
                    .assets(|a| a.large_text(state.clone()))
                    .instance(true)
                })
                .unwrap();
        }
        DC_ACTIVE.store(false, Ordering::Relaxed);
    }

    fn listenbrainz_scrobble(token: &str, song_tx: Receiver<Song>, abt_fn_tx: Receiver<()>, eos_tx: Receiver<()>) {
        let mut client = ListenBrainz::new();
        client.authenticate(token).unwrap();
        if !client.is_authenticated() {
            return;
        }

        let mut song: Option<Song> = None;
        let mut last_song: Option<Song> = None;
        LB_ACTIVE.store(true, Ordering::Relaxed);
        println!("ListenBrainz connected");

        while true {
            let song = &mut song;
            let last_song = &mut last_song;

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
                        let release = _song.get_tag(&Tag::Key(String::from("MusicBrainzReleaseId"))).map(|id| id.as_str());

                        client.playing_now(artist, title, release).unwrap();
                        println!("Song Listening = {artist} - {title}");
                        *song = Some(_song);
                    }
                },
                recv(abt_fn_tx) -> _ => {
                    *last_song = song.take();
                    println!("song = {:?}", last_song.as_ref().map(|s| s.get_tag(&Tag::Title).map_or("No Title", |t| t.as_str())));
                },
                recv(eos_tx) -> _ => {
                    if let Some(song) = last_song {
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
                        let release = song.get_tag(&Tag::Key(String::from("MusicBrainzReleaseId"))).map(|id| id.as_str());

                        client.listen(artist, title, release).unwrap();
                        println!("Song Scrobbled");
                    }
                }
            }
        }
        LB_ACTIVE.store(false, Ordering::Relaxed);
    }
}