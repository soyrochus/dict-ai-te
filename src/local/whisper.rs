use silero::{SampleRate, Session as VadSession, StreamState};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::AppError;
use crate::realtime::audio::AudioSpec;

use super::{LocalEngine, LocalOutput, LocalSessionConfig};

const SPEECH_THRESHOLD: f32 = 0.5;
// whisper.cpp refuses inputs shorter than 100 ms. Four 512-sample frames at
// 16 kHz are 128 ms and also filter isolated VAD spikes before inference.
const MIN_SPEECH_FRAMES: usize = 4;
const PARTIAL_FRAMES: usize = 16; // 512 @ 16 kHz ~= 32 ms; 16 frames ~= 500 ms.
const SILENCE_FRAMES: usize = 19; // ~= 600 ms.
const MAX_SEGMENT_FRAMES: usize = 469; // ~= 15 s.

pub struct WhisperEngine {
    context: Option<WhisperContext>,
    vad: Option<VadSession>,
    vad_stream: StreamState,
    segmenter: Segmenter,
    language_hint: Option<String>,
}

impl WhisperEngine {
    pub fn new() -> Self {
        Self {
            context: None,
            vad: None,
            vad_stream: StreamState::new(SampleRate::Rate16k),
            segmenter: Segmenter::default(),
            language_hint: None,
        }
    }

    fn decode(&self, audio: &[f32]) -> Result<String, AppError> {
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| AppError::Message("Whisper engine is not started".to_string()))?;
        let mut state = context
            .create_state()
            .map_err(|err| AppError::Message(format!("Whisper state failed: {err}")))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(self.language_hint.as_deref());
        params.set_translate(false);
        params.set_no_context(true);
        params.set_no_timestamps(true);
        params.set_single_segment(true);
        params.set_suppress_nst(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        state
            .full(params, audio)
            .map_err(|err| AppError::Message(format!("Whisper decode failed: {err}")))?;
        let text = state
            .as_iter()
            .filter_map(|segment| segment.to_str().ok())
            .collect::<String>()
            .trim()
            .to_string();
        Ok(text)
    }

    fn decode_action(&mut self, action: DecodeAction) -> Result<Vec<LocalOutput>, AppError> {
        let result = self.decode(&action.audio);
        self.segmenter.finish_decode();
        match result {
            Ok(text) if text.is_empty() => Ok(Vec::new()),
            Ok(text) if action.final_decode => Ok(vec![LocalOutput::Completed(text)]),
            Ok(text) => Ok(vec![LocalOutput::Update(text)]),
            Err(err) => {
                self.segmenter.drop_segment();
                Err(err)
            }
        }
    }
}

impl LocalEngine for WhisperEngine {
    fn audio_spec(&self) -> AudioSpec {
        AudioSpec::local_whisper()
    }

    fn start(&mut self, config: &LocalSessionConfig) -> Result<(), AppError> {
        let requested_gpu = match config.device.as_str() {
            "cpu" => false,
            "metal" => cfg!(all(feature = "metal", target_os = "macos")),
            "cuda" => cfg!(feature = "cuda"),
            "auto" => cfg!(any(
                all(feature = "metal", target_os = "macos"),
                feature = "cuda"
            )),
            other => {
                return Err(AppError::Message(format!(
                    "Unsupported local device '{other}'"
                )))
            }
        };
        let parameters = WhisperContextParameters {
            use_gpu: requested_gpu,
            ..WhisperContextParameters::default()
        };
        let context =
            WhisperContext::new_with_params(&config.model_path, parameters).or_else(|gpu_error| {
                if !requested_gpu {
                    return Err(gpu_error);
                }
                let cpu = WhisperContextParameters {
                    use_gpu: false,
                    ..WhisperContextParameters::default()
                };
                WhisperContext::new_with_params(&config.model_path, cpu)
            });
        let context = context
            .map_err(|err| AppError::Message(format!("Whisper model load failed: {err}")))?;
        self.language_hint =
            effective_language_hint(context.is_multilingual(), config.language_hint.as_deref());
        self.context = Some(context);
        self.vad = Some(
            VadSession::bundled()
                .map_err(|err| AppError::Message(format!("Silero VAD load failed: {err}")))?,
        );
        self.vad_stream.reset();
        self.segmenter = Segmenter::default();
        Ok(())
    }

    fn feed_frame(&mut self, frame: &[f32]) -> Result<Vec<LocalOutput>, AppError> {
        if frame.len() != AudioSpec::local_whisper().frame_samples {
            return Err(AppError::Audio(format!(
                "Local engine expected 512 samples, received {}",
                frame.len()
            )));
        }
        let probability = self
            .vad
            .as_mut()
            .ok_or_else(|| AppError::Message("Silero VAD is not started".to_string()))?
            .infer_chunk(&mut self.vad_stream, frame)
            .map_err(|err| AppError::Message(format!("Silero VAD failed: {err}")))?;
        match self.segmenter.push(probability >= SPEECH_THRESHOLD, frame) {
            Some(action) => self.decode_action(action),
            None => Ok(Vec::new()),
        }
    }

    fn stop(&mut self) -> Result<Vec<LocalOutput>, AppError> {
        match self.segmenter.finalize_open() {
            Some(action) => self.decode_action(action),
            None => Ok(Vec::new()),
        }
    }
}

#[derive(Debug)]
struct DecodeAction {
    audio: Vec<f32>,
    final_decode: bool,
}

#[derive(Default)]
struct Segmenter {
    audio: Vec<f32>,
    speech_frames: usize,
    silence_frames: usize,
    frames_since_partial: usize,
    decode_in_flight: bool,
}

impl Segmenter {
    fn push(&mut self, speech: bool, frame: &[f32]) -> Option<DecodeAction> {
        if !speech {
            if self.speech_frames == 0 {
                return None;
            }
            self.silence_frames += 1;
            if self.silence_frames >= SILENCE_FRAMES {
                return self.finalize_open();
            }
            return None;
        }

        self.silence_frames = 0;
        self.speech_frames += 1;
        self.frames_since_partial += 1;
        self.audio.extend_from_slice(frame);

        if self.speech_frames >= MAX_SEGMENT_FRAMES {
            return self.finalize_open();
        }
        if self.frames_since_partial >= PARTIAL_FRAMES && !self.decode_in_flight {
            self.frames_since_partial = 0;
            self.decode_in_flight = true;
            return Some(DecodeAction {
                audio: self.audio.clone(),
                final_decode: false,
            });
        }
        None
    }

    fn finalize_open(&mut self) -> Option<DecodeAction> {
        if self.audio.is_empty() {
            return None;
        }
        if self.speech_frames < MIN_SPEECH_FRAMES {
            self.drop_segment();
            return None;
        }
        let audio = std::mem::take(&mut self.audio);
        self.speech_frames = 0;
        self.silence_frames = 0;
        self.frames_since_partial = 0;
        self.decode_in_flight = true;
        Some(DecodeAction {
            audio,
            final_decode: true,
        })
    }

    fn finish_decode(&mut self) {
        self.decode_in_flight = false;
    }

    fn drop_segment(&mut self) {
        self.audio.clear();
        self.speech_frames = 0;
        self.silence_frames = 0;
        self.frames_since_partial = 0;
        self.decode_in_flight = false;
    }
}

fn effective_language_hint(multilingual: bool, configured: Option<&str>) -> Option<String> {
    if multilingual {
        configured.map(str::to_owned)
    } else {
        Some("en".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Vec<f32> {
        vec![0.25; 512]
    }

    #[test]
    fn silence_never_produces_decode_work() {
        let mut segmenter = Segmenter::default();
        for _ in 0..100 {
            assert!(segmenter.push(false, &frame()).is_none());
        }
        assert!(segmenter.audio.is_empty());
    }

    #[test]
    fn speech_onset_produces_periodic_partial() {
        let mut segmenter = Segmenter::default();
        for _ in 0..PARTIAL_FRAMES - 1 {
            assert!(segmenter.push(true, &frame()).is_none());
        }
        let action = segmenter.push(true, &frame()).unwrap();
        assert!(!action.final_decode);
        assert_eq!(action.audio.len(), PARTIAL_FRAMES * 512);
    }

    #[test]
    fn sustained_silence_closes_segment() {
        let mut segmenter = Segmenter::default();
        for _ in 0..MIN_SPEECH_FRAMES {
            segmenter.push(true, &frame());
        }
        for _ in 0..SILENCE_FRAMES - 1 {
            assert!(segmenter.push(false, &frame()).is_none());
        }
        assert!(segmenter.push(false, &frame()).unwrap().final_decode);
    }

    #[test]
    fn short_vad_blip_is_not_decoded() {
        let mut segmenter = Segmenter::default();
        for _ in 0..MIN_SPEECH_FRAMES - 1 {
            assert!(segmenter.push(true, &frame()).is_none());
        }
        for _ in 0..SILENCE_FRAMES {
            assert!(segmenter.push(false, &frame()).is_none());
        }
        assert!(segmenter.audio.is_empty());
    }

    #[test]
    fn english_only_model_never_auto_detects_language() {
        assert_eq!(effective_language_hint(false, None).as_deref(), Some("en"));
        assert_eq!(
            effective_language_hint(false, Some("es")).as_deref(),
            Some("en")
        );
    }

    #[test]
    fn multilingual_model_respects_configured_language() {
        assert_eq!(effective_language_hint(true, None), None);
        assert_eq!(
            effective_language_hint(true, Some("es")).as_deref(),
            Some("es")
        );
    }

    #[test]
    fn maximum_length_force_cuts_segment() {
        let mut segmenter = Segmenter::default();
        for index in 0..MAX_SEGMENT_FRAMES {
            if let Some(action) = segmenter.push(true, &frame()) {
                if action.final_decode {
                    assert_eq!(index + 1, MAX_SEGMENT_FRAMES);
                    return;
                }
                segmenter.finish_decode();
            }
        }
        panic!("segment was not force-cut");
    }

    #[test]
    fn overlapping_partial_is_skipped() {
        let mut segmenter = Segmenter::default();
        for _ in 0..PARTIAL_FRAMES {
            let _ = segmenter.push(true, &frame());
        }
        assert!(segmenter.decode_in_flight);
        for _ in 0..PARTIAL_FRAMES {
            assert!(segmenter.push(true, &frame()).is_none());
        }
    }
}
