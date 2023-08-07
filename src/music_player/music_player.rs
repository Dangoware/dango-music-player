use std::path::PathBuf;
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;

use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions, Decoder};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::errors::Error;
use symphonia::core::units::Time;

use crate::music_player::music_output::AudioStream;

// Struct that controls playback of music
pub struct MusicPlayer {
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

#[derive(Debug)]
pub enum PlayerMessage {
    Play,
    Pause,
    Stop,
    SeekTo(u64),
}

impl MusicPlayer {
    pub fn new() -> Self {
        MusicPlayer {
            player_status: PlayerStatus::Stopped,
            message_sender: None,
            status_receiver: None,
        }
    }
    
    // Opens and plays song with given path in separate thread
    pub fn open_song(&mut self, path: &Box<PathBuf>) {
        // Creates mspc channels to communicate with thread
        let (message_sender, message_receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();
        self.message_sender = Some(message_sender);
        self.status_receiver = Some(status_receiver);
        
        let cloned = path.clone();
        
        // Creates thread that audio is decoded in
        thread::spawn(move || {
            
            let (mut reader, mut decoder) = MusicPlayer::get_reader_and_dec(cloned);
            
            let mut seek_time: Option<u64> = None;
            
            let mut audio_output: Option<Box<dyn AudioStream>> = None;
            
            'main_decode: loop {
                // Handles message received from the MusicPlayer if there is one
                let received_message = message_receiver.try_recv();
                if received_message.is_ok() {
                match received_message.unwrap() {
                    PlayerMessage::Pause => { 
                        status_sender.send(PlayerStatus::Paused).unwrap();
                        // Loops on a blocking message receiver to wait for a play/stop message
                        'inner_pause: loop {
                            let message = message_receiver.try_recv();
                            if message.is_ok() {
                                match message.unwrap() {
                                    PlayerMessage::Play => {
                                        status_sender.send(PlayerStatus::Playing).unwrap();
                                        break 'inner_pause
                                    },
                                    PlayerMessage::Stop => {
                                        status_sender.send(PlayerStatus::Stopped).unwrap();
                                        break 'main_decode
                                    },
                                    _ => {},
                                }
                            }
                        }
                    },
                    // Exits main decode loop and subsequently ends thread (?)
                    PlayerMessage::Stop => {
                        status_sender.send(PlayerStatus::Stopped).unwrap();
                        break 'main_decode
                    },
                    PlayerMessage::SeekTo(time) => seek_time = Some(time),
                    _ => {},
                } }
                
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
                        
                        if let Some(ref mut audio_output) = audio_output {
                            // Writes decoded packet to audio out
                            
                            audio_output.write(decoded).unwrap()
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
    
    fn get_reader_and_dec(path: Box<PathBuf>) -> (Box<dyn FormatReader>, Box<dyn Decoder>) {
        // Opens file and creates media source steram
        let src = std::fs::File::open(*path).expect("Failed to open file");
        let mss = MediaSourceStream::new(Box::new(src), Default::default());
        
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