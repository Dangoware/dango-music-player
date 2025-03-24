use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::sleep,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chrono::TimeDelta;
use crossbeam::select;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
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
    TryEnableConnection(TryConnectionType),
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub(super) enum TryConnectionType {
    Discord(u64),
    LastFM {
        api_key: String,
        api_secret: String,
        auth: LastFMAuth,
    },
    ListenBrainz(String),
}

#[derive(Debug, Clone)]
pub enum LastFMAuth {
    Session(Option<String>),
    UserPass { username: String, password: String },
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
    let (dc_now_playing_rx, dc_now_playing_tx) = unbounded::<Song>();
    let (dc_position_rx, dc_position_tx) = bounded::<Option<TimeDelta>>(0);
    let (lb_now_playing_rx, lb_now_playing_tx) = unbounded::<Song>();
    let (lb_scrobble_rx, lb_scrobble_tx) = unbounded::<()>();
    let (last_now_playing_rx, last_now_playing_tx) = unbounded::<Song>();
    let (last_scrobble_rx, last_scrobble_tx) = unbounded::<()>();

    let mut song_scrobbled = false;

    //TODO: update scrobble position on seek
    // /// The position at which you can scrobble the song. changes on seek
    // struct ScrobblePosition {
    //     percent: f32,
    //     position: i32
    // }
    // let mut scrobble_position = ScrobblePosition { percent: f32::MAX, position: i32::MAX };

    use ConnectionsNotification::*;
    while true {
        match notifications_tx.recv().unwrap() {
            Playback {
                position: _position,
                duration: _duration,
            } => {
                _ = dc_position_rx.send_timeout(_position.clone(), Duration::from_millis(0));
                if song_scrobbled {
                    continue;
                }

                let Some(position) = _position.map(|t| t.num_milliseconds()) else {
                    continue;
                };
                let Some(duration) = _duration.map(|t| t.num_milliseconds()) else {
                    continue;
                };

                // Scrobble at 50% or at 4 minutes
                if duration < 30000 || position == 0 {
                    continue;
                }
                let percent_played = position as f32 / duration as f32;

                if percent_played != 0.0 && (percent_played > 0.5 || position >= 240000) {
                    if LB_ACTIVE.load(Ordering::Relaxed) {
                        lb_scrobble_rx.send(()).unwrap();
                    }
                    if LAST_FM_ACTIVE.load(Ordering::Relaxed) {
                        last_scrobble_rx.send(()).unwrap();
                    }
                    song_scrobbled = true;
                }
            }
            StateChange(state) => {
                if DC_ACTIVE.load(Ordering::Relaxed) {
                    dc_state_rx.send(state.clone()).unwrap();
                }
            }
            SongChange(song) => {
                song_scrobbled = false;
                if DC_ACTIVE.load(Ordering::Relaxed) {
                    dc_now_playing_rx.send(song.clone()).unwrap();
                }
                if LB_ACTIVE.load(Ordering::Relaxed) {
                    lb_now_playing_rx.send(song.clone()).unwrap();
                }
                if LAST_FM_ACTIVE.load(Ordering::Relaxed) {
                    last_now_playing_rx.send(song.clone()).unwrap();
                }
            }
            EOS => continue,
            AboutToFinish => continue,
            TryEnableConnection(c) => {
                match c {
                    TryConnectionType::Discord(client_id) => {
                        let (dc_song_tx, dc_state_tx, dc_position_tx) = (
                            dc_now_playing_tx.clone(),
                            dc_state_tx.clone(),
                            dc_position_tx.clone(),
                        );
                        std::thread::Builder::new()
                            .name("Discord RPC Handler".to_string())
                            .spawn(move || {
                                // TODO: add proper error handling here
                                discord_rpc(client_id, dc_song_tx, dc_state_tx, dc_position_tx);
                            })
                            .unwrap();
                    }
                    TryConnectionType::ListenBrainz(token) => {
                        let (lb_now_playing_tx, lb_scrobble_tx) =
                            (lb_now_playing_tx.clone(), lb_scrobble_tx.clone());
                        std::thread::Builder::new()
                            .name("ListenBrainz Handler".to_string())
                            .spawn(move || {
                                listenbrainz_scrobble(&token, lb_now_playing_tx, lb_scrobble_tx);
                            })
                            .unwrap();
                    }
                    TryConnectionType::LastFM {
                        api_key,
                        api_secret,
                        auth,
                    } => {
                        let (config, notifications_rx) = (config.clone(), notifications_rx.clone());
                        let (last_now_playing_tx, last_scrobble_tx) =
                            (last_now_playing_tx.clone(), last_scrobble_tx.clone());
                        std::thread::Builder::new()
                            .name("last.fm Handler".to_string())
                            .spawn(move || {
                                let scrobbler = match auth {
                                    LastFMAuth::Session(key) => if let Some(session) = key {
                                        let mut scrobbler = Scrobbler::new(&api_key, &api_secret);
                                        scrobbler.authenticate_with_session_key(&session);
                                        Ok(scrobbler)
                                    } else {
                                        last_fm_auth(
                                            config,
                                            notifications_rx,
                                            &api_key,
                                            &api_secret,
                                        )
                                    }
                                    .unwrap(),
                                    LastFMAuth::UserPass { username, password } => {
                                        let mut scrobbler = Scrobbler::new(&api_key, &api_secret);
                                        scrobbler
                                            .authenticate_with_password(&username, &password)
                                            .unwrap();
                                        scrobbler
                                    }
                                };
                                last_fm_scrobble(scrobbler, last_now_playing_tx, last_scrobble_tx);
                            })
                            .unwrap();
                    }
                }
            }
        }
    }
}

fn discord_rpc(
    client_id: u64,
    song_tx: Receiver<Song>,
    state_tx: Receiver<PrismState>,
    position_tx: Receiver<Option<TimeDelta>>,
) {
    let mut client =
        discord_presence::Client::with_error_config(client_id, Duration::from_secs(5), None);
    client.start();
    while !Client::is_ready() {
        sleep(Duration::from_millis(100));
    }
    println!("discord connected");

    let mut state = None;
    let mut song: Option<Song> = None;
    let mut now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards?")
        .as_secs();

    DC_ACTIVE.store(true, Ordering::Relaxed);

    while true {
        let state: &mut Option<PrismState> = &mut state;
        let song: &mut Option<Song> = &mut song;

        select! {
            recv(state_tx) -> res => {
                if let Ok(state_) = res {
                    *state = Some(state_);
                }
            },
            recv(song_tx) -> res => {
                if let Ok(song_) = res {
                    *song = Some(song_);
                    now = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards?").as_secs();
                }
            },
            default(Duration::from_millis(1000)) => {}
        }

        if let Ok(Some(pos)) = position_tx.recv_timeout(Duration::from_millis(100)) {
            // set back the start position to where it would be if it hadn't been paused / seeked
            now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards?")
                .as_secs()
                - u64::try_from(pos.num_seconds()).unwrap();
        }

        client
            .set_activity(|activity| {
                let activity = activity
                    .state(song.as_ref().map_or(String::new(), |s| {
                        format!(
                            "{}{}{}",
                            s.get_tag(&Tag::Artist)
                                .map_or(String::new(), |album| album.clone()),
                            if s.get_tag(&Tag::Album).is_some() && s.get_tag(&Tag::Artist).is_some()
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
                    if *state == Some(PrismState::Playing) {
                        activity.timestamps(|timestamps| {
                            timestamps.start(now).end(now + s.duration.as_secs())
                        })
                    } else {
                        activity
                    }
                } else {
                    activity
                }
                .assets(|a| {
                    a.large_text(match state {
                        Some(PrismState::Playing) => "Playing",
                        Some(PrismState::Paused) => "Paused",
                        Some(PrismState::Stopped) => "Stopped",
                        None => "Started",
                        _ => "I'm Scared, Boss",
                    })
                })
                .instance(true)
            })
            .unwrap();
    }
    DC_ACTIVE.store(false, Ordering::Relaxed);
}

fn listenbrainz_scrobble(token: &str, now_playing_tx: Receiver<Song>, scrobble_tx: Receiver<()>) {
    let mut client = ListenBrainz::new();
    client.authenticate(token).unwrap();
    if !client.is_authenticated() {
        return;
    }

    let mut song: Option<Song> = None;
    LB_ACTIVE.store(true, Ordering::Relaxed);
    println!("ListenBrainz connected");

    while true {
        let now_playing = &mut song;

        let client = &client;
        select! {
            recv(now_playing_tx) -> res => {
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
                    *now_playing = Some(_song);
                }
            },
            recv(scrobble_tx) -> _ => {
                if let Some(song) = now_playing.take() {
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
                    println!("Song {title} Listened");
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
    api_secret: &str,
) -> Result<Scrobbler, Box<dyn std::error::Error>> {
    let token = {
        tokio::runtime::Builder::new_current_thread()
        .enable_all()
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
    opener::open_browser(format!(
        "http://www.last.fm/api/auth/?api_key={api_key}&token={}",
        token.token
    ))
    .unwrap();

    let session = loop {
        if let Ok(session) = scrobbler.authenticate_with_token(&token.token) {
            break session;
        }
        sleep(Duration::from_millis(1000));
    };
    println!("Session: {}", session.key);
    {
        let mut config = config.write();
        config.connections.last_fm_session = Some(session.key);
        config.write_file().unwrap();
    }
    Ok(scrobbler)
}

fn last_fm_scrobble(
    scrobbler: Scrobbler,
    now_playing_tx: Receiver<Song>,
    scrobble_tx: Receiver<()>,
) {
    // TODO: Add support for scrobble storage for later

    let mut song: Option<Song> = None;
    LAST_FM_ACTIVE.store(true, Ordering::Relaxed);
    println!("last.fm connected");

    while true {
        let now_playing = &mut song;

        let scrobbler = &scrobbler;

        select! {
            recv(now_playing_tx) -> res => {
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

                    *now_playing = Some(_song);
                }
            },
            recv(scrobble_tx) -> _ => {
                if let Some(song) = now_playing.take() {
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
                        Ok(_) => println!("Song {title} Scrobbled"),
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
