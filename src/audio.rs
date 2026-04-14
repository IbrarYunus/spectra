use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::Arc;

pub struct AudioBuffer {
    data: Mutex<Vec<f32>>,
    capacity: usize,
}

impl AudioBuffer {
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            data: Mutex::new(Vec::with_capacity(capacity)),
            capacity,
        })
    }

    pub fn push(&self, samples: &[f32]) {
        let mut buf = self.data.lock();
        buf.extend_from_slice(samples);
        let len = buf.len();
        if len > self.capacity {
            buf.drain(..len - self.capacity);
        }
    }

    pub fn snapshot(&self, n: usize) -> Vec<f32> {
        let buf = self.data.lock();
        let len = buf.len();
        if len >= n {
            buf[len - n..].to_vec()
        } else {
            let mut v = vec![0.0; n - len];
            v.extend_from_slice(&buf);
            v
        }
    }
}

#[allow(dead_code)]
pub enum Keepalive {
    Mic(cpal::Stream),
    File(rodio::OutputStream, rodio::Sink),
    System(Box<dyn std::any::Any>),
}

pub struct AudioSource {
    pub sample_rate: u32,
    pub buffer: Arc<AudioBuffer>,
    pub source_label: String,
    pub _keepalive: Keepalive,
}

pub fn start_microphone(device_name: Option<&str>) -> Result<AudioSource> {
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .input_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| anyhow!("Input device '{}' not found", name))?,
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow!("No default input device"))?,
    };

    let label = device.name().unwrap_or_else(|_| "mic".into());
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let buffer = AudioBuffer::new(sample_rate as usize * 4);

    let err_fn = |e| eprintln!("audio stream error: {e}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let buf = buffer.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| {
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|c| c.iter().sum::<f32>() / channels as f32)
                        .collect();
                    buf.push(&mono);
                },
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let buf = buffer.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|c| {
                            c.iter().map(|s| *s as f32 / i16::MAX as f32).sum::<f32>()
                                / channels as f32
                        })
                        .collect();
                    buf.push(&mono);
                },
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let buf = buffer.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[u16], _: &_| {
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|c| {
                            c.iter()
                                .map(|s| (*s as f32 - 32768.0) / 32768.0)
                                .sum::<f32>()
                                / channels as f32
                        })
                        .collect();
                    buf.push(&mono);
                },
                err_fn,
                None,
            )?
        }
        fmt => return Err(anyhow!("Unsupported sample format: {:?}", fmt)),
    };

    stream.play()?;
    Ok(AudioSource {
        sample_rate,
        buffer,
        source_label: format!("mic: {}", label),
        _keepalive: Keepalive::Mic(stream),
    })
}

use rodio::Source;
use std::time::Duration;

struct TappingSource<S> {
    inner: S,
    buffer: Arc<AudioBuffer>,
    channels: u16,
    batch: Vec<f32>,
}

impl<S: Source<Item = f32>> Iterator for TappingSource<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = self.inner.next()?;
        self.batch.push(s);
        if self.batch.len() >= self.channels as usize * 512 {
            let mono: Vec<f32> = self
                .batch
                .chunks(self.channels as usize)
                .map(|c| c.iter().sum::<f32>() / self.channels as f32)
                .collect();
            self.buffer.push(&mono);
            self.batch.clear();
        }
        Some(s)
    }
}

impl<S: Source<Item = f32>> Source for TappingSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

pub fn start_file(path: &str) -> Result<AudioSource> {
    use rodio::{Decoder, OutputStream, Sink};
    use std::fs::File;
    use std::io::BufReader;

    let (stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let file = File::open(path)?;
    let decoder = Decoder::new(BufReader::new(file))?;
    let source = decoder.convert_samples::<f32>();
    let sample_rate = source.sample_rate();
    let channels = source.channels();

    let buffer = AudioBuffer::new(sample_rate as usize * 4);
    let tap = TappingSource {
        inner: source,
        buffer: buffer.clone(),
        channels,
        batch: Vec::with_capacity(channels as usize * 512),
    };

    sink.append(tap);
    sink.play();

    Ok(AudioSource {
        sample_rate,
        buffer,
        source_label: format!("file: {}", path),
        _keepalive: Keepalive::File(stream, sink),
    })
}
