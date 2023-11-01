use std::io::SeekFrom;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;

use async_std::io::ReadExt;
use async_std::task;

use futures::future::join_all;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

use futures::AsyncBufRead;

use crate::music_controller::config::Config;
use crate::music_player::music_output::AudioStream;
use crate::music_processor::music_processor::MusicProcessor;
use crate::music_storage::music_db::{Song, URI};
use crate::music_tracker::music_tracker::{
    DiscordRPC, LastFM, ListenBrainz, MusicTracker, TrackerError,
};

// Struct that controls playback of music
pub struct MusicPlayer {
    pub music_processor: MusicProcessor,
    player_status: PlayerStatus,
    music_trackers: Vec<Box<dyn MusicTracker + Send>>,
    current_song: Arc<RwLock<Option<Song>>>,
    message_sender: Sender<DecoderMessage>,
    status_receiver: Receiver<PlayerStatus>,
    config: Arc<RwLock<Config>>,
}

#[derive(Clone, Copy, Debug)]
pub enum PlayerStatus {
    Playing(f64),
    Paused,
    Stopped,
    Error,
}

#[derive(Debug, Clone)]
pub enum DecoderMessage {
    OpenSong(Song),
    Play,
    Pause,
    Stop,
    SeekTo(u64),
    DSP(DSPMessage),
}

#[derive(Clone)]
pub enum TrackerMessage {
    Track(Song),
    TrackNow(Song),
}

#[derive(Debug, Clone)]
pub enum DSPMessage {
    UpdateProcessor(Box<MusicProcessor>),
}

// Holds a song decoder reader, etc
struct SongHandler {
    pub reader: Box<dyn FormatReader>,
    pub decoder: Box<dyn Decoder>,
    pub time_base: Option<TimeBase>,
    pub duration: Option<u64>,
}

// TODO: actual error handling here
impl SongHandler {
    pub fn new(uri: &URI) -> Result<Self, ()> {
        // Opens remote/local source and creates MediaSource for symphonia
        let config = RemoteOptions {
            media_buffer_len: 10000,
            forward_buffer_len: 10000,
        };
        let src: Box<dyn MediaSource> = match uri {
            URI::Local(path) => match std::fs::File::open(path) {
                Ok(file) => Box::new(file),
                Err(_) => return Err(()),
            },
            URI::Remote(_, location) => {
                match RemoteSource::new(location.to_str().unwrap(), &config) {
                    Ok(remote_source) => Box::new(remote_source),
                    Err(_) => return Err(()),
                }
            }
            _ => todo!(),
        };

        let mss = MediaSourceStream::new(src, Default::default());

        // Use default metadata and format options
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let hint = Hint::new();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .expect("Unsupported format");

        let reader = probed.format;

        let track = reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .expect("no supported audio tracks");

        let time_base = track.codec_params.time_base;
        let duration = track.codec_params.n_frames;

        let dec_opts: DecoderOptions = Default::default();

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)
            .expect("unsupported codec");

        return Ok(SongHandler {
            reader,
            decoder,
            time_base,
            duration,
        });
    }
}

impl MusicPlayer {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        // Creates mpsc channels to communicate with music player threads
        let (message_sender, message_receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();
        let current_song = Arc::new(RwLock::new(None));

        MusicPlayer::start_player(
            message_receiver,
            status_sender,
            config.clone(),
            current_song.clone(),
        );

        MusicPlayer {
            music_processor: MusicProcessor::new(),
            music_trackers: Vec::new(),
            player_status: PlayerStatus::Stopped,
            current_song,
            message_sender,
            status_receiver,
            config,
        }
    }

    fn start_tracker(
        status_sender: Sender<Result<(), TrackerError>>,
        tracker_receiver: Receiver<TrackerMessage>,
        config: Arc<RwLock<Config>>,
    ) {
        thread::spawn(move || {
            let global_config = &*config.read().unwrap();
            // Sets local config for trackers to detect changes
            let local_config = global_config.clone();
            let mut trackers: Vec<Box<dyn MusicTracker>> = Vec::new();
            // Updates local trackers to the music controller config // TODO: refactor
            let update_trackers = |trackers: &mut Vec<Box<dyn MusicTracker>>| {
                if let Some(lastfm_config) = global_config.lastfm.clone() {
                    if lastfm_config.enabled {
                        trackers.push(Box::new(LastFM::new(&lastfm_config)));
                    }
                }
                if let Some(discord_config) = global_config.discord.clone() {
                    if discord_config.enabled {
                        trackers.push(Box::new(DiscordRPC::new(&discord_config)));
                    }
                }
                if let Some(listenbz_config) = global_config.listenbrainz.clone() {
                    if listenbz_config.enabled {
                        trackers.push(Box::new(ListenBrainz::new(&listenbz_config)));
                    }
                }
            };
            update_trackers(&mut trackers);
            loop {
                if let message = tracker_receiver.recv() {
                    if local_config != global_config {
                        update_trackers(&mut trackers);
                    }

                    let mut results = Vec::new();
                    task::block_on(async {
                        let mut futures = Vec::new();
                        for tracker in trackers.iter_mut() {
                            match message.clone() {
                                Ok(TrackerMessage::Track(song)) => {
                                    futures.push(tracker.track_song(song))
                                }
                                Ok(TrackerMessage::TrackNow(song)) => {
                                    futures.push(tracker.track_now(song))
                                }
                                Err(_) => {}
                            }
                        }
                        results = join_all(futures).await;
                    });

                    for result in results {
                        status_sender.send(result).unwrap_or_default()
                    }
                }
            }
        });
    }

    // Opens and plays song with given path in separate thread
    fn start_player(
        message_receiver: Receiver<DecoderMessage>,
        status_sender: Sender<PlayerStatus>,
        config: Arc<RwLock<Config>>,
        current_song: Arc<RwLock<Option<Song>>>,
    ) {
        // Creates thread that audio is decoded in
        thread::spawn(move || {
            let current_song = current_song;

            let mut song_handler = None;

            let mut seek_time: Option<u64> = None;

            let mut audio_output: Option<Box<dyn AudioStream>> = None;

            let mut music_processor = MusicProcessor::new();

            let (tracker_sender, tracker_receiver): (
                Sender<TrackerMessage>,
                Receiver<TrackerMessage>,
            ) = mpsc::channel();
            let (tracker_status_sender, tracker_status_receiver): (
                Sender<Result<(), TrackerError>>,
                Receiver<Result<(), TrackerError>>,
            ) = mpsc::channel();

            MusicPlayer::start_tracker(tracker_status_sender, tracker_receiver, config);

            let mut song_tracked = false;
            let mut song_time = 0.0;
            let mut paused = true;
            'main_decode: loop {
                'handle_message: loop {
                    let message = if paused {
                        // Pauses playback by blocking on waiting for new player messages
                        match message_receiver.recv() {
                            Ok(message) => Some(message),
                            Err(_) => None,
                        }
                    } else {
                        // Resumes playback by not blocking
                        match message_receiver.try_recv() {
                            Ok(message) => Some(message),
                            Err(_) => break 'handle_message,
                        }
                    };
                    // Handles message received from MusicPlayer struct
                    match message {
                        Some(DecoderMessage::OpenSong(song)) => {
                            let song_uri = song.location.clone();
                            match SongHandler::new(&song_uri) {
                                Ok(new_handler) => {
                                    song_handler = Some(new_handler);
                                    *current_song.write().unwrap() = Some(song);
                                    paused = false;
                                    song_tracked = false;
                                }
                                Err(_) => status_sender.send(PlayerStatus::Error).unwrap(),
                            }
                        }
                        Some(DecoderMessage::Play) => {
                            if song_handler.is_some() {
                                paused = false;
                            }
                        }
                        Some(DecoderMessage::Pause) => {
                            paused = true;
                            status_sender.send(PlayerStatus::Paused).unwrap();
                        }
                        Some(DecoderMessage::SeekTo(time)) => seek_time = Some(time),
                        Some(DecoderMessage::DSP(dsp_message)) => match dsp_message {
                            DSPMessage::UpdateProcessor(new_processor) => {
                                music_processor = *new_processor
                            }
                        },
                        // Exits main decode loop and subsequently ends thread
                        Some(DecoderMessage::Stop) => {
                            status_sender.send(PlayerStatus::Stopped).unwrap();
                            break 'main_decode;
                        }
                        None => {}
                    }
                    status_sender.send(PlayerStatus::Error).unwrap();
                }
                // In theory this check should not need to occur?
                if let (Some(song_handler), current_song) =
                    (&mut song_handler, &*current_song.read().unwrap())
                {
                    match seek_time {
                        Some(time) => {
                            let seek_to = SeekTo::Time {
                                time: Time::from(time),
                                track_id: Some(0),
                            };
                            song_handler
                                .reader
                                .seek(SeekMode::Accurate, seek_to)
                                .unwrap();
                            seek_time = None;
                        }
                        None => {} //Nothing to do!
                    }
                    let packet = match song_handler.reader.next_packet() {
                        Ok(packet) => packet,
                        Err(Error::ResetRequired) => panic!(), //TODO,
                        Err(err) => {
                            // Unrecoverable?
                            panic!("{}", err);
                        }
                    };

                    if let (Some(time_base), Some(song)) = (song_handler.time_base, current_song) {
                        let time_units = time_base.calc_time(packet.ts);
                        song_time = time_units.seconds as f64 + time_units.frac;
                        // Tracks song now if song has just started
                        if song_time == 0.0 {
                            tracker_sender
                                .send(TrackerMessage::TrackNow(song.clone()))
                                .unwrap();
                        }

                        if let Some(duration) = song_handler.duration {
                            let song_duration = time_base.calc_time(duration);
                            let song_duration_secs =
                                song_duration.seconds as f64 + song_duration.frac;
                            // Tracks song if current time is past half of total song duration or past 4 minutes
                            if (song_duration_secs / 2.0 < song_time || song_time > 240.0)
                                && !song_tracked
                            {
                                song_tracked = true;
                                tracker_sender
                                    .send(TrackerMessage::Track(song.clone()))
                                    .unwrap();
                            }
                        }
                    }

                    status_sender
                        .send(PlayerStatus::Playing(song_time))
                        .unwrap();

                    match song_handler.decoder.decode(&packet) {
                        Ok(decoded) => {
                            // Opens audio stream if there is not one
                            if audio_output.is_none() {
                                let spec = *decoded.spec();
                                let duration = decoded.capacity() as u64;

                                audio_output.replace(
                                    crate::music_player::music_output::open_stream(spec, duration)
                                        .unwrap(),
                                );
                            }
                            // Handles audio normally provided there is an audio stream
                            if let Some(ref mut audio_output) = audio_output {
                                // Changes buffer of the MusicProcessor if the packet has a differing capacity or spec
                                if music_processor.audio_buffer.capacity() != decoded.capacity()
                                    || music_processor.audio_buffer.spec() != decoded.spec()
                                {
                                    let spec = *decoded.spec();
                                    let duration = decoded.capacity() as u64;

                                    music_processor.set_buffer(duration, spec);
                                }
                                let transformed_audio = music_processor.process(&decoded);

                                // Writes transformed packet to audio out
                                audio_output.write(transformed_audio).unwrap()
                            }
                        }
                        Err(Error::IoError(_)) => {
                            // rest in peace packet
                            continue;
                        }
                        Err(Error::DecodeError(_)) => {
                            // may you one day be decoded
                            continue;
                        }
                        Err(err) => {
                            // Unrecoverable, though shouldn't panic here
                            panic!("{}", err);
                        }
                    }
                }
            }
        });
    }

    // Updates status by checking on messages from spawned thread
    fn update_player(&mut self) {
        for message in self.status_receiver.try_recv() {
            self.player_status = message;
        }
    }

    pub fn get_current_song(&self) -> Option<Song> {
        match self.current_song.try_read() {
            Ok(song) => return (*song).clone(),
            Err(_) => return None,
        }
    }

    // Sends message to spawned thread
    pub fn send_message(&mut self, message: DecoderMessage) {
        self.update_player();
        // Checks that message sender exists before sending a message off
        self.message_sender.send(message).unwrap();
    }

    pub fn get_status(&mut self) -> PlayerStatus {
        self.update_player();
        return self.player_status;
    }
}

// TODO: Make the buffer length do anything
/// Options for remote sources
///
/// media_buffer_len is how many bytes are to be buffered in totala
///
/// forward_buffer is how many bytes can ahead of the seek position without the remote source being read from
pub struct RemoteOptions {
    media_buffer_len: u64,
    forward_buffer_len: u64,
}

impl Default for RemoteOptions {
    fn default() -> Self {
        RemoteOptions {
            media_buffer_len: 100000,
            forward_buffer_len: 1024,
        }
    }
}

/// A remote source of media
struct RemoteSource {
    reader: Box<dyn AsyncBufRead + Send + Sync + Unpin>,
    media_buffer: Vec<u8>,
    forward_buffer_len: u64,
    offset: u64,
}

impl RemoteSource {
    /// Creates a new RemoteSource with given uri and configuration
    pub fn new(uri: &str, config: &RemoteOptions) -> Result<Self, surf::Error> {
        let mut response = task::block_on(async {
            return surf::get(uri).await;
        })?;

        let reader = response.take_body().into_reader();

        Ok(RemoteSource {
            reader,
            media_buffer: Vec::new(),
            forward_buffer_len: config.forward_buffer_len,
            offset: 0,
        })
    }
}
// TODO: refactor this + buffer into the buffer passed into the function, not a newly allocated one
impl std::io::Read for RemoteSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Reads bytes into the media buffer if the offset is within the specified distance from the end of the buffer
        if self.media_buffer.len() as u64 - self.offset < self.forward_buffer_len {
            let mut buffer = [0; 1024];
            let read_bytes = task::block_on(async {
                match self.reader.read_exact(&mut buffer).await {
                    Ok(_) => {
                        self.media_buffer.extend_from_slice(&buffer);
                        return Ok(());
                    }
                    Err(err) => return Err(err),
                }
            });
            match read_bytes {
                Err(err) => return Err(err),
                _ => {}
            }
        }
        // Reads bytes from the media buffer into the buffer given by
        let mut bytes_read = 0;
        for location in 0..1024 {
            if (location + self.offset as usize) < self.media_buffer.len() {
                buf[location] = self.media_buffer[location + self.offset as usize];
                bytes_read += 1;
            }
        }

        self.offset += bytes_read;
        return Ok(bytes_read as usize);
    }
}

impl std::io::Seek for RemoteSource {
    // Seeks to a given position
    // Seeking past the internal buffer's length results in the seeking to the end of content
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            // Offset is set to given position
            SeekFrom::Start(pos) => {
                if pos > self.media_buffer.len() as u64 {
                    self.offset = self.media_buffer.len() as u64;
                } else {
                    self.offset = pos;
                }
                return Ok(self.offset);
            }
            // Offset is set to length of buffer + given position
            SeekFrom::End(pos) => {
                if self.media_buffer.len() as u64 + pos as u64 > self.media_buffer.len() as u64 {
                    self.offset = self.media_buffer.len() as u64;
                } else {
                    self.offset = self.media_buffer.len() as u64 + pos as u64;
                }
                return Ok(self.offset);
            }
            // Offset is set to current offset + given position
            SeekFrom::Current(pos) => {
                if self.offset + pos as u64 > self.media_buffer.len() as u64 {
                    self.offset = self.media_buffer.len() as u64;
                } else {
                    self.offset += pos as u64
                }
                return Ok(self.offset);
            }
        }
    }
}

impl MediaSource for RemoteSource {
    fn is_seekable(&self) -> bool {
        return true;
    }

    fn byte_len(&self) -> Option<u64> {
        return None;
    }
}
