use std::io::Cursor;
use std::time::{Duration, Instant};

use crate::audio::AudioClip;
use crate::error::AppError;

pub struct AudioPlayer {
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
    current: Option<PlaybackHandle>,
}

pub struct PlaybackHandle {
    clip: AudioClip,
    sink: rodio::Sink,
    started: Instant,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, AppError> {
        let (stream, handle) = rodio::OutputStream::try_default()
            .map_err(|err| AppError::Audio(format!("Output device error: {err}")))?;
        Ok(Self {
            _stream: stream,
            handle,
            current: None,
        })
    }

    pub fn play(&mut self, mut clip: AudioClip) -> Result<(), AppError> {
        let wav_bytes = clip.wav_bytes()?;
        let cursor = Cursor::new((*wav_bytes).clone());
        let decoder = rodio::Decoder::new(cursor)
            .map_err(|err| AppError::Audio(format!("Decode error: {err}")))?;
        let sink = rodio::Sink::try_new(&self.handle)
            .map_err(|err| AppError::Audio(format!("Audio sink error: {err}")))?;
        sink.append(decoder);
        sink.play();
        self.current = Some(PlaybackHandle {
            clip,
            sink,
            started: Instant::now(),
        });
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(playback) = self.current.take() {
            playback.sink.stop();
        }
    }

    pub fn refresh(&mut self) {
        if let Some(handle) = &self.current {
            if handle.sink.empty() {
                self.current = None;
            }
        }
    }

    pub fn is_playing(&self) -> bool {
        self.current
            .as_ref()
            .map(|handle| !handle.sink.empty())
            .unwrap_or(false)
    }

    pub fn elapsed(&self) -> Duration {
        self.current
            .as_ref()
            .map(|handle| handle.started.elapsed())
            .unwrap_or_default()
    }

    pub fn duration(&self) -> Duration {
        self.current
            .as_ref()
            .map(|handle| handle.clip.duration())
            .unwrap_or_default()
    }

    pub fn level(&self) -> f32 {
        self.current
            .as_ref()
            .map(|handle| handle.clip.level_at(handle.started.elapsed()))
            .unwrap_or(0.0)
    }
}
