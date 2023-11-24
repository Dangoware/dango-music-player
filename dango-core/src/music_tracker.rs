use serde_json::json;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use discord_presence::Event;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use surf::StatusCode;

use crate::music_storage::music_db::{Song, Tag};

#[async_trait]
pub trait MusicTracker {
    /// Adds one listen to a song halfway through playback
    async fn track_song(&mut self, song: Song) -> Result<(), TrackerError>;

    /// Adds a 'listening' status to the music tracker service of choice
    async fn track_now(&mut self, song: Song) -> Result<(), TrackerError>;

    /// Reads config files, and attempts authentication with service
    async fn test_tracker(&mut self) -> Result<(), TrackerError>;

    /// Returns plays for a given song according to tracker service
    async fn get_times_tracked(&mut self, song: &Song) -> Result<u32, TrackerError>;
}

#[derive(Debug)]
pub enum TrackerError {
    /// Tracker does not accept the song's format/content
    InvalidSong,
    /// Tracker requires authentication
    InvalidAuth,
    /// Tracker request was malformed
    InvalidRequest,
    /// Tracker is unavailable
    ServiceUnavailable,
    /// Unknown tracker error
    Unknown,
}

impl TrackerError {
    pub fn from_surf_error(error: surf::Error) -> TrackerError {
        match error.status() {
            StatusCode::Forbidden => TrackerError::InvalidAuth,
            StatusCode::Unauthorized => TrackerError::InvalidAuth,
            StatusCode::NetworkAuthenticationRequired => TrackerError::InvalidAuth,
            StatusCode::BadRequest => TrackerError::InvalidRequest,
            StatusCode::BadGateway => TrackerError::ServiceUnavailable,
            StatusCode::ServiceUnavailable => TrackerError::ServiceUnavailable,
            StatusCode::NotFound => TrackerError::ServiceUnavailable,
            _ => TrackerError::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LastFMConfig {
    pub enabled: bool,
    pub dango_api_key: String,
    pub shared_secret: String,
    pub session_key: String,
}

pub struct LastFM {
    config: LastFMConfig,
}

#[async_trait]
impl MusicTracker for LastFM {
    async fn track_song(&mut self, song: Song) -> Result<(), TrackerError> {
        let mut params: BTreeMap<&str, &str> = BTreeMap::new();

        // Sets timestamp of song beginning play time
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Your time is off.")
            .as_secs()
            - 30;
        let string_timestamp = timestamp.to_string();

        let (artist, track) = match (song.get_tag(&Tag::Artist), song.get_tag(&Tag::Title)) {
            (Some(artist), Some(track)) => (artist, track),
            _ => return Err(TrackerError::InvalidSong),
        };

        params.insert("method", "track.scrobble");
        params.insert("artist", artist);
        params.insert("track", track);
        params.insert("timestamp", &string_timestamp);

        return match self.api_request(params).await {
            Ok(_) => Ok(()),
            Err(err) => Err(TrackerError::from_surf_error(err)),
        };
    }

    async fn track_now(&mut self, song: Song) -> Result<(), TrackerError> {
        let mut params: BTreeMap<&str, &str> = BTreeMap::new();

        let (artist, track) = match (song.get_tag(&Tag::Artist), song.get_tag(&Tag::Title)) {
            (Some(artist), Some(track)) => (artist, track),
            _ => return Err(TrackerError::InvalidSong),
        };

        params.insert("method", "track.updateNowPlaying");
        params.insert("artist", artist);
        params.insert("track", track);

        return match self.api_request(params).await {
            Ok(_) => Ok(()),
            Err(err) => Err(TrackerError::from_surf_error(err)),
        };
    }

    async fn test_tracker(&mut self) -> Result<(), TrackerError> {
        let mut params: BTreeMap<&str, &str> = BTreeMap::new();
        params.insert("method", "chart.getTopArtists");

        return match self.api_request(params).await {
            Ok(_) => Ok(()),
            Err(err) => Err(TrackerError::from_surf_error(err)),
        };
    }

    async fn get_times_tracked(&mut self, _song: &Song) -> Result<u32, TrackerError> {
        todo!();
    }
}

#[derive(Deserialize, Serialize)]
struct AuthToken {
    token: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct SessionResponse {
    name: String,
    key: String,
    subscriber: i32,
}

#[derive(Deserialize, Serialize, Debug)]
struct Session {
    session: SessionResponse,
}

impl LastFM {
    /// Returns a url to be approved by the user along with the auth token
    pub async fn get_auth(api_key: &String) -> Result<String, surf::Error> {
        let method = String::from("auth.gettoken");
        let api_request_url = format!(
            "http://ws.audioscrobbler.com/2.0/?method={method}&api_key={api_key}&format=json"
        );

        let auth_token: AuthToken = surf::get(api_request_url).await?.body_json().await?;

        let auth_url = format!(
            "http://www.last.fm/api/auth/?api_key={api_key}&token={}",
            auth_token.token
        );

        Ok(auth_url)
    }

    /// Returns a LastFM session key
    pub async fn get_session_key(
        api_key: &String,
        shared_secret: &String,
        auth_token: &String,
    ) -> Result<String, surf::Error> {
        let method = String::from("auth.getSession");
        // Creates api_sig as defined in last.fm documentation
        let api_sig =
            format!("api_key{api_key}methodauth.getSessiontoken{auth_token}{shared_secret}");

        // Creates insecure MD5 hash for last.fm api sig
        let mut hasher = Md5::new();
        hasher.update(api_sig);
        let hash_result = hasher.finalize();
        let hex_string_hash = format!("{:#02x}", hash_result);

        let api_request_url = format!("http://ws.audioscrobbler.com/2.0/?method={method}&api_key={api_key}&token={auth_token}&api_sig={hex_string_hash}&format=json");

        let response = surf::get(api_request_url).recv_string().await?;

        // Sets session key from received response
        let session_response: Session = serde_json::from_str(&response)?;
        Ok(session_response.session.key)
    }

    /// Creates a new LastFM struct with a given config
    pub fn new(config: &LastFMConfig) -> LastFM {
        
        LastFM {
            config: config.clone(),
        }
    }

    // Creates an api request with the given parameters
    pub async fn api_request(
        &self,
        mut params: BTreeMap<&str, &str>,
    ) -> Result<surf::Response, surf::Error> {
        params.insert("api_key", &self.config.dango_api_key);
        params.insert("sk", &self.config.session_key);

        // Creates and sets api call signature
        let api_sig = LastFM::request_sig(&params, &self.config.shared_secret);
        params.insert("api_sig", &api_sig);
        let mut string_params = String::from("");

        // Creates method call string
        // Just iterate over values???
        for key in params.keys() {
            let param_value = params.get(key).unwrap();
            string_params.push_str(&format!("{key}={param_value}&"));
        }

        string_params.pop();

        let url = "http://ws.audioscrobbler.com/2.0/";

        

        surf::post(url).body_string(string_params).await
    }

    // Returns an api signature as defined in the last.fm api documentation
    fn request_sig(params: &BTreeMap<&str, &str>, shared_secret: &str) -> String {
        let mut sig_string = String::new();
        // Appends keys and values of parameters to the unhashed sig
        for key in params.keys() {
            let param_value = params.get(*key);
            sig_string.push_str(&format!("{key}{}", param_value.unwrap()));
        }
        sig_string.push_str(shared_secret);

        // Hashes signature using **INSECURE** MD5 (Required by last.fm api)
        let mut md5_hasher = Md5::new();
        md5_hasher.update(sig_string);
        let hash_result = md5_hasher.finalize();
        let hashed_sig = format!("{:#02x}", hash_result);

        hashed_sig
    }

    // Removes last.fm account from dango-music-player
    pub fn reset_account() {
        todo!();
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DiscordRPCConfig {
    pub enabled: bool,
    pub dango_client_id: u64,
    pub dango_icon: String,
}

pub struct DiscordRPC {
    config: DiscordRPCConfig,
    pub client: discord_presence::client::Client,
}

impl DiscordRPC {
    pub fn new(config: &DiscordRPCConfig) -> Self {
        
        DiscordRPC {
            client: discord_presence::client::Client::new(config.dango_client_id),
            config: config.clone(),
        }
    }
}

#[async_trait]
impl MusicTracker for DiscordRPC {
    async fn track_now(&mut self, song: Song) -> Result<(), TrackerError> {
        let unknown = String::from("Unknown");

        // Sets song title
        let song_name = if let Some(song_name) = song.get_tag(&Tag::Title) {
            song_name
        } else {
            &unknown
        };

        // Sets album
        let album = if let Some(album) = song.get_tag(&Tag::Album) {
            album
        } else {
            &unknown
        };

        let _client_thread = self.client.start();

        // Blocks thread execution until it has connected to local discord client
        let ready = self.client.block_until_event(Event::Ready);
        if ready.is_err() {
            return Err(TrackerError::ServiceUnavailable);
        }

        let start_time = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        // Sets discord account activity to current playing song
        let send_activity = self.client.set_activity(|activity| {
            activity
                .state(album.to_string())
                .details(song_name.to_string())
                .assets(|assets| assets.large_image(&self.config.dango_icon))
                .timestamps(|time| time.start(start_time))
        });

        match send_activity {
            Ok(_) => return Ok(()),
            Err(_) => return Err(TrackerError::ServiceUnavailable),
        }
    }

    async fn track_song(&mut self, _song: Song) -> Result<(), TrackerError> {
        return Ok(());
    }

    async fn test_tracker(&mut self) -> Result<(), TrackerError> {
        return Ok(());
    }

    async fn get_times_tracked(&mut self, _song: &Song) -> Result<u32, TrackerError> {
        return Ok(0);
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ListenBrainzConfig {
    pub enabled: bool,
    pub api_url: String,
    pub auth_token: String,
}

pub struct ListenBrainz {
    config: ListenBrainzConfig,
}

#[async_trait]
impl MusicTracker for ListenBrainz {
    async fn track_now(&mut self, song: Song) -> Result<(), TrackerError> {
        let (artist, track) = match (song.get_tag(&Tag::Artist), song.get_tag(&Tag::Title)) {
            (Some(artist), Some(track)) => (artist, track),
            _ => return Err(TrackerError::InvalidSong),
        };
        // Creates a json to submit a single song as defined in the listenbrainz documentation
        let json_req = json!({
            "listen_type": "playing_now",
            "payload": [
                {
                    "track_metadata": {
                        "artist_name": artist,
                        "track_name": track,
                    }
                }
            ]
        });

        return match self
            .api_request(&json_req.to_string(), &String::from("/1/submit-listens"))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(TrackerError::from_surf_error(err)),
        };
    }

    async fn track_song(&mut self, song: Song) -> Result<(), TrackerError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Your time is off.")
            .as_secs()
            - 30;

        let (artist, track) = match (song.get_tag(&Tag::Artist), song.get_tag(&Tag::Title)) {
            (Some(artist), Some(track)) => (artist, track),
            _ => return Err(TrackerError::InvalidSong),
        };

        let json_req = json!({
            "listen_type": "single",
            "payload": [
                {
                    "listened_at": timestamp,
                    "track_metadata": {
                        "artist_name": artist,
                        "track_name": track,
                    }
                }
            ]
        });

        return match self
            .api_request(&json_req.to_string(), &String::from("/1/submit-listens"))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(TrackerError::from_surf_error(err)),
        };
    }
    async fn test_tracker(&mut self) -> Result<(), TrackerError> {
        todo!()
    }
    async fn get_times_tracked(&mut self, _song: &Song) -> Result<u32, TrackerError> {
        todo!()
    }
}

impl ListenBrainz {
    pub fn new(config: &ListenBrainzConfig) -> Self {
        ListenBrainz {
            config: config.clone(),
        }
    }
    // Makes an api request to configured url with given json
    pub async fn api_request(
        &self,
        request: &str,
        endpoint: &String,
    ) -> Result<surf::Response, surf::Error> {

        surf::post(format!("{}{}", &self.config.api_url, endpoint))
            .body_string(request.to_owned())
            .header("Authorization", format!("Token {}", self.config.auth_token))
            .await
    }
}
