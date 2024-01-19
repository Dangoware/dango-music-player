use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::io::SeekFrom;

use async_std::io::ReadExt;
use async_std::task;

use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions, Decoder};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, MediaSource};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::errors::Error;
use symphonia::core::units::Time;

use futures::AsyncBufRead;

use crate::music_player::music_output::AudioStream;
use crate::music_processor::music_processor::MusicProcessor;
use crate::music_storage::music_db::URI;

// Struct that controls playback of music
pub struct MusicPlayer {
    pub music_processor: MusicProcessor,
    player_status: PlayerStatus,
    message_sender: Option<Sender<PlayerMessage>>,
    status_receiver: Option<Receiver<PlayerStatus>>,
}

#[derive(Clone, Copy)]
pub enum PlayerStatus {
    Playing,
    Paused,
    Stopped,
    Error,
}

pub enum PlayerMessage {
    Play,
    Pause,
    Stop,
    SeekTo(u64),
    DSP(DSPMessage)
}

pub enum DSPMessage {
    UpdateProcessor(Box<MusicProcessor>)
}

impl MusicPlayer {
    pub fn new() -> Self {
        MusicPlayer {
            music_processor: MusicProcessor::new(),
            player_status: PlayerStatus::Stopped,
            message_sender: None,
            status_receiver: None,
        }
    }
    
    // Opens and plays song with given path in separate thread
    pub fn open_song(&mut self, uri: &URI) {
        // Creates mpsc channels to communicate with thread
        let (message_sender, message_receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();
        self.message_sender = Some(message_sender);
        self.status_receiver = Some(status_receiver);
        
        let owned_uri = uri.clone();

        // Creates thread that audio is decoded in
        thread::spawn(move || {
            let (mut reader, mut decoder) = MusicPlayer::get_reader_and_dec(&owned_uri);
            
            let mut seek_time: Option<u64> = None;
            
            let mut audio_output: Option<Box<dyn AudioStream>> = None;
            
            let mut music_processor = MusicProcessor::new();
            
            'main_decode: loop {    
                // Handles message received from the MusicPlayer if there is one // TODO: Refactor
                let received_message = message_receiver.try_recv();
                match received_message {
                    Ok(PlayerMessage::Pause) => { 
                        status_sender.send(PlayerStatus::Paused).unwrap();
                        // Loops on a blocking message receiver to wait for a play/stop message
                        'inner_pause: loop {
                            let message = message_receiver.try_recv();
                            match message {
                                Ok(PlayerMessage::Play) => {
                                    status_sender.send(PlayerStatus::Playing).unwrap();
                                    break 'inner_pause
                                },
                                Ok(PlayerMessage::Stop) => {
                                    status_sender.send(PlayerStatus::Stopped).unwrap();
                                    break 'main_decode
                                },
                                _ => {},
                            }
                        }
                    },
                    // Exits main decode loop and subsequently ends thread (?)
                    Ok(PlayerMessage::Stop) => {
                        status_sender.send(PlayerStatus::Stopped).unwrap();
                        break 'main_decode
                    },
                    Ok(PlayerMessage::SeekTo(time)) => seek_time = Some(time),
                    Ok(PlayerMessage::DSP(dsp_message)) => {
                        match dsp_message {
                            DSPMessage::UpdateProcessor(new_processor) => music_processor = *new_processor,
                        }
                    }
                    _ => {},
                } 
                
                match seek_time {
                    Some(time) => {
                        let seek_to = SeekTo::Time { time: Time::from(time), track_id: Some(0) };
                        reader.seek(SeekMode::Accurate, seek_to).unwrap();
                        seek_time = None;
                    }
                    None => {} //Nothing to do!
                }
                
                let packet = match reader.next_packet() {
                    Ok(packet) => packet,
                    Err(Error::ResetRequired) => panic!(), //TODO,
                    Err(err) => {
                        //Unrecoverable?
                        panic!("{}", err);
                    }
                };
                
                match decoder.decode(&packet) {
                    Ok(decoded) => {
                        // Opens audio stream if there is not one
                        if audio_output.is_none() {
                            let spec = *decoded.spec();
                            let duration = decoded.capacity() as u64;
                            
                            audio_output.replace(crate::music_player::music_output::open_stream(spec, duration).unwrap());
                        }
                        
                        // Handles audio normally provided there is an audio stream
                        if let Some(ref mut audio_output) = audio_output {
                            // Changes buffer of the MusicProcessor if the packet has a differing capacity or spec
                            if music_processor.audio_buffer.capacity() != decoded.capacity() ||music_processor.audio_buffer.spec() != decoded.spec() {
                                let spec = *decoded.spec();
                                let duration = decoded.capacity() as u64;
                                
                                music_processor.set_buffer(duration, spec);
                            }
                            
                            let transformed_audio = music_processor.process(&decoded);
                            
                            // Writes transformed packet to audio out
                            audio_output.write(transformed_audio).unwrap()
                        }
                    },
                    Err(Error::IoError(_)) => {
                        // rest in peace packet
                        continue;
                    },
                    Err(Error::DecodeError(_)) => {
                        // may you one day be decoded
                        continue;
                    },
                    Err(err) => {
                        // Unrecoverable, though shouldn't panic here
                        panic!("{}", err);
                    }
                }
            }
        });
    }
    
    fn get_reader_and_dec(uri: &URI) -> (Box<dyn FormatReader>, Box<dyn Decoder>) {
        // Opens remote/local source and creates MediaSource for symphonia
        let config = RemoteOptions { media_buffer_len: 10000, forward_buffer_len: 10000};
        let src: Box<dyn MediaSource> = match uri {
            URI::Local(path) => Box::new(std::fs::File::open(path).expect("Failed to open file")),
            URI::Remote(_, location) => Box::new(RemoteSource::new(location.as_ref(), &config).unwrap()),
        };
        
        let mss = MediaSourceStream::new(src, Default::default());
        
        // Use default metadata and format options
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let mut hint = Hint::new();
        
        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts).expect("Unsupported format");
        
        let mut reader  = probed.format;
        
        let track = reader.tracks()
                    .iter()
                    .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                    .expect("no supported audio tracks");
                    
        let dec_opts: DecoderOptions = Default::default();
        
        let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)
                                                    .expect("unsupported codec");
        
        return (reader, decoder);
    }
    
    // Updates status by checking on messages from spawned thread
    fn update_status(&mut self) {
        let status = self.status_receiver.as_mut().unwrap().try_recv();
        if status.is_ok() {
            self.player_status = status.unwrap();
            match status.unwrap() {
                // Removes receiver and sender since spawned thread no longer exists
                PlayerStatus::Stopped => {
                    self.status_receiver = None;
                    self.message_sender = None;
                }
                _ => {}
            }
        }
    }
    
    // Sends message to spawned thread
    pub fn send_message(&mut self, message: PlayerMessage) {
        self.update_status();
        // Checks that message sender exists before sending a message off
        if self.message_sender.is_some() {
            self.message_sender.as_mut().unwrap().send(message).unwrap();
        }
    }
    
    pub fn get_status(&mut self) -> PlayerStatus {
        self.update_status();
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
                    },
                    Err(err) => return Err(err),
                }
            });
            match read_bytes {
                Err(err) => return Err(err),
                _ => {},
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
                if pos > self.media_buffer.len() as u64{
                    self.offset = self.media_buffer.len() as u64;
                } else {
                    self.offset = pos;
                }
                return Ok(self.offset);
            },
            // Offset is set to length of buffer + given position
            SeekFrom::End(pos) => {
                if self.media_buffer.len() as u64 + pos as u64 > self.media_buffer.len() as u64 {
                    self.offset = self.media_buffer.len() as u64;
                } else {
                    self.offset = self.media_buffer.len() as u64 + pos as u64;
                }
                return Ok(self.offset);
            },
            // Offset is set to current offset + given position
            SeekFrom::Current(pos) => {
                if self.offset + pos as u64 > self.media_buffer.len() as u64{
                    self.offset = self.media_buffer.len() as u64;
                } else {
                    self.offset += pos as u64
                }
                return Ok(self.offset);
            },
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