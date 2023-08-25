use std::ops::Deref;
use std::sync::RwLock;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;

use symphonia::core::audio::AudioBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions, Decoder};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::errors::Error;
use symphonia::core::units::Time;

use crate::music_player::music_output::AudioStream;
use crate::music_processor::music_processor::MusicProcessor;

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
    pub fn open_song<T: AsRef<str>>(&mut self, path: T) {
        // Creates mpsc channels to communicate with thread
        let (message_sender, message_receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();
        self.message_sender = Some(message_sender);
        self.status_receiver = Some(status_receiver);

        let owned_path = String::from(path.as_ref());
        
        // Creates thread that audio is decoded in
        thread::spawn(move || {
            let (mut reader, mut decoder) = MusicPlayer::get_reader_and_dec(owned_path);
            
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
    
    fn get_reader_and_dec<T: AsRef<str>>(path: T) -> (Box<dyn FormatReader>, Box<dyn Decoder>) {
        // Opens file and creates media source steram
        let src = std::fs::File::open(path.as_ref()).expect("Failed to open file");
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