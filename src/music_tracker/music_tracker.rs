use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Serialize, Deserialize, Serializer};
use md5::{Md5, Digest};
use discord_presence::{ Event, DiscordError};
use surf::StatusCode;

#[async_trait]
pub trait MusicTracker {
    /// Adds one listen to a song halfway through playback
    async fn track_song(&mut self, song: &String) -> Result<(), TrackerError>;
    
    /// Adds a 'listening' status to the music tracker service of choice
    async fn track_now(&mut self, song: &String) -> Result<(), TrackerError>;
    
    /// Reads config files, and attempts authentication with service
    async fn test_tracker(&mut self) -> Result<(), TrackerError>;
    
    /// Returns plays for a given song according to tracker service
    async fn get_times_tracked(&mut self, song: &String) -> Result<u32, TrackerError>;
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
        return match error.status() {
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

#[derive(Serialize, Deserialize, Clone)]
pub struct LastFMConfig {
    pub enabled: bool,
    pub dango_api_key: String,
    pub auth_token: Option<String>,
    pub shared_secret: Option<String>,
    pub session_key: Option<String>,
}

pub struct LastFM {
    config: LastFMConfig
}

#[async_trait]
impl MusicTracker for LastFM {
    async fn track_song(&mut self, song: &String) -> Result<(), TrackerError> {
        let mut params: BTreeMap<&str, &str> = BTreeMap::new();
        
        // Sets timestamp of song beginning play time
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("Your time is off.").as_secs() - 30;
        let string_timestamp = timestamp.to_string();
        params.insert("method", "track.scrobble");
        params.insert("artist", "Kikuo");
        params.insert("track", "A Happy Death - Again");
        params.insert("timestamp", &string_timestamp);
        
        return match self.api_request(params).await {
            Ok(_) => Ok(()),
            Err(err) => Err(TrackerError::from_surf_error(err)),
        }
    }
    
    async fn track_now(&mut self, song: &String) -> Result<(), TrackerError> {
        let mut params: BTreeMap<&str, &str> = BTreeMap::new();
        params.insert("method", "track.updateNowPlaying");
        params.insert("artist", "Kikuo");
        params.insert("track", "A Happy Death - Again");
        
        return match self.api_request(params).await {
            Ok(_) => Ok(()),
            Err(err) => Err(TrackerError::from_surf_error(err)),
        }
    }
    
    async fn test_tracker(&mut self) -> Result<(), TrackerError> {
        let mut params: BTreeMap<&str, &str> = BTreeMap::new();
        params.insert("method", "chart.getTopArtists");
        
        return match self.api_request(params).await {
            Ok(_) => Ok(()),
            Err(err) => Err(TrackerError::from_surf_error(err)),
        }
    }
    
    async fn get_times_tracked(&mut self, song: &String) -> Result<u32, TrackerError> {
        todo!();
    }
}

#[derive(Deserialize, Serialize)]
struct AuthToken {
    token: String
}

#[derive(Deserialize, Serialize, Debug)]
struct SessionResponse {
    name: String,
    key: String,
    subscriber: i32,
}

#[derive(Deserialize, Serialize, Debug)]
struct Session {
    session: SessionResponse
}

impl LastFM {
    // Returns a url to be accessed by the user
    pub async fn get_auth_url(&mut self) -> Result<String, surf::Error> {
        let method = String::from("auth.gettoken");
        let api_key = self.config.dango_api_key.clone();
        let api_request_url = format!("http://ws.audioscrobbler.com/2.0/?method={method}&api_key={api_key}&format=json");
        
        let auth_token: AuthToken = surf::get(api_request_url).await?.body_json().await?;
        self.config.auth_token = Some(auth_token.token.clone());
        
        let auth_url = format!("http://www.last.fm/api/auth/?api_key={api_key}&token={}", auth_token.token);
        
        return Ok(auth_url);
    }
    
    pub async fn set_session(&mut self) {
        let method = String::from("auth.getSession");
        let api_key = self.config.dango_api_key.clone();
        let auth_token = self.config.auth_token.clone().unwrap();
        let shared_secret = self.config.shared_secret.clone().unwrap();
        
        // Creates api_sig as defined in last.fm documentation
        let api_sig = format!("api_key{api_key}methodauth.getSessiontoken{auth_token}{shared_secret}");

        // Creates insecure MD5 hash for last.fm api sig
        let mut hasher = Md5::new();
        hasher.update(api_sig);
        let hash_result = hasher.finalize();
        let hex_string_hash = format!("{:#02x}", hash_result);
        
        let api_request_url = format!("http://ws.audioscrobbler.com/2.0/?method={method}&api_key={api_key}&token={auth_token}&api_sig={hex_string_hash}&format=json");
        
        let response = surf::get(api_request_url).recv_string().await.unwrap();
        
        // Sets session key from received response
        let session_response: Session = serde_json::from_str(&response).unwrap();
        self.config.session_key = Some(session_response.session.key.clone());
    }
    
    // Creates a new LastFM struct
    pub fn new(config: &LastFMConfig) -> LastFM {
        let last_fm = LastFM {
            config: config.clone()
        };
        return last_fm;
    }
    
    // Creates an api request with the given parameters
    pub async fn api_request(&self, mut params: BTreeMap<&str, &str>) -> Result<surf::Response, surf::Error> {
        params.insert("api_key", &self.config.dango_api_key);
        params.insert("sk", &self.config.session_key.as_ref().unwrap());
        
        // Creates and sets api call signature
        let api_sig = LastFM::request_sig(&params, &self.config.shared_secret.as_ref().unwrap());
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
        
        let response = surf::post(url).body_string(string_params).await;

        return response;
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
    
        return hashed_sig;
    }
    
    // Removes last.fm account from dango-music-player
    pub fn reset_account() {
        todo!();
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiscordRPCConfig {
    pub enabled: bool,
    pub dango_client_id: u64, 
    pub dango_icon: String,
}

pub struct DiscordRPC {
    config: DiscordRPCConfig,
    pub client: discord_presence::client::Client
}

impl DiscordRPC {
    pub fn new(config: &DiscordRPCConfig) -> Self {
        let rpc = DiscordRPC {
            client: discord_presence::client::Client::new(config.dango_client_id),
            config: config.clone(),
        };
        return rpc;
    }
}

#[async_trait]
impl MusicTracker for DiscordRPC {
    async fn track_now(&mut self, song: &String) -> Result<(), TrackerError> {
        let _client_thread = self.client.start();
        
        // Blocks thread execution until it has connected to local discord client
        let ready = self.client.block_until_event(Event::Ready);
        if ready.is_err() {
            return Err(TrackerError::ServiceUnavailable);
        }
        
        let start_time = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        // Sets discord account activity to current playing song
        let send_activity = self.client.set_activity(|activity| {
            activity
                .state(song)
                .assets(|assets| assets.large_image(&self.config.dango_icon))
                .timestamps(|time| time.start(start_time))
        });
        
        match send_activity {
            Ok(_) => return Ok(()),
            Err(_) => return Err(TrackerError::ServiceUnavailable),
        }
    }
    
    async fn track_song(&mut self, song: &String) -> Result<(), TrackerError> {
        return Ok(())
    }
    
    async fn test_tracker(&mut self) -> Result<(), TrackerError> {
        return Ok(())
    }
    
    async fn get_times_tracked(&mut self, song: &String) -> Result<u32, TrackerError> {
        return Ok(0);
    }
}