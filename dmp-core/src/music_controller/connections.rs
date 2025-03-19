use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::TimeDelta;
use crossbeam::select;
use crossbeam_channel::{unbounded, Receiver, Sender};
use discord_presence::Client;
use listenbrainz::ListenBrainz;
use parking_lot::RwLock;
use prismriver::State as PrismState;
use rustfm_scrobble::{Scrobble, Scrobbler};
use serde::Deserialize;

use crate::{
    config::Config,
    music_storage::library::{Song, Tag},
};


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
    TryEnableConnection(TryConnectionType)
}

#[derive(Debug, Clone)]
pub(super) enum TryConnectionType {
    Discord(u64),
    LastFM {
        api_key: String,
        api_secret: String,
        auth: LastFMAuth
    },
    ListenBrainz(String),
    Custom(String)
}

#[derive(Debug, Clone)]
pub(super) enum LastFMAuth {
    Session(Option<String>),
    UserPass {
        username: String,
        password: String
    }
}

pub(super) struct ControllerConnections {
    pub notifications_rx: Sender<ConnectionsNotification>,
    pub notifications_tx: Receiver<ConnectionsNotification>,
}

static DC_ACTIVE: AtomicBool = AtomicBool::new(false);
static LB_ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST_FM_ACTIVE: AtomicBool = AtomicBool::new(false);

pub(super) fn handle_connections(
    config: Arc<RwLock<Config>>,
    ControllerConnections {
        notifications_rx,
        notifications_tx,
    }: ControllerConnections,
) {
    let (dc_state_rx, dc_state_tx) = unbounded::<PrismState>();
    let (dc_song_rx, dc_song_tx) = unbounded::<Song>();
    let (lb_song_rx, lb_song_tx) = unbounded::<Song>();
    let (lb_abt_fin_rx, lb_abt_fin_tx) = unbounded::<()>();
    let (lb_eos_rx, lb_eos_tx) = unbounded::<()>();
    let (last_song_rx, last_song_tx) = unbounded::<Song>();
    let (last_abt_fin_rx, last_abt_fin_tx) = unbounded::<()>();
    let (last_eos_rx, last_eos_tx) = unbounded::<()>();


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
            TryEnableConnection(c) => { match c {
                TryConnectionType::Discord(client_id) => {
                    let (dc_song_tx, dc_state_tx) = (dc_song_tx.clone(), dc_state_tx.clone());
                    std::thread::Builder::new()
                        .name("Discord RPC Handler".to_string())
                        .spawn(move || {
                                // TODO: add proper error handling here
                                discord_rpc(client_id, dc_song_tx, dc_state_tx);
                        })
                        .unwrap();
                },
                TryConnectionType::ListenBrainz(token) => {
                    let (lb_song_tx, lb_abt_fin_tx, lb_eos_tx) = (lb_song_tx.clone(), lb_abt_fin_tx.clone(), lb_eos_tx.clone());
                    std::thread::Builder::new()
                        .name("ListenBrainz Handler".to_string())
                        .spawn(move || {
                            listenbrainz_scrobble(&token, lb_song_tx, lb_abt_fin_tx, lb_eos_tx);
                        })
                        .unwrap();
                }
                TryConnectionType::LastFM { api_key, api_secret, auth } => {
                    let (config, notifications_rx) = (config.clone(), notifications_rx.clone());
                    let (last_song_tx, last_abt_fin_tx, last_eos_tx) = (last_song_tx.clone(), last_abt_fin_tx.clone(), last_eos_tx.clone());
                    std::thread::Builder::new()
                        .name("last.fm Handler".to_string())
                        .spawn(move || {
                            let scrobbler = match auth {
                                LastFMAuth::Session(key) => {
                                    if let Some(session) = key {
                                        let mut scrobbler = Scrobbler::new(&api_key, &api_secret);
                                        scrobbler.authenticate_with_session_key(&session);
                                        Ok(scrobbler)
                                    } else {
                                        last_fm_auth(config, notifications_rx, &api_key, &api_secret)
                                    }.unwrap()
                                },
                                LastFMAuth::UserPass { username, password } => {
                                    let mut scrobbler = Scrobbler::new(&api_key, &api_secret);
                                    scrobbler.authenticate_with_password(&username, &password).unwrap();
                                    scrobbler
                                }
                            };
                            last_fm_scrobble(scrobbler, last_song_tx, last_abt_fin_tx, last_eos_tx);
                        })
                        .unwrap();
                }
                TryConnectionType::Custom(_) => unimplemented!()
            }}
        }
    }

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
fn last_fm_auth(
    config: Arc<RwLock<Config>>,
    notifications_rx: Sender<ConnectionsNotification>,
    api_key: &str,
    api_secret: &str
) -> Result<Scrobbler, Box<dyn std::error::Error>> {
    let token = {
        tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(
            async {
                reqwest::get(
                    format!("http://ws.audioscrobbler.com/2.0/?method=auth.gettoken&api_key={api_key}&format=json"))
                    .await
                    .unwrap()
                    .json::<Token>()
                    .await
                    .unwrap()
            }
        )
    };
    let mut scrobbler = Scrobbler::new(api_key, api_secret);
    println!("Token: {}", token.token);
    opener::open_browser(format!("http://www.last.fm/api/auth/?api_key={api_key}&token={}", token.token)).unwrap();

    let session = loop {
        if let Ok(session) = scrobbler.authenticate_with_token(&token.token) {
            break session;
        }
        sleep(Duration::from_millis(1000));
    };
    println!("Session: {}", session.key);

    config.write().connections.last_fm_session = Some(session.key);
    Ok(scrobbler)
}

fn last_fm_scrobble(scrobbler: Scrobbler, song_tx: Receiver<Song>, abt_fn_tx: Receiver<()>, eos_tx: Receiver<()>) {
    // TODO: Add support for scrobble storage for later

    let mut song: Option<Song> = None;
    let mut last_song: Option<Song> = None;
    LAST_FM_ACTIVE.store(true, Ordering::Relaxed);
    println!("ListenBrainz connected");

    while true {
        let song = &mut song;
        let last_song = &mut last_song;

        let scrobbler = &scrobbler;

        select! {
            recv(song_tx) -> res => {
                if let Ok(_song) = res {
                    let title = if let Some(tag) = _song.get_tag(&Tag::Title) {
                        tag.as_str()
                    } else {
                        continue
                    };
                    let artist = if let Some(tag) = _song.get_tag(&Tag::Artist) {
                        tag.as_str()
                    } else {
                        ""
                    };
                    let album = if let Some(tag) = _song.get_tag(&Tag::Album) {
                        tag.as_str()
                    } else {
                        ""
                    };

                    match scrobbler.now_playing(&Scrobble::new(artist, title, album)) {
                        Ok(_) => println!("Song Scrobbling = {artist} - {title} - {album}"),
                        Err(e) => println!("Error at last.fm now playing:\n{e}")
                    };

                    *song = Some(_song);
                }
            },
            recv(abt_fn_tx) -> _ => {
                *last_song = song.take();
                println!("song = {:?}", last_song.as_ref().map(|s| s.get_tag(&Tag::Title).map_or("No Title", |t| t.as_str())));
            },
            recv(eos_tx) -> _ => {
                if let Some(song) = last_song {
                    let title = if let Some(tag) = song.get_tag(&Tag::Title) {
                        tag.as_str()
                    } else {
                        continue
                    };
                    let artist = if let Some(tag) = song.get_tag(&Tag::Artist) {
                        tag.as_str()
                    } else {
                        ""
                    };
                    let album = if let Some(tag) = song.get_tag(&Tag::Album) {
                        tag.as_str()
                    } else {
                        ""
                    };

                    match scrobbler.scrobble(&Scrobble::new(artist, title, album)) {
                        Ok(_) => println!("Song Scrobbled"),
                        Err(e) => println!("Error at last.fm scrobbler:\n{e:?}")
                    }
                }
            }
        }
    }
    LAST_FM_ACTIVE.store(false, Ordering::Relaxed);
}



#[derive(Deserialize)]
pub struct Token {
    token: String,
}