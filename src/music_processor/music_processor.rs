use symphonia::core::audio::{AudioBuffer, AudioBufferRef, Signal, AsAudioBufferRef, SignalSpec};

#[derive(Clone)]
pub struct MusicProcessor {
    pub audio_buffer: AudioBuffer<f32>,
    pub audio_volume: f32,
}

impl MusicProcessor {
    /// Returns new MusicProcessor with blank buffer and 100% volume
    pub fn new() -> Self {
        MusicProcessor {
            audio_buffer: AudioBuffer::unused(),
            audio_volume: 1.0,
        }
    }
    
    /// Processes audio samples
    /// 
    /// Currently only supports transformations of volume
    pub fn process(&mut self, audio_buffer_ref: &AudioBufferRef) -> AudioBufferRef {
        audio_buffer_ref.convert(&mut self.audio_buffer);
        
        let process = |sample| sample * self.audio_volume;
        
        self.audio_buffer.transform(process);
        
        return self.audio_buffer.as_audio_buffer_ref();
    }
    
    /// Sets buffer of the MusicProcessor
    pub fn set_buffer(&mut self, duration: u64, spec: SignalSpec) {
        self.audio_buffer = AudioBuffer::new(duration, spec);
    }
}