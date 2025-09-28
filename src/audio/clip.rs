use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::error::AppError;
use rodio::{Decoder, Source};

#[derive(Clone)]
pub struct AudioClip {
    pub sample_rate: u32,
    pub channels: u16,
    samples: Vec<f32>,
    wav_bytes: Option<Arc<Vec<u8>>>,
}

impl AudioClip {
    pub fn from_samples(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
            samples,
            wav_bytes: None,
        }
    }

    pub fn from_wav_bytes(bytes: Vec<u8>) -> Result<Self, AppError> {
        match Self::decode_wav_bytes(&bytes) {
            Ok(mut clip) => {
                clip.wav_bytes = Some(Arc::new(bytes));
                Ok(clip)
            }
            Err(_) => Self::decode_with_rodio(bytes),
        }
    }

    fn decode_wav_bytes(bytes: &[u8]) -> Result<Self, AppError> {
        let cursor = Cursor::new(bytes.to_vec());
        let mut reader = hound::WavReader::new(cursor)
            .context("Failed to parse WAV data")
            .map_err(AppError::from)?;
        let spec = reader.spec();
        let channels = spec.channels;
        let sample_rate = spec.sample_rate;
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .map(|res| res.unwrap_or(0.0))
                .collect(),
            hound::SampleFormat::Int => match spec.bits_per_sample {
                8 => reader
                    .samples::<i8>()
                    .map(|res| res.unwrap_or(0) as f32 / i8::MAX as f32)
                    .collect(),
                16 => reader
                    .samples::<i16>()
                    .map(|res| res.unwrap_or(0) as f32 / i16::MAX as f32)
                    .collect(),
                24 | 32 => reader
                    .samples::<i32>()
                    .map(|res| res.unwrap_or(0) as f32 / i32::MAX as f32)
                    .collect(),
                other => {
                    return Err(AppError::Audio(format!(
                        "Unsupported PCM bit depth: {other}"
                    )));
                }
            },
        };
        Ok(Self {
            sample_rate,
            channels,
            samples,
            wav_bytes: None,
        })
    }

    fn decode_with_rodio(bytes: Vec<u8>) -> Result<Self, AppError> {
        let cursor = Cursor::new(bytes.clone());
        let decoder = Decoder::new(cursor)
            .map_err(|err| AppError::Audio(format!("Failed to decode audio stream: {err}")))?;
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();
        let samples: Vec<f32> = decoder.convert_samples::<f32>().collect();
        Ok(Self {
            sample_rate,
            channels: channels as u16,
            samples,
            wav_bytes: Some(Arc::new(bytes)),
        })
    }

    pub fn duration(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let total_frames = self.samples.len() as f64 / self.channels as f64;
        let seconds = total_frames / self.sample_rate as f64;
        Duration::from_secs_f64(seconds)
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn wav_bytes(&mut self) -> Result<Arc<Vec<u8>>, AppError> {
        if let Some(bytes) = &self.wav_bytes {
            return Ok(bytes.clone());
        }
        let bytes = self.render_wav()?;
        let arc = Arc::new(bytes);
        self.wav_bytes = Some(arc.clone());
        Ok(arc)
    }

    pub fn level_at(&self, timestamp: Duration) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let window = Duration::from_millis(120);
        let frames_per_window = ((self.sample_rate as f64) * window.as_secs_f64()) as usize;
        let frames_per_window = frames_per_window.max(1);
        let center_frame = (timestamp.as_secs_f64() * self.sample_rate as f64) as usize;

        let channels = self.channels as usize;
        let total_frames = self.samples.len() / channels;
        if total_frames == 0 {
            return 0.0;
        }
        let start_frame = center_frame
            .saturating_sub(frames_per_window / 2)
            .min(total_frames);
        let end_frame = (start_frame + frames_per_window).min(total_frames);
        if start_frame >= end_frame {
            return 0.0;
        }

        let mut max_amp = 0.0f32;
        for frame in start_frame..end_frame {
            let idx = frame * channels;
            for ch in 0..channels {
                let sample = self.samples[idx + ch].abs();
                if sample > max_amp {
                    max_amp = sample;
                }
            }
        }
        max_amp.min(1.0)
    }

    fn render_wav(&self) -> Result<Vec<u8>, AppError> {
        let spec = hound::WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec)
                .context("Failed to create WAV writer")
                .map_err(AppError::from)?;
            for sample in &self.samples {
                let scaled = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                writer
                    .write_sample(scaled)
                    .context("Failed writing WAV sample")
                    .map_err(AppError::from)?;
            }
            writer
                .finalize()
                .context("Failed finalising WAV payload")
                .map_err(AppError::from)?;
        }
        Ok(cursor.into_inner())
    }
}
