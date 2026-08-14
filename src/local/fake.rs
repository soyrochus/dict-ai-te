use std::collections::VecDeque;

use crate::error::AppError;
use crate::realtime::audio::AudioSpec;

use super::{LocalEngine, LocalOutput, LocalSessionConfig};

pub struct FakeLocalEngine {
    script: VecDeque<Vec<LocalOutput>>,
    final_output: Vec<LocalOutput>,
}

impl FakeLocalEngine {
    pub fn new(script: Vec<Vec<LocalOutput>>, final_output: Vec<LocalOutput>) -> Self {
        Self {
            script: script.into(),
            final_output,
        }
    }
}

impl LocalEngine for FakeLocalEngine {
    fn audio_spec(&self) -> AudioSpec {
        AudioSpec::local_whisper()
    }

    fn start(&mut self, _config: &LocalSessionConfig) -> Result<(), AppError> {
        Ok(())
    }

    fn feed_frame(&mut self, _frame: &[f32]) -> Result<Vec<LocalOutput>, AppError> {
        Ok(self.script.pop_front().unwrap_or_default())
    }

    fn stop(&mut self) -> Result<Vec<LocalOutput>, AppError> {
        Ok(std::mem::take(&mut self.final_output))
    }
}
