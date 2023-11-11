use std::{result, thread};

use symphonia::core::audio::{AudioBufferRef, RawSample, SampleBuffer, SignalSpec};
use symphonia::core::conv::{ConvertibleSample, FromSample, IntoSample};
use symphonia::core::units::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{self, SizedSample};

use rb::*;

use crate::music_player::music_resampler::Resampler;

pub trait AudioStream {
    fn write(&mut self, decoded: AudioBufferRef<'_>) -> Result<()>;
    fn flush(&mut self);
}

#[derive(Debug)]
pub enum AudioOutputError {
    OpenStreamError,
    PlayStreamError,
    StreamClosedError,
}

pub type Result<T> = result::Result<T, AudioOutputError>;

pub trait OutputSample:
    SizedSample
    + FromSample<f32>
    + IntoSample<f32>
    + cpal::Sample
    + ConvertibleSample
    + RawSample
    + std::marker::Send
    + 'static
{
}

pub struct AudioOutput<T>
where
    T: OutputSample,
{
    ring_buf_producer: rb::Producer<T>,
    sample_buf: SampleBuffer<T>,
    stream: cpal::Stream,
    resampler: Option<Resampler<T>>,
}
impl OutputSample for i8 {}
impl OutputSample for i16 {}
impl OutputSample for i32 {}
//impl OutputSample for i64 {}
impl OutputSample for u8 {}
impl OutputSample for u16 {}
impl OutputSample for u32 {}
//impl OutputSample for u64 {}
impl OutputSample for f32 {}
impl OutputSample for f64 {}
//create a new trait with functions, then impl that somehow

pub fn open_stream(spec: SignalSpec, duration: Duration) -> Result<Box<dyn AudioStream>> {
    let host = cpal::default_host();

    // Uses default audio device
    let device = match host.default_output_device() {
        Some(device) => device,
        _ => return Err(AudioOutputError::OpenStreamError),
    };

    let config = match device.default_output_config() {
        Ok(config) => config,
        Err(err) => return Err(AudioOutputError::OpenStreamError),
    };

    return match config.sample_format() {
        cpal::SampleFormat::I8 => {
            AudioOutput::<i8>::create_stream(spec, &device, &config.into(), duration)
        }
        cpal::SampleFormat::I16 => {
            AudioOutput::<i16>::create_stream(spec, &device, &config.into(), duration)
        }
        cpal::SampleFormat::I32 => {
            AudioOutput::<i32>::create_stream(spec, &device, &config.into(), duration)
        }
        //cpal::SampleFormat::I64 => AudioOutput::<i64>::create_stream(spec, &device, &config.into(), duration),
        cpal::SampleFormat::U8 => {
            AudioOutput::<u8>::create_stream(spec, &device, &config.into(), duration)
        }
        cpal::SampleFormat::U16 => {
            AudioOutput::<u16>::create_stream(spec, &device, &config.into(), duration)
        }
        cpal::SampleFormat::U32 => {
            AudioOutput::<u32>::create_stream(spec, &device, &config.into(), duration)
        }
        //cpal::SampleFormat::U64 => AudioOutput::<u64>::create_stream(spec, &device, &config.into(), duration),
        cpal::SampleFormat::F32 => {
            AudioOutput::<f32>::create_stream(spec, &device, &config.into(), duration)
        }
        cpal::SampleFormat::F64 => {
            AudioOutput::<f64>::create_stream(spec, &device, &config.into(), duration)
        }
        _ => todo!(),
    };
}

impl<T: OutputSample> AudioOutput<T> {
    // Creates the stream (TODO: Merge w/open_stream?)
    fn create_stream(
        spec: SignalSpec,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        duration: Duration,
    ) -> Result<Box<dyn AudioStream>> {
        let num_channels = config.channels as usize;

        // Ring buffer is created with 200ms audio capacity
        let ring_len = ((50 * config.sample_rate.0 as usize) / 1000) * num_channels;
        let ring_buf = rb::SpscRb::new(ring_len);

        let ring_buf_producer = ring_buf.producer();
        let ring_buf_consumer = ring_buf.consumer();

        let stream_result = device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                // Writes samples in the ring buffer to the audio output
                let written = ring_buf_consumer.read(data).unwrap_or(0);

                // Mutes non-written samples
                data[written..]
                    .iter_mut()
                    .for_each(|sample| *sample = T::MID);
            },
            //TODO: Handle error here properly
            move |err| println!("Yeah we erroring out here"),
            None,
        );

        if let Err(err) = stream_result {
            return Err(AudioOutputError::OpenStreamError);
        }

        let stream = stream_result.unwrap();

        //Start output stream
        if let Err(err) = stream.play() {
            return Err(AudioOutputError::PlayStreamError);
        }

        let sample_buf = SampleBuffer::<T>::new(duration, spec);

        let mut resampler = None;
        if spec.rate != config.sample_rate.0 {
            println!("Resampling enabled");
            resampler = Some(Resampler::new(
                spec,
                config.sample_rate.0 as usize,
                duration,
            ))
        }

        Ok(Box::new(AudioOutput {
            ring_buf_producer,
            sample_buf,
            stream,
            resampler,
        }))
    }
}

impl<T: OutputSample> AudioStream for AudioOutput<T> {
    // Writes given samples to ring buffer
    fn write(&mut self, decoded: AudioBufferRef<'_>) -> Result<()> {
        if decoded.frames() == 0 {
            return Ok(());
        }

        let mut samples: &[T] = if let Some(resampler) = &mut self.resampler {
            // Resamples if required
            match resampler.resample(decoded) {
                Some(resampled) => resampled,
                None => return Ok(()),
            }
        } else {
            self.sample_buf.copy_interleaved_ref(decoded);
            self.sample_buf.samples()
        };

        // Write samples into ring buffer
        while let Some(written) = self.ring_buf_producer.write_blocking(samples) {
            samples = &samples[written..];
        }

        Ok(())
    }

    // Flushes resampler if needed
    fn flush(&mut self) {
        if let Some(resampler) = &mut self.resampler {
            let mut stale_samples = resampler.flush().unwrap_or_default();

            while let Some(written) = self.ring_buf_producer.write_blocking(stale_samples) {
                stale_samples = &stale_samples[written..];
            }
        }

        let _ = self.stream.pause();
    }
}
