use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample as SampleExt};
use parking_lot::Mutex;
use tokio::sync::mpsc as tokio_mpsc;

use crate::error::AppError;
use crate::realtime::audio::{
    base64_pcm16, chunk_pcm16, downmix_to_mono, pcm16_le, resample_linear, TARGET_SAMPLE_RATE,
};
use crate::realtime::events::RealtimeEvent;

const SAMPLE_QUEUE_CAPACITY: usize = 8;
const AUDIO_CHUNK_MS: u32 = 40;

pub struct LiveCapture {
    stream: Option<cpal::Stream>,
    worker: Option<thread::JoinHandle<()>>,
    sample_tx: Option<mpsc::SyncSender<Vec<f32>>>,
    level_bits: Arc<AtomicU32>,
    error_flag: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
struct CaptureConfig {
    sample_rate: u32,
    channels: u16,
}

impl LiveCapture {
    pub fn start(
        audio_tx: tokio_mpsc::Sender<String>,
        event_tx: mpsc::Sender<RealtimeEvent>,
    ) -> Result<Self, AppError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AppError::Audio("No default input device available".into()))?;
        let supported = choose_input_config(&device)?;
        let sample_format = supported.sample_format();
        let sample_rate = supported.sample_rate().0;
        let config: cpal::StreamConfig = supported.into();
        let capture_config = CaptureConfig {
            sample_rate,
            channels: config.channels,
        };

        let (sample_tx, sample_rx) = mpsc::sync_channel(SAMPLE_QUEUE_CAPACITY);
        let level_bits = Arc::new(AtomicU32::new(0));
        let error_flag = Arc::new(Mutex::new(None::<String>));

        let worker_events = event_tx.clone();
        let worker = thread::spawn(move || {
            audio_worker(capture_config, sample_rx, audio_tx, worker_events);
        });

        let stream = build_live_stream(
            sample_format,
            &device,
            &config,
            sample_tx.clone(),
            level_bits.clone(),
            error_flag.clone(),
            event_tx,
        )?;
        stream
            .play()
            .context("Failed to start live audio stream")
            .map_err(AppError::from)?;

        Ok(Self {
            stream: Some(stream),
            worker: Some(worker),
            sample_tx: Some(sample_tx),
            level_bits,
            error_flag,
        })
    }

    pub fn current_level(&self) -> f32 {
        f32::from_bits(self.level_bits.load(Ordering::Relaxed))
    }

    pub fn take_error(&self) -> Option<String> {
        self.error_flag.lock().take()
    }

    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.sample_tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for LiveCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

fn choose_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, AppError> {
    let supported_configs = device
        .supported_input_configs()
        .context("Failed to query device capabilities")
        .map_err(AppError::from)?;

    let desired_sample_rate = cpal::SampleRate(TARGET_SAMPLE_RATE);
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

    mono_exact
        .or(any_exact)
        .or(mono_fallback)
        .or(any_fallback)
        .ok_or_else(|| AppError::Audio("No supported capture configuration available".into()))
}

fn build_live_stream(
    sample_format: cpal::SampleFormat,
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_tx: mpsc::SyncSender<Vec<f32>>,
    level_bits: Arc<AtomicU32>,
    error_flag: Arc<Mutex<Option<String>>>,
    event_tx: mpsc::Sender<RealtimeEvent>,
) -> Result<cpal::Stream, AppError> {
    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _| on_audio_data(data, &sample_tx, &level_bits, &event_tx),
            move |err| capture_error(err, &error_flag),
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _| on_audio_data(data, &sample_tx, &level_bits, &event_tx),
            move |err| capture_error(err, &error_flag),
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _| on_audio_data(data, &sample_tx, &level_bits, &event_tx),
            move |err| capture_error(err, &error_flag),
            None,
        ),
        cpal::SampleFormat::I8 => device.build_input_stream(
            config,
            move |data: &[i8], _| on_audio_data(data, &sample_tx, &level_bits, &event_tx),
            move |err| capture_error(err, &error_flag),
            None,
        ),
        cpal::SampleFormat::U8 => device.build_input_stream(
            config,
            move |data: &[u8], _| on_audio_data(data, &sample_tx, &level_bits, &event_tx),
            move |err| capture_error(err, &error_flag),
            None,
        ),
        other => {
            return Err(AppError::Audio(format!(
                "Unsupported sample format: {other:?}"
            )))
        }
    }
    .context("Failed to build live input stream")
    .map_err(AppError::from)?;

    Ok(stream)
}

fn on_audio_data<T>(
    input: &[T],
    sample_tx: &mpsc::SyncSender<Vec<f32>>,
    level_bits: &Arc<AtomicU32>,
    event_tx: &mpsc::Sender<RealtimeEvent>,
) where
    T: cpal::Sample + SampleExt,
    f32: FromSample<T>,
{
    let mut max_amp = 0.0f32;
    let mut samples = Vec::with_capacity(input.len());
    for sample in input {
        let sample = SampleExt::to_sample::<f32>(*sample);
        max_amp = max_amp.max(sample.abs());
        samples.push(sample);
    }
    level_bits.store(max_amp.min(1.0).to_bits(), Ordering::Relaxed);

    match sample_tx.try_send(samples) {
        Ok(()) => {}
        Err(mpsc::TrySendError::Full(_)) => {
            let _ = event_tx.send(RealtimeEvent::Error {
                message: "Live audio queue is full; dropping microphone audio".to_string(),
            });
        }
        Err(mpsc::TrySendError::Disconnected(_)) => {}
    }
}

fn audio_worker(
    config: CaptureConfig,
    sample_rx: mpsc::Receiver<Vec<f32>>,
    audio_tx: tokio_mpsc::Sender<String>,
    event_tx: mpsc::Sender<RealtimeEvent>,
) {
    let chunk_samples = ((TARGET_SAMPLE_RATE * AUDIO_CHUNK_MS) / 1000).max(1) as usize;
    let mut pending = Vec::<f32>::with_capacity(chunk_samples * 2);

    while let Ok(samples) = sample_rx.recv() {
        let mono = downmix_to_mono(&samples, config.channels);
        let resampled = resample_linear(&mono, config.sample_rate, TARGET_SAMPLE_RATE);
        pending.extend(resampled);

        while pending.len() >= chunk_samples {
            let remainder = pending.split_off(chunk_samples);
            let pcm = pcm16_le(&pending);
            for chunk in chunk_pcm16(&pcm, TARGET_SAMPLE_RATE, AUDIO_CHUNK_MS) {
                if audio_tx.blocking_send(base64_pcm16(&chunk)).is_err() {
                    return;
                }
            }
            pending = remainder;
        }
    }

    if !pending.is_empty() {
        let pcm = pcm16_le(&pending);
        for chunk in chunk_pcm16(&pcm, TARGET_SAMPLE_RATE, AUDIO_CHUNK_MS) {
            if audio_tx.blocking_send(base64_pcm16(&chunk)).is_err() {
                return;
            }
        }
    }
    drop(audio_tx);
    let _ = event_tx.send(RealtimeEvent::SessionState {
        state: "audio.capture.stopped".to_string(),
    });
}

fn capture_error(err: cpal::StreamError, flag: &Arc<Mutex<Option<String>>>) {
    *flag.lock() = Some(err.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::TrySendError;

    #[test]
    fn bounded_channel_reports_full_without_blocking() {
        let (tx, _rx) = mpsc::sync_channel::<Vec<f32>>(1);
        tx.try_send(vec![0.0]).unwrap();
        assert!(matches!(tx.try_send(vec![1.0]), Err(TrySendError::Full(_))));
    }

    #[test]
    fn worker_converts_to_base64_pcm_chunks() {
        let (sample_tx, sample_rx) = mpsc::channel();
        let (audio_tx, mut audio_rx) = tokio_mpsc::channel(4);
        let (event_tx, _event_rx) = mpsc::channel();
        let config = CaptureConfig {
            sample_rate: TARGET_SAMPLE_RATE,
            channels: 1,
        };

        let handle = thread::spawn(move || audio_worker(config, sample_rx, audio_tx, event_tx));
        sample_tx
            .send(vec![0.0; (TARGET_SAMPLE_RATE / 25) as usize])
            .unwrap();
        drop(sample_tx);
        handle.join().unwrap();

        let chunk = audio_rx.blocking_recv().expect("audio chunk");
        assert!(!chunk.is_empty());
        assert!(audio_rx.blocking_recv().is_none());
    }

    #[test]
    fn worker_downmixes_resamples_and_flushes_partial_audio() {
        let (sample_tx, sample_rx) = mpsc::channel();
        let (audio_tx, mut audio_rx) = tokio_mpsc::channel(4);
        let (event_tx, _event_rx) = mpsc::channel();
        let config = CaptureConfig {
            sample_rate: 48_000,
            channels: 2,
        };

        let handle = thread::spawn(move || audio_worker(config, sample_rx, audio_tx, event_tx));
        sample_tx.send(vec![0.25, -0.25, 0.5, 0.5]).unwrap();
        drop(sample_tx);
        handle.join().unwrap();

        assert!(audio_rx.blocking_recv().is_some());
    }
}
