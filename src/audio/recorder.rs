use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample as SampleExt};
use parking_lot::Mutex;

use crate::audio::AudioClip;
use crate::error::AppError;

pub struct Recorder {
    handle: Option<RecorderHandle>,
    last_error: Option<String>,
}

pub struct RecorderHandle {
    stream: Option<cpal::Stream>,
    shared: Arc<SharedBuffer>,
    sample_rate: u32,
    channels: u16,
    started: Instant,
}

struct SharedBuffer {
    samples: Mutex<Vec<f32>>,
    level_bits: AtomicU32,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            handle: None,
            last_error: None,
        }
    }

    pub fn start(&mut self) -> Result<(), AppError> {
        if self.handle.is_some() {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AppError::Audio("No default input device available".into()))?;
        let supported_configs = device
            .supported_input_configs()
            .context("Failed to query device capabilities")
            .map_err(AppError::from)?;

        let desired_sample_rate = cpal::SampleRate(16_000);
        let mut mono_exact = None;
        let mut any_exact = None;
        let mut mono_fallback = None;
        let mut any_fallback = None;
        for config in supported_configs {
            let supports_desired = config.min_sample_rate() <= desired_sample_rate
                && config.max_sample_rate() >= desired_sample_rate;

            if config.channels() == 1 && supports_desired && mono_exact.is_none() {
                mono_exact = Some(config.with_sample_rate(desired_sample_rate));
            }
            if supports_desired && any_exact.is_none() {
                any_exact = Some(config.with_sample_rate(desired_sample_rate));
            }
            if config.channels() == 1 && mono_fallback.is_none() {
                mono_fallback = Some(config.with_max_sample_rate());
            }
            if any_fallback.is_none() {
                any_fallback = Some(config.with_max_sample_rate());
            }
        }

        let supported = mono_exact
            .or(any_exact)
            .or(mono_fallback)
            .or(any_fallback)
            .ok_or_else(|| {
                AppError::Audio("No supported capture configuration available".into())
            })?;
        let sample_format = supported.sample_format();
        let sample_rate = supported.sample_rate().0;
        let config: cpal::StreamConfig = supported.into();

        let shared = Arc::new(SharedBuffer {
            samples: Mutex::new(Vec::new()),
            level_bits: AtomicU32::new(0),
        });

        let shared_clone = shared.clone();
        let err_flag = Arc::new(Mutex::new(None::<String>));
        let err_clone = err_flag.clone();

        let stream = build_input_stream(sample_format, &device, &config, shared_clone, err_clone)?;
        stream
            .play()
            .context("Failed to start audio stream")
            .map_err(AppError::from)?;

        self.handle = Some(RecorderHandle {
            stream: Some(stream),
            shared,
            sample_rate,
            channels: config.channels,
            started: Instant::now(),
        });

        if let Some(err) = err_flag.lock().take() {
            self.last_error = Some(err);
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<Option<AudioClip>, AppError> {
        if let Some(mut handle) = self.handle.take() {
            if let Some(stream) = handle.stream.take() {
                drop(stream);
            }
            let samples = {
                let mut guard = handle.shared.samples.lock();
                std::mem::take(&mut *guard)
            };
            if samples.is_empty() {
                return Ok(None);
            }
            let clip = AudioClip::from_samples(samples, handle.sample_rate, handle.channels);
            return Ok(Some(clip));
        }
        Ok(None)
    }

    pub fn current_level(&self) -> f32 {
        self.handle
            .as_ref()
            .map(|handle| f32::from_bits(handle.shared.level_bits.load(Ordering::Relaxed)))
            .unwrap_or(0.0)
    }

    pub fn elapsed(&self) -> Duration {
        self.handle
            .as_ref()
            .map(|handle| handle.started.elapsed())
            .unwrap_or_default()
    }
}

fn build_input_stream(
    sample_format: cpal::SampleFormat,
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Arc<SharedBuffer>,
    err_flag: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, AppError> {
    let shared_clone = shared.clone();

    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _| on_audio_data(data, &shared_clone),
            move |err| capture_error(err, &err_flag),
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _| on_audio_data(data, &shared_clone),
            move |err| capture_error(err, &err_flag),
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _| on_audio_data(data, &shared_clone),
            move |err| capture_error(err, &err_flag),
            None,
        ),
        cpal::SampleFormat::I8 => device.build_input_stream(
            config,
            move |data: &[i8], _| on_audio_data(data, &shared_clone),
            move |err| capture_error(err, &err_flag),
            None,
        ),
        cpal::SampleFormat::U8 => device.build_input_stream(
            config,
            move |data: &[u8], _| on_audio_data(data, &shared_clone),
            move |err| capture_error(err, &err_flag),
            None,
        ),
        other => {
            return Err(AppError::Audio(format!(
                "Unsupported sample format: {other:?}"
            )));
        }
    }
    .context("Failed to build input stream")
    .map_err(AppError::from)?;

    Ok(stream)
}

fn on_audio_data<T>(input: &[T], shared: &Arc<SharedBuffer>)
where
    T: cpal::Sample + SampleExt,
    f32: FromSample<T>,
{
    let mut max_amp = 0.0f32;
    {
        let mut buffer = shared.samples.lock();
        buffer.reserve(input.len());
        for frame in input {
            let sample = SampleExt::to_sample::<f32>(*frame);
            max_amp = max_amp.max(sample.abs());
            buffer.push(sample);
        }
    }
    shared
        .level_bits
        .store(max_amp.min(1.0).to_bits(), Ordering::Relaxed);
}

fn capture_error(err: cpal::StreamError, flag: &Arc<Mutex<Option<String>>>) {
    *flag.lock() = Some(err.to_string());
}
