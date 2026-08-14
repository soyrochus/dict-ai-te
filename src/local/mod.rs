use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use crate::error::AppError;
use crate::realtime::audio::AudioSpec;
use crate::realtime::events::RealtimeEvent;
use crate::settings::{Backend, Settings};

#[cfg(feature = "test-local-engine")]
pub mod fake;
pub mod model;
#[cfg(feature = "local-whisper")]
pub mod whisper;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveBackend {
    OpenAi,
    Local,
}

pub const LOCAL_BACKEND_AVAILABLE: bool = cfg!(any(
    feature = "local-whisper",
    feature = "test-local-engine"
));

pub fn effective_live_backend(settings: &Settings) -> LiveBackend {
    if settings.backend == Backend::Local && LOCAL_BACKEND_AVAILABLE {
        LiveBackend::Local
    } else {
        LiveBackend::OpenAi
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
pub struct LocalSessionConfig {
    pub language_hint: Option<String>,
    pub model_path: PathBuf,
    pub device: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
pub enum LocalOutput {
    Update(String),
    Completed(String),
}

pub trait LocalEngine: Send + 'static {
    fn audio_spec(&self) -> AudioSpec;
    fn start(&mut self, config: &LocalSessionConfig) -> Result<(), AppError>;
    fn feed_frame(&mut self, frame: &[f32]) -> Result<Vec<LocalOutput>, AppError>;
    fn stop(&mut self) -> Result<Vec<LocalOutput>, AppError>;
}

pub struct LocalSession {
    cancelled: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl LocalSession {
    pub fn stop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        // Inference may be in native code and cannot be interrupted safely. Detach rather than
        // blocking the UI; the worker observes cancellation before accepting another frame,
        // finalizes the open segment, and emits `disconnected`.
        self.worker.take();
    }
}

impl Drop for LocalSession {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start_local_session(
    mut engine: Box<dyn LocalEngine>,
    config: LocalSessionConfig,
    mut audio_rx: tokio::sync::mpsc::Receiver<Vec<f32>>,
    event_tx: mpsc::Sender<RealtimeEvent>,
) -> LocalSession {
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let worker = thread::Builder::new()
        .name("dictaite-local-inference".to_string())
        .spawn(move || {
            send_state(&event_tx, "local.loading");
            if let Err(err) = engine.start(&config) {
                send_error(&event_tx, err);
                send_state(&event_tx, "disconnected");
                return;
            }
            send_state(&event_tx, "connecting");
            send_state(&event_tx, "local.listening");

            let mut segment_number = 0_u64;
            while !worker_cancelled.load(Ordering::Acquire) {
                let Some(frame) = audio_rx.blocking_recv() else {
                    break;
                };
                match engine.feed_frame(&frame) {
                    Ok(outputs) => {
                        if !outputs.is_empty() {
                            send_state(&event_tx, "local.processing");
                            emit_outputs(&event_tx, &mut segment_number, outputs);
                            send_state(&event_tx, "local.listening");
                        }
                    }
                    Err(err) => {
                        send_error(&event_tx, err);
                        segment_number += 1;
                    }
                }
            }

            send_state(&event_tx, "local.processing");
            match engine.stop() {
                Ok(outputs) => emit_outputs(&event_tx, &mut segment_number, outputs),
                Err(err) => send_error(&event_tx, err),
            }
            send_state(&event_tx, "disconnected");
        })
        .expect("failed to spawn local inference thread");

    LocalSession {
        cancelled,
        worker: Some(worker),
    }
}

fn emit_outputs(
    event_tx: &mpsc::Sender<RealtimeEvent>,
    segment_number: &mut u64,
    outputs: Vec<LocalOutput>,
) {
    for output in outputs {
        let item_id = Some(format!("local-{segment_number}"));
        debug_assert!(item_id.as_deref().is_some_and(|id| !id.is_empty()));
        let event = match output {
            LocalOutput::Update(text) => RealtimeEvent::SourceUpdate { item_id, text },
            LocalOutput::Completed(text) => {
                *segment_number += 1;
                RealtimeEvent::SourceCompleted { item_id, text }
            }
        };
        let _ = event_tx.send(event);
    }
}

fn send_state(event_tx: &mpsc::Sender<RealtimeEvent>, state: &str) {
    let _ = event_tx.send(RealtimeEvent::SessionState {
        state: state.to_string(),
    });
}

fn send_error(event_tx: &mpsc::Sender<RealtimeEvent>, error: AppError) {
    let _ = event_tx.send(RealtimeEvent::Error {
        message: format!("Local recognition error: {error}"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScriptedEngine {
        calls: usize,
    }

    impl LocalEngine for ScriptedEngine {
        fn audio_spec(&self) -> AudioSpec {
            AudioSpec::local_whisper()
        }

        fn start(&mut self, config: &LocalSessionConfig) -> Result<(), AppError> {
            assert_eq!(config.language_hint.as_deref(), Some("es"));
            Ok(())
        }

        fn feed_frame(&mut self, _frame: &[f32]) -> Result<Vec<LocalOutput>, AppError> {
            self.calls += 1;
            Ok(match self.calls {
                1 => vec![LocalOutput::Update("hola".into())],
                2 => vec![LocalOutput::Completed("hola mundo".into())],
                _ => Vec::new(),
            })
        }

        fn stop(&mut self) -> Result<Vec<LocalOutput>, AppError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn runner_emits_stable_nonempty_ids_and_finalizes_before_disconnect() {
        let (audio_tx, audio_rx) = tokio::sync::mpsc::channel(4);
        let (event_tx, event_rx) = mpsc::channel();
        let config = LocalSessionConfig {
            language_hint: Some("es".into()),
            model_path: PathBuf::from("unused"),
            device: "cpu".into(),
        };
        let _session = start_local_session(
            Box::new(ScriptedEngine { calls: 0 }),
            config,
            audio_rx,
            event_tx,
        );
        audio_tx.blocking_send(vec![0.0; 512]).unwrap();
        audio_tx.blocking_send(vec![0.0; 512]).unwrap();
        drop(audio_tx);

        let events: Vec<_> = event_rx.iter().collect();
        let transcript_events: Vec<_> = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    RealtimeEvent::SourceUpdate { .. } | RealtimeEvent::SourceCompleted { .. }
                )
            })
            .collect();
        assert!(matches!(
            transcript_events[0],
            RealtimeEvent::SourceUpdate { item_id: Some(id), .. } if id == "local-0"
        ));
        assert!(matches!(
            transcript_events[1],
            RealtimeEvent::SourceCompleted { item_id: Some(id), .. } if id == "local-0"
        ));
        assert!(matches!(
            events.last(),
            Some(RealtimeEvent::SessionState { state }) if state == "disconnected"
        ));
    }

    #[cfg(feature = "test-local-engine")]
    #[test]
    fn fake_pipeline_routes_settings_frames_events_and_assembler_without_remote_config() {
        use crate::local::fake::FakeLocalEngine;
        use crate::realtime::transcript::TranscriptAssembler;
        use crate::settings::{Backend, Settings};

        let settings = Settings {
            backend: Backend::Local,
            ..Settings::default()
        };
        assert_eq!(effective_live_backend(&settings), LiveBackend::Local);
        let engine = FakeLocalEngine::new(
            vec![
                vec![LocalOutput::Update("recognise beach".into())],
                vec![LocalOutput::Update("recognise speech".into())],
            ],
            vec![LocalOutput::Completed("recognise speech".into())],
        );
        let (audio_tx, audio_rx) = tokio::sync::mpsc::channel(4);
        let (event_tx, event_rx) = mpsc::channel();
        let config = LocalSessionConfig {
            language_hint: None,
            model_path: PathBuf::from("fake-does-not-use-a-model"),
            device: "cpu".into(),
        };
        let _session = start_local_session(Box::new(engine), config, audio_rx, event_tx);
        audio_tx.blocking_send(vec![0.0; 512]).unwrap();
        audio_tx.blocking_send(vec![0.0; 512]).unwrap();
        drop(audio_tx);

        let mut assembler = TranscriptAssembler::default();
        for event in event_rx.iter() {
            match event {
                RealtimeEvent::SourceUpdate { item_id, text } => {
                    assembler.update(item_id.as_deref(), &text)
                }
                RealtimeEvent::SourceCompleted { item_id, text } => {
                    assembler.complete(item_id.as_deref(), &text)
                }
                RealtimeEvent::SessionState { state } if state == "disconnected" => break,
                _ => {}
            }
        }
        assert_eq!(assembler.text(), "recognise speech");
    }
}
