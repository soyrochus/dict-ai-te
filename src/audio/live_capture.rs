use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample as SampleExt};
use parking_lot::Mutex;
use tokio::sync::mpsc as tokio_mpsc;

use crate::error::AppError;
use crate::realtime::audio::{downmix_to_mono, resample_linear, AudioSpec};
use crate::realtime::events::RealtimeEvent;

const SAMPLE_QUEUE_CAPACITY: usize = 8;

pub struct LiveCapture {
    stream: Option<cpal::Stream>,
    worker: Option<thread::JoinHandle<()>>,
    sample_tx: Option<mpsc::SyncSender<Vec<f32>>>,
    stop_requested: Arc<AtomicBool>,
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
        spec: AudioSpec,
        audio_tx: tokio_mpsc::Sender<Vec<f32>>,
        event_tx: mpsc::Sender<RealtimeEvent>,
    ) -> Result<Self, AppError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AppError::Audio("No default input device available".into()))?;
        let supported = choose_input_config(&device, spec.sample_rate)?;
        let sample_format = supported.sample_format();
        let sample_rate = supported.sample_rate().0;
        let config: cpal::StreamConfig = supported.into();
        let capture_config = CaptureConfig {
            sample_rate,
            channels: config.channels,
        };

        let (sample_tx, sample_rx) = mpsc::sync_channel(SAMPLE_QUEUE_CAPACITY);
        let level_bits = Arc::new(AtomicU32::new(0));
        let stop_requested = Arc::new(AtomicBool::new(false));
        let error_flag = Arc::new(Mutex::new(None::<String>));

        let worker_events = event_tx.clone();
        let worker_stop = stop_requested.clone();
        let worker = thread::spawn(move || {
            audio_worker(
                spec,
                capture_config,
                sample_rx,
                audio_tx,
                worker_events,
                worker_stop,
            );
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
            stop_requested,
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
        self.stop_requested.store(true, Ordering::Release);
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

fn choose_input_config(
    device: &cpal::Device,
    target_sample_rate: u32,
) -> Result<cpal::SupportedStreamConfig, AppError> {
    let supported_configs = device
        .supported_input_configs()
        .context("Failed to query device capabilities")
        .map_err(AppError::from)?;

    let desired_sample_rate = cpal::SampleRate(target_sample_rate);
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
    spec: AudioSpec,
    config: CaptureConfig,
    sample_rx: mpsc::Receiver<Vec<f32>>,
    audio_tx: tokio_mpsc::Sender<Vec<f32>>,
    event_tx: mpsc::Sender<RealtimeEvent>,
    stop_requested: Arc<AtomicBool>,
) {
    let mut pending = Vec::<f32>::with_capacity(spec.frame_samples * 2);

    'capture: while let Ok(samples) = sample_rx.recv() {
        let mono = downmix_to_mono(&samples, config.channels);
        let resampled = resample_linear(&mono, config.sample_rate, spec.sample_rate);
        pending.extend(resampled);

        while pending.len() >= spec.frame_samples {
            let remainder = pending.split_off(spec.frame_samples);
            if !send_audio_frame(&audio_tx, pending, &stop_requested) {
                pending = Vec::new();
                break 'capture;
            }
            pending = remainder;
        }
        if stop_requested.load(Ordering::Acquire) && pending.is_empty() {
            break;
        }
    }

    if !pending.is_empty() {
        pending.resize(spec.frame_samples, 0.0);
        let _ = send_audio_frame(&audio_tx, pending, &stop_requested);
    }
    drop(audio_tx);
    let _ = event_tx.send(RealtimeEvent::SessionState {
        state: "audio.capture.stopped".to_string(),
    });
}

fn send_audio_frame(
    audio_tx: &tokio_mpsc::Sender<Vec<f32>>,
    mut frame: Vec<f32>,
    stop_requested: &AtomicBool,
) -> bool {
    loop {
        match audio_tx.try_send(frame) {
            Ok(()) => return true,
            Err(tokio_mpsc::error::TrySendError::Closed(_)) => return false,
            Err(tokio_mpsc::error::TrySendError::Full(returned)) => {
                if stop_requested.load(Ordering::Acquire) {
                    return false;
                }
                frame = returned;
                thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }
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
    fn worker_emits_exact_frames_across_misaligned_callbacks() {
        let (sample_tx, sample_rx) = mpsc::channel();
        let (audio_tx, mut audio_rx) = tokio_mpsc::channel(4);
        let (event_tx, _event_rx) = mpsc::channel();
        let config = CaptureConfig {
            sample_rate: 24_000,
            channels: 1,
        };

        let handle = thread::spawn(move || {
            audio_worker(
                AudioSpec::openai(),
                config,
                sample_rx,
                audio_tx,
                event_tx,
                Arc::new(AtomicBool::new(false)),
            )
        });
        sample_tx.send(vec![0.25; 500]).unwrap();
        sample_tx.send(vec![0.5; 500]).unwrap();
        drop(sample_tx);
        handle.join().unwrap();

        let first = audio_rx.blocking_recv().expect("first audio frame");
        let second = audio_rx.blocking_recv().expect("padded audio frame");
        assert_eq!(first.len(), 960);
        assert_eq!(second.len(), 960);
        assert_eq!(&first[..500], vec![0.25; 500]);
        assert_eq!(&first[500..], vec![0.5; 460]);
        assert_eq!(&second[..40], vec![0.5; 40]);
        assert!(second[40..].iter().all(|sample| *sample == 0.0));
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

        let handle = thread::spawn(move || {
            audio_worker(
                AudioSpec::local_whisper(),
                config,
                sample_rx,
                audio_tx,
                event_tx,
                Arc::new(AtomicBool::new(false)),
            )
        });
        sample_tx
            .send(vec![
                0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0, 1.0,
            ])
            .unwrap();
        drop(sample_tx);
        handle.join().unwrap();

        let frame = audio_rx.blocking_recv().expect("padded local frame");
        assert_eq!(frame.len(), 512);
        assert!((frame[0] - 0.0).abs() < f32::EPSILON);
        assert!((frame[1] - 0.75).abs() < f32::EPSILON);
        assert!(frame[2..].iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn full_callback_queue_emits_dropped_audio_error() {
        let (sample_tx, _sample_rx) = mpsc::sync_channel::<Vec<f32>>(1);
        let (event_tx, event_rx) = mpsc::channel();
        let level = Arc::new(AtomicU32::new(0));
        sample_tx.try_send(vec![0.0]).unwrap();

        on_audio_data(&[1.0f32], &sample_tx, &level, &event_tx);

        assert!(matches!(
            event_rx.try_recv(),
            Ok(RealtimeEvent::Error { message }) if message.contains("dropping microphone audio")
        ));
    }

    #[test]
    fn stopping_cancels_a_worker_blocked_on_a_full_backend_queue() {
        let (sample_tx, sample_rx) = mpsc::channel();
        let (audio_tx, _audio_rx) = tokio_mpsc::channel(1);
        audio_tx.try_send(vec![0.0; 960]).unwrap();
        let (event_tx, event_rx) = mpsc::channel();
        let stop_requested = Arc::new(AtomicBool::new(true));
        let config = CaptureConfig {
            sample_rate: 24_000,
            channels: 1,
        };

        sample_tx.send(vec![0.25; 960]).unwrap();
        drop(sample_tx);
        audio_worker(
            AudioSpec::openai(),
            config,
            sample_rx,
            audio_tx,
            event_tx,
            stop_requested,
        );

        assert!(matches!(
            event_rx.try_recv(),
            Ok(RealtimeEvent::SessionState { state }) if state == "audio.capture.stopped"
        ));
    }
}
