use std::fs;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use eframe::App;
use egui::{self, Align, Color32, Context, Frame, Layout, RichText, Ui, Vec2};

use crate::audio::{AudioClip, AudioPlayer, LiveCapture};
use crate::constants::{FEMALE_VOICES, LANGUAGES, MALE_VOICES, VOICE_SAMPLE_TEXT};
use crate::error::AppError;
use crate::local::model::{
    availability as model_availability, manifest_entry, model_path, resolve_model_path,
    ModelAvailability, ModelDownload, ModelManifestEntry, MODEL_MANIFEST,
};
use crate::local::{
    effective_live_backend, start_local_session, LiveBackend, LocalEngine, LocalSession,
    LocalSessionConfig, LOCAL_BACKEND_AVAILABLE,
};
use crate::openai::OpenAiClient;
use crate::realtime::audio::AudioSpec;
use crate::realtime::events::RealtimeEvent;
use crate::realtime::state::LiveState;
use crate::realtime::transcript::TranscriptAssembler;
use crate::realtime::transport::{
    run_live_transcription, run_live_translation, RealtimeSessionConfig,
};
use crate::settings::{effective_translate, load_settings, save_settings, Backend, Settings};

pub struct DictaiteApp {
    live_capture: Option<LiveCapture>,
    live_runtime: Option<tokio::runtime::Runtime>,
    live_event_tx: mpsc::Sender<RealtimeEvent>,
    live_event_rx: mpsc::Receiver<RealtimeEvent>,
    live_stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    local_session: Option<LocalSession>,
    active_backend: LiveBackend,
    live_state: LiveState,
    is_recording: bool,
    record_started_at: Option<Instant>,
    player: Option<AudioPlayer>,
    player_error: Option<String>,
    openai: Option<OpenAiClient>,

    settings: Settings,
    settings_modal: Option<SettingsModal>,

    origin_language_index: usize,
    translate_enabled: bool,
    target_language_index: usize,

    transcript: String,
    raw_transcript: Option<String>,
    source_transcript: String,
    translated_transcript: String,
    source_assembler: TranscriptAssembler,

    preferred_gender: VoiceGender,

    tts_clip: Option<AudioClip>,
    tts_voice_id: Option<String>,

    tts_task: Option<BackgroundTask<TtsOutcome>>,
    model_download: Option<ModelDownload>,
    model_download_entry: Option<ModelManifestEntry>,

    status_text: String,
    error_text: Option<String>,
    copy_feedback_until: Option<Instant>,
}

impl DictaiteApp {
    pub fn new(openai: Option<OpenAiClient>) -> Self {
        let settings = load_settings();
        let origin_language_index = language_index(settings.default_language.as_deref());
        let target_language_index =
            language_index(settings.default_target_language.as_deref()).max(1);

        let (player, player_error) = match AudioPlayer::new() {
            Ok(player) => (Some(player), None),
            Err(err) => (None, Some(err.to_string())),
        };

        let (live_event_tx, live_event_rx) = mpsc::channel();
        let mut app = Self {
            live_capture: None,
            live_runtime: None,
            live_event_tx,
            live_event_rx,
            live_stop_tx: None,
            local_session: None,
            active_backend: LiveBackend::OpenAi,
            live_state: LiveState::Disconnected,
            is_recording: false,
            record_started_at: None,
            player,
            player_error,
            openai,
            settings,
            settings_modal: None,
            origin_language_index,
            translate_enabled: false,
            target_language_index,
            transcript: String::new(),
            raw_transcript: None,
            source_transcript: String::new(),
            translated_transcript: String::new(),
            source_assembler: TranscriptAssembler::default(),
            preferred_gender: VoiceGender::Female,
            tts_clip: None,
            tts_voice_id: None,
            tts_task: None,
            model_download: None,
            model_download_entry: None,
            status_text: "Press to start listening".to_string(),
            error_text: None,
            copy_feedback_until: None,
        };
        app.apply_settings_defaults();
        app.maybe_warn_api_key();
        app
    }

    fn apply_settings_defaults(&mut self) {
        self.origin_language_index = language_index(self.settings.default_language.as_deref());
        self.translate_enabled =
            effective_translate(&self.settings, self.settings.translate_by_default);
        let target_idx = language_index(self.settings.default_target_language.as_deref()).max(1);
        self.target_language_index = target_idx;
    }

    fn maybe_warn_api_key(&mut self) {
        if effective_live_backend(&self.settings) == LiveBackend::OpenAi && self.openai.is_none() {
            self.error_text = Some("OPENAI_API_KEY not configured".to_string());
        }
    }

    fn start_recording(&mut self) {
        if self.is_recording {
            return;
        }
        self.tts_task = None;
        self.error_text = None;
        self.source_assembler = TranscriptAssembler::default();
        self.source_transcript.clear();
        self.translated_transcript.clear();
        self.transcript.clear();
        self.raw_transcript = None;
        self.tts_clip = None;
        self.tts_voice_id = None;

        self.active_backend = effective_live_backend(&self.settings);
        if self.settings.backend == Backend::Local && self.active_backend == LiveBackend::OpenAi {
            self.error_text = Some(
                "Local transcription is unavailable in this build; using OpenAI Realtime"
                    .to_string(),
            );
        }

        match self.active_backend {
            LiveBackend::OpenAi => self.start_remote_recording(),
            LiveBackend::Local => self.start_local_recording(),
        }
    }

    fn ensure_live_runtime(&mut self) -> Result<&tokio::runtime::Runtime, AppError> {
        if self.live_runtime.is_none() {
            self.live_runtime = Some(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("dictaite-realtime")
                    .build()
                    .map_err(|err| {
                        AppError::Message(format!("Realtime runtime unavailable: {err}"))
                    })?,
            );
        }
        Ok(self
            .live_runtime
            .as_ref()
            .expect("runtime was just created"))
    }

    fn start_remote_recording(&mut self) {
        let Some(client) = self.openai.clone() else {
            self.error_text = Some("OpenAI client unavailable".to_string());
            self.live_state = LiveState::Error;
            return;
        };
        if let Err(err) = self.ensure_live_runtime() {
            self.error_text = Some(err.to_string());
            self.live_state = LiveState::Error;
            return;
        }

        let translate = effective_translate(&self.settings, self.translate_enabled)
            && self.target_language_index > 0;
        let source_language = if self.origin_language_index == 0 {
            None
        } else {
            Some(LANGUAGES[self.origin_language_index].code.to_string())
        };
        let target_language = if translate {
            Some(LANGUAGES[self.target_language_index].name.to_string())
        } else {
            None
        };

        let (audio_tx, audio_rx) = tokio::sync::mpsc::channel(32);
        let (rt_event_tx, mut rt_event_rx) = tokio::sync::mpsc::channel(128);
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        let ui_tx = self.live_event_tx.clone();
        self.live_runtime.as_ref().unwrap().spawn(async move {
            while let Some(event) = rt_event_rx.recv().await {
                let _ = ui_tx.send(event);
            }
        });

        let config = RealtimeSessionConfig {
            api_key: client.api_key().to_string(),
            source_language,
            target_language,
        };
        if translate {
            let failure_tx = rt_event_tx.clone();
            self.live_runtime.as_ref().unwrap().spawn(async move {
                if let Err(err) = run_live_translation(config, audio_rx, rt_event_tx, stop_rx).await
                {
                    let _ = failure_tx
                        .send(RealtimeEvent::Error {
                            message: err.to_string(),
                        })
                        .await;
                }
            });
        } else {
            let failure_tx = rt_event_tx.clone();
            self.live_runtime.as_ref().unwrap().spawn(async move {
                if let Err(err) =
                    run_live_transcription(config, audio_rx, rt_event_tx, stop_rx).await
                {
                    let _ = failure_tx
                        .send(RealtimeEvent::Error {
                            message: err.to_string(),
                        })
                        .await;
                }
            });
        }

        match LiveCapture::start(AudioSpec::openai(), audio_tx, self.live_event_tx.clone()) {
            Ok(capture) => {
                self.live_capture = Some(capture);
                self.live_stop_tx = Some(stop_tx);
                self.is_recording = true;
                self.record_started_at = Some(Instant::now());
                self.live_state = LiveState::connected(translate);
                self.status_text = if translate {
                    format!(
                        "Translating live to {}",
                        LANGUAGES[self.target_language_index].name
                    )
                } else {
                    "Listening live...".to_string()
                };
            }
            Err(err) => {
                let _ = stop_tx.send(());
                self.live_state = LiveState::Error;
                self.error_text = Some(err.to_string());
                self.status_text = "Press to start listening".to_string();
            }
        }
    }

    fn start_local_recording(&mut self) {
        let source_language = if self.origin_language_index == 0 {
            None
        } else {
            Some(LANGUAGES[self.origin_language_index].code.to_string())
        };
        let mut language_hint = source_language;
        let device_warning = match self.settings.local.device.as_str() {
            "metal" if !cfg!(feature = "metal") => {
                Some("Metal is unavailable in this build; using CPU".to_string())
            }
            "cuda" if !cfg!(feature = "cuda") => {
                Some("CUDA is unavailable in this build; using CPU".to_string())
            }
            _ => None,
        };
        if let Some(warning) = device_warning {
            self.error_text = Some(warning);
        }
        if self.settings.local.model.ends_with(".en")
            && language_hint.as_deref().is_some_and(|lang| lang != "en")
        {
            self.error_text = Some(format!(
                "Model {} is English-only; using English instead",
                self.settings.local.model
            ));
            language_hint = Some("en".to_string());
        }

        let engine = match self.create_local_engine() {
            Ok(engine) => engine,
            Err(err) => {
                self.error_text = Some(err.to_string());
                self.live_state = LiveState::Error;
                return;
            }
        };
        let spec = engine.audio_spec();
        let model_path = resolve_model_path(&self.settings.local);
        if !model_path.is_file() && !cfg!(feature = "test-local-engine") {
            self.error_text = Some(format!(
                "Local model '{}' is not available at {}. Open Settings to download or choose it.",
                self.settings.local.model,
                model_path.display()
            ));
            self.live_state = LiveState::Error;
            return;
        }
        let config = LocalSessionConfig {
            language_hint,
            model_path,
            device: self.settings.local.device.clone(),
        };
        let (audio_tx, audio_rx) = tokio::sync::mpsc::channel(32);
        let mut session = start_local_session(engine, config, audio_rx, self.live_event_tx.clone());

        match LiveCapture::start(spec, audio_tx, self.live_event_tx.clone()) {
            Ok(capture) => {
                self.live_capture = Some(capture);
                self.local_session = Some(session);
                self.is_recording = true;
                self.record_started_at = Some(Instant::now());
                self.live_state = LiveState::Transcribing;
                self.status_text = "Loading local model...".to_string();
            }
            Err(err) => {
                session.stop();
                self.live_state = LiveState::Error;
                self.error_text = Some(err.to_string());
                self.status_text = "Press to start listening".to_string();
            }
        }
    }

    fn create_local_engine(&self) -> Result<Box<dyn LocalEngine>, AppError> {
        if self.settings.local.engine != "whisper" {
            return Err(AppError::Message(format!(
                "Unsupported local engine '{}'",
                self.settings.local.engine
            )));
        }
        #[cfg(feature = "local-whisper")]
        {
            return Ok(Box::new(crate::local::whisper::WhisperEngine::new()));
        }
        #[cfg(all(not(feature = "local-whisper"), feature = "test-local-engine"))]
        {
            return Ok(Box::new(crate::local::fake::FakeLocalEngine::new(
                Vec::new(),
                Vec::new(),
            )));
        }
        #[allow(unreachable_code)]
        Err(AppError::Message(
            "Local transcription support is not compiled into this build".to_string(),
        ))
    }

    fn stop_recording(&mut self) {
        self.is_recording = false;
        self.record_started_at = None;
        if let Some(stop_tx) = self.live_stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(mut session) = self.local_session.take() {
            session.stop();
        }
        if let Some(mut capture) = self.live_capture.take() {
            capture.stop();
        }
        self.live_state = self.live_state.stop();
        self.status_text = "Stopped".to_string();
    }

    fn show_record_controls(&mut self, ui: &mut Ui, ctx: &Context) {
        let available_width = ui.available_width();
        let frame_margin = egui::Margin::same(12);
        Frame::group(ui.style())
            .inner_margin(frame_margin)
            .corner_radius(egui::CornerRadius::same(8))
            .show(ui, |ui| {
                let content_width =
                    (available_width - f32::from(frame_margin.left + frame_margin.right)).max(0.0);
                ui.set_width(content_width);
                ui.add_space(8.0);

                let button_label = if self.is_recording {
                    "Stop Listening"
                } else {
                    "Start Listening"
                };
                if ui
                    .add_sized(
                        Vec2::new(content_width, 42.0),
                        egui::Button::new(RichText::new(button_label).size(18.0).strong()),
                    )
                    .clicked()
                {
                    if self.is_recording {
                        self.stop_recording();
                    } else {
                        self.start_recording();
                    }
                    ctx.request_repaint();
                }

                ui.add_space(10.0);
                ui.label(RichText::new(&self.status_text).heading().size(16.0));
                if self.is_recording {
                    let elapsed = self
                        .record_started_at
                        .map(|instant| instant.elapsed())
                        .unwrap_or_default();
                    ui.label(RichText::new(time_display(elapsed)).monospace());
                } else if let Some(player) = &self.player {
                    if player.is_playing() {
                        let elapsed = player.elapsed();
                        let duration = player.duration();
                        ui.label(
                            RichText::new(format!(
                                "{} / {}",
                                time_display(elapsed),
                                time_display(duration)
                            ))
                            .monospace(),
                        );
                        ctx.request_repaint();
                    }
                }
            });
    }

    fn poll_live_events(&mut self, ctx: &Context) {
        while let Ok(event) = self.live_event_rx.try_recv() {
            match event {
                RealtimeEvent::SourceDelta { item_id, text } => {
                    self.source_assembler.add_delta(item_id.as_deref(), &text);
                    self.source_transcript = self.source_assembler.text();
                    self.transcript = self.source_transcript.clone();
                    self.raw_transcript = Some(self.source_transcript.clone());
                }
                RealtimeEvent::SourceCompleted { item_id, text } => {
                    self.source_assembler.complete(item_id.as_deref(), &text);
                    self.source_transcript = self.source_assembler.text();
                    self.transcript = self.source_transcript.clone();
                    self.raw_transcript = Some(self.source_transcript.clone());
                }
                RealtimeEvent::SourceUpdate { item_id, text } => {
                    self.source_assembler.update(item_id.as_deref(), &text);
                    self.source_transcript = self.source_assembler.text();
                    self.transcript = self.source_transcript.clone();
                    self.raw_transcript = Some(self.source_transcript.clone());
                }
                RealtimeEvent::TranslationDelta { text } => {
                    self.translated_transcript.push_str(&text);
                    self.transcript = self.translated_transcript.clone();
                }
                RealtimeEvent::TranslatedAudioDelta => {}
                RealtimeEvent::SessionState { state } => {
                    self.status_text = live_state_text(&state);
                    if state == "disconnected" {
                        self.live_state = LiveState::Disconnected;
                        self.is_recording = false;
                        self.record_started_at = None;
                        if let Some(mut capture) = self.live_capture.take() {
                            capture.stop();
                        }
                        self.live_stop_tx = None;
                    }
                }
                RealtimeEvent::Error { message } => {
                    self.error_text = Some(message);
                    if self.active_backend == LiveBackend::Local {
                        ctx.request_repaint();
                        continue;
                    }
                    self.live_state = LiveState::Error;
                    self.status_text = "Live session error".to_string();
                    self.is_recording = false;
                    self.record_started_at = None;
                    if let Some(mut capture) = self.live_capture.take() {
                        capture.stop();
                    }
                    self.live_stop_tx = None;
                }
                RealtimeEvent::Unknown { .. } => {}
            }
            ctx.request_repaint();
        }

        if let Some(capture) = &self.live_capture {
            if let Some(err) = capture.take_error() {
                self.error_text = Some(format!("Microphone error: {err}"));
                self.live_state = LiveState::Error;
                ctx.request_repaint();
            }
        }
    }

    fn transcript_for_actions(&self) -> String {
        if effective_translate(&self.settings, self.translate_enabled)
            && !self.translated_transcript.trim().is_empty()
        {
            let mut parts = Vec::new();
            if !self.source_transcript.trim().is_empty() {
                parts.push(format!("Source:\n{}", self.source_transcript.trim()));
            }
            parts.push(format!(
                "Translation:\n{}",
                self.translated_transcript.trim()
            ));
            parts.join("\n\n")
        } else if !self.source_transcript.trim().is_empty() {
            self.source_transcript.clone()
        } else {
            self.transcript.clone()
        }
    }

    fn request_tts(&mut self, intent: TtsIntent, text: String) {
        let Some(client) = self.openai.clone() else {
            self.error_text = Some("OpenAI client unavailable".to_string());
            return;
        };
        self.status_text = "Generating speech...".to_string();
        let voice_id = match &intent {
            TtsIntent::Transcript { voice_id, .. } => voice_id.clone(),
            TtsIntent::Preview { voice_id, .. } => voice_id.clone(),
        };
        self.tts_task = Some(BackgroundTask::spawn(move || {
            let audio = client.text_to_speech(&text, &voice_id)?;
            let clip = AudioClip::from_wav_bytes(audio)?;
            Ok(TtsOutcome { clip, intent })
        }));
    }

    fn poll_tts(&mut self, ctx: &Context) {
        if let Some(task) = &mut self.tts_task {
            if let Some(result) = task.try_take() {
                self.tts_task = None;
                match result {
                    Ok(outcome) => {
                        self.error_text = None;
                        if let Some(player) = self.player.as_mut() {
                            let clip = outcome.clip;
                            let status = match outcome.intent {
                                TtsIntent::Transcript {
                                    voice_id,
                                    voice_label,
                                } => {
                                    self.tts_voice_id = Some(voice_id.clone());
                                    self.tts_clip = Some(clip.clone());
                                    format!("Playing transcript ({voice_label})")
                                }
                                TtsIntent::Preview { voice_label, .. } => {
                                    format!("Previewing {voice_label}")
                                }
                            };
                            if let Err(err) = player.play(clip) {
                                self.error_text = Some(err.to_string());
                            } else {
                                self.status_text = status;
                            }
                        } else {
                            self.error_text = Some("Audio output unavailable".to_string());
                        }
                    }
                    Err(err) => {
                        self.error_text = Some(err.to_string());
                        self.status_text = "Speech synthesis failed".to_string();
                    }
                }
            } else {
                ctx.request_repaint();
            }
        }
    }

    fn poll_model_download(&mut self, ctx: &Context) {
        let result = self
            .model_download
            .as_ref()
            .and_then(ModelDownload::ready)
            .cloned();
        if let Some(result) = result {
            self.model_download = None;
            self.model_download_entry = None;
            match result {
                Ok(path) => {
                    self.status_text = format!("Model downloaded to {}", path.display());
                    self.error_text = None;
                }
                Err(err) => self.error_text = Some(err),
            }
            ctx.request_repaint();
        } else if self.model_download.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn copy_transcript(&mut self) {
        let text = self.transcript_for_actions();
        if text.trim().is_empty() {
            return;
        }
        match Clipboard::new() {
            Ok(mut clipboard) => {
                if clipboard.set_text(text).is_ok() {
                    self.copy_feedback_until = Some(Instant::now() + Duration::from_secs(2));
                    self.status_text = "Copied transcript".to_string();
                }
            }
            Err(err) => {
                self.error_text = Some(format!("Clipboard error: {err}"));
            }
        }
    }

    fn save_transcript(&mut self) {
        let text = self.transcript_for_actions();
        if text.trim().is_empty() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Save Transcript")
            .set_file_name("transcript.txt")
            .save_file()
        {
            if let Err(err) = fs::write(&path, text.as_bytes()) {
                self.error_text = Some(format!("Failed to save file: {err}"));
            } else {
                self.status_text = format!("Transcript saved to {}", path.display());
                self.error_text = None;
            }
        }
    }

    fn play_transcript_audio(&mut self) {
        let text = if effective_translate(&self.settings, self.translate_enabled)
            && !self.translated_transcript.trim().is_empty()
        {
            self.translated_transcript.trim()
        } else if !self.source_transcript.trim().is_empty() {
            self.source_transcript.trim()
        } else {
            self.transcript.trim()
        };
        if text.is_empty() {
            self.error_text = Some("Transcript is empty".to_string());
            return;
        }
        let voice_id = match self.preferred_gender {
            VoiceGender::Female => self.settings.female_voice.clone(),
            VoiceGender::Male => self.settings.male_voice.clone(),
        };
        let voice_label = voice_label_for(&voice_id);
        if let (Some(clip), Some(cached_voice)) =
            (self.tts_clip.clone(), self.tts_voice_id.as_ref())
        {
            if !clip.samples().is_empty() && cached_voice.eq_ignore_ascii_case(&voice_id) {
                if let Some(player) = self.player.as_mut() {
                    if let Err(err) = player.play(clip) {
                        self.error_text = Some(err.to_string());
                    } else {
                        self.status_text = format!("Playing transcript ({voice_label})");
                    }
                    return;
                }
            }
        }
        self.tts_voice_id = None;
        self.request_tts(
            TtsIntent::Transcript {
                voice_id: voice_id.clone(),
                voice_label,
            },
            text.to_string(),
        );
    }

    fn preview_voice(&mut self, voice_id: &str) {
        let label = voice_label_for(voice_id);
        self.request_tts(
            TtsIntent::Preview {
                voice_id: voice_id.to_string(),
                voice_label: label,
            },
            VOICE_SAMPLE_TEXT.to_string(),
        );
    }

    fn update_copy_feedback(&mut self, ui: &mut Ui) {
        if let Some(deadline) = self.copy_feedback_until {
            if Instant::now() < deadline {
                ui.label(RichText::new("Copied to clipboard").color(Color32::from_rgb(0, 150, 0)));
            } else {
                self.copy_feedback_until = None;
            }
        }
    }
}

impl App for DictaiteApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        self.poll_live_events(&ctx);
        self.poll_tts(&ctx);
        self.poll_model_download(&ctx);
        if let Some(player) = &mut self.player {
            player.refresh();
        }

        egui::Panel::top("topbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("dict-ai-te").heading());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Settings").clicked() {
                        self.settings_modal = Some(SettingsModal::from(&self.settings));
                    }
                });
            });
        });

        // Bottom controls bar anchored to the window bottom
        egui::Panel::bottom("controls_bar").show_inside(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("⬇ Save").clicked() {
                    self.save_transcript();
                }
                if ui.button("⧉ Copy").clicked() {
                    self.copy_transcript();
                }
                let mut play_label = "▶ Play";
                if let Some(player) = &self.player {
                    if player.is_playing() {
                        play_label = "■ Stop";
                    }
                }
                let tts_available =
                    self.settings.backend != Backend::Local || self.openai.is_some();
                let play_response = ui
                    .add_enabled(tts_available, egui::Button::new(play_label))
                    .on_disabled_hover_text("Text-to-speech requires an OpenAI API key");
                if play_response.clicked() {
                    if let Some(player) = &mut self.player {
                        if player.is_playing() {
                            player.stop();
                        } else {
                            self.play_transcript_audio();
                        }
                    } else {
                        self.error_text = Some("Audio output unavailable".to_string());
                    }
                }

                ui.separator();
                ui.radio_value(&mut self.preferred_gender, VoiceGender::Female, "Female");
                ui.radio_value(&mut self.preferred_gender, VoiceGender::Male, "Male");
            });

            ui.add_space(6.0);
            if let Some(err) = &self.error_text {
                ui.colored_label(Color32::from_rgb(200, 60, 60), err);
            } else if let Some(msg) = &self.player_error {
                ui.colored_label(Color32::from_rgb(200, 60, 60), msg);
            }
            self.update_copy_feedback(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // let top_button_label = if self.is_recording {
            //     "Stop Listening"
            // } else {
            //     "Start Listening"
            // };
            // let full_width = ui.available_width();
            // if ui
            //     .add_sized(
            //         Vec2::new(full_width, 32.0),
            //         egui::Button::new(top_button_label),
            //     )
            //     .clicked()
            // {
            //     if self.is_recording {
            //         self.stop_recording();
            //     } else {
            //         self.start_recording();
            //     }
            //     ctx.request_repaint();
            // }

            ui.add_space(6.0);
            let level = if self.is_recording {
                self.live_capture
                    .as_ref()
                    .map(LiveCapture::current_level)
                    .unwrap_or(0.0)
            } else if let Some(player) = &self.player {
                if player.is_playing() {
                    player.level()
                } else {
                    0.0
                }
            } else {
                0.0
            };
            ui.add(egui::widgets::ProgressBar::new(level).desired_width(ui.available_width()));

            ui.add_space(8.0);
            self.show_record_controls(ui, &ctx);

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Origin language");
                ui.separator();
            });
            egui::ComboBox::from_id_salt("origin_lang")
                .selected_text(LANGUAGES[self.origin_language_index].name)
                .show_ui(ui, |ui| {
                    for (idx, lang) in LANGUAGES.iter().enumerate() {
                        if ui
                            .selectable_value(&mut self.origin_language_index, idx, lang.name)
                            .clicked()
                        {
                            // nothing else for now
                        }
                    }
                });

            ui.add_space(8.0);
            let local_mode = effective_live_backend(&self.settings) == LiveBackend::Local;
            ui.horizontal(|ui| {
                ui.label("Translate Live");
                let mut flag = self.translate_enabled;
                if ui.add_enabled(!local_mode, egui::Checkbox::new(&mut flag, "")).changed() {
                    self.translate_enabled = flag;
                    if !flag {
                        if let Some(original) = &self.raw_transcript {
                            self.transcript = original.clone();
                        }
                    }
                }
            });

            if local_mode {
                ui.small(
                    "Local mode supports transcription only. Translation requires the OpenAI Realtime API.",
                );
            } else if self.translate_enabled {
                ui.horizontal(|ui| {
                    ui.label("Target language");
                    egui::ComboBox::from_id_salt("target_lang")
                        .selected_text(LANGUAGES[self.target_language_index].name)
                        .show_ui(ui, |ui| {
                            for (idx, lang) in LANGUAGES.iter().enumerate() {
                                if idx == 0 {
                                    continue;
                                }
                                ui.selectable_value(
                                    &mut self.target_language_index,
                                    idx,
                                    lang.name,
                                );
                            }
                        });
                });
            }

            ui.add_space(10.0);
            let width = ui.available_width();
            let height = ui.available_height();
            if effective_translate(&self.settings, self.translate_enabled) {
                let pane_height = (height - 32.0).max(120.0) / 2.0;
                ui.label("Source transcript");
                let source_response = ui.add_sized(
                    Vec2::new(width, pane_height),
                    egui::TextEdit::multiline(&mut self.source_transcript)
                        .hint_text("Source speech will appear here..."),
                );
                if source_response.changed() {
                    self.transcript = self.source_transcript.clone();
                    self.raw_transcript = Some(self.source_transcript.clone());
                }
                ui.add_space(8.0);
                ui.label("Translated transcript");
                let translated_response = ui.add_sized(
                    Vec2::new(width, pane_height),
                    egui::TextEdit::multiline(&mut self.translated_transcript)
                        .hint_text("Live translation will appear here..."),
                );
                if translated_response.changed() {
                    self.transcript = self.translated_transcript.clone();
                }
            } else {
                let response = ui.add_sized(
                    Vec2::new(width, height),
                    egui::TextEdit::multiline(&mut self.source_transcript)
                        .hint_text("Transcribed text will appear here..."),
                );
                if response.changed() {
                    self.transcript = self.source_transcript.clone();
                    self.raw_transcript = Some(self.source_transcript.clone());
                }
            }
        });

        if let Some(mut modal) = self.settings_modal.take() {
            let mut open = true;
            let mut keep_modal = true;
            egui::Window::new("Settings")
                .collapsible(false)
                .resizable(false)
                .default_size(Vec2::new(380.0, 360.0))
                .open(&mut open)
                .show(&ctx, |ui| {
                    keep_modal = modal.show(ui, self);
                });
            if open && keep_modal {
                self.settings_modal = Some(modal);
            }
        }

        if self.is_recording {
            ctx.request_repaint();
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VoiceGender {
    Female,
    Male,
}

enum TtsIntent {
    Transcript {
        voice_id: String,
        voice_label: String,
    },
    Preview {
        voice_id: String,
        voice_label: String,
    },
}

struct TtsOutcome {
    clip: AudioClip,
    intent: TtsIntent,
}

struct BackgroundTask<T> {
    receiver: Option<mpsc::Receiver<Result<T, AppError>>>,
}

impl<T: Send + 'static> BackgroundTask<T> {
    fn spawn<F>(task: F) -> Self
    where
        F: FnOnce() -> Result<T, AppError> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = task();
            let _ = tx.send(result);
        });
        Self { receiver: Some(rx) }
    }

    fn try_take(&mut self) -> Option<Result<T, AppError>> {
        let rx = self.receiver.as_ref()?;
        match rx.try_recv() {
            Ok(result) => {
                self.receiver = None;
                Some(result)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver = None;
                Some(Err(AppError::Message(
                    "Background task channel disconnected".to_string(),
                )))
            }
        }
    }
}

struct SettingsModal {
    backend: Backend,
    language_index: usize,
    translate_default: bool,
    target_index: usize,
    female_voice_index: usize,
    male_voice_index: usize,
    local_model: String,
    local_model_path: Option<std::path::PathBuf>,
    local_device: String,
}

impl SettingsModal {
    fn from(settings: &Settings) -> Self {
        Self {
            backend: settings.backend,
            language_index: language_index(settings.default_language.as_deref()),
            translate_default: settings.translate_by_default,
            target_index: language_index(settings.default_target_language.as_deref()).max(1),
            female_voice_index: voice_index(FEMALE_VOICES, &settings.female_voice),
            male_voice_index: voice_index(MALE_VOICES, &settings.male_voice),
            local_model: settings.local.model.clone(),
            local_model_path: settings.local.model_path.clone(),
            local_device: settings.local.device.clone(),
        }
    }

    fn show(&mut self, ui: &mut Ui, app: &mut DictaiteApp) -> bool {
        ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
        let mut keep_open = true;

        ui.vertical(|ui| {
            ui.label("Transcription backend");
            ui.radio_value(&mut self.backend, Backend::Openai, "OpenAI Realtime");
            let local_response = ui.add_enabled(
                LOCAL_BACKEND_AVAILABLE,
                egui::RadioButton::new(self.backend == Backend::Local, "Local model"),
            );
            if local_response.clicked() {
                self.backend = Backend::Local;
            }
            if !LOCAL_BACKEND_AVAILABLE {
                local_response.on_disabled_hover_text(
                    "Local support is unavailable in this build (enable local-whisper)",
                );
                ui.small("Local support is unavailable in this build.");
            }
            let mut staged_settings = app.settings.clone();
            staged_settings.backend = self.backend;
            let translation_available = effective_translate(&staged_settings, true);

            ui.separator();
            ui.label("Default language");
            egui::ComboBox::from_id_salt("settings_default_language")
                .selected_text(LANGUAGES[self.language_index].name)
                .show_ui(ui, |ui| {
                    for (idx, lang) in LANGUAGES.iter().enumerate() {
                        ui.selectable_value(&mut self.language_index, idx, lang.name);
                    }
                });

            ui.horizontal(|ui| {
                ui.label("Translate by default");
                ui.add_enabled(
                    translation_available,
                    egui::Checkbox::new(&mut self.translate_default, ""),
                );
            });

            if self.backend == Backend::Local {
                ui.small(
                    "Local mode supports transcription only. Translation requires the OpenAI Realtime API.",
                );
            } else {
                ui.horizontal(|ui| {
                ui.label("Default target language");
                egui::ComboBox::from_id_salt("settings_target_language")
                    .selected_text(LANGUAGES[self.target_index].name)
                    .show_ui(ui, |ui| {
                        for (idx, lang) in LANGUAGES.iter().enumerate() {
                            if idx == 0 {
                                continue;
                            }
                            ui.selectable_value(&mut self.target_index, idx, lang.name);
                        }
                    });
                });
            }

            if self.backend == Backend::Local {
                ui.separator();
                ui.label("Local model");
                ui.horizontal(|ui| {
                    ui.label("Engine");
                    ui.add_enabled(false, egui::Label::new("Whisper"));
                });
                let selected_model_label = manifest_entry(&self.local_model)
                    .map(|entry| entry.display_name)
                    .unwrap_or(&self.local_model);
                egui::ComboBox::from_id_salt("settings_local_model")
                    .selected_text(selected_model_label)
                    .show_ui(ui, |ui| {
                        for entry in MODEL_MANIFEST {
                            ui.selectable_value(
                                &mut self.local_model,
                                entry.id.to_string(),
                                entry.display_name,
                            );
                        }
                    });
                egui::ComboBox::from_id_salt("settings_local_device")
                    .selected_text(&self.local_device)
                    .show_ui(ui, |ui| {
                        for device in ["auto", "cpu", "metal", "cuda"] {
                            ui.selectable_value(
                                &mut self.local_device,
                                device.to_string(),
                                device,
                            );
                        }
                    });
                if ui.button("Choose model file...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose Whisper model")
                        .pick_file()
                    {
                        self.local_model_path = Some(path);
                    }
                }
                let local_settings = crate::settings::LocalSettings {
                    engine: "whisper".to_string(),
                    model: self.local_model.clone(),
                    model_path: self.local_model_path.clone(),
                    device: self.local_device.clone(),
                };
                if let (Some(download), Some(entry)) =
                    (&app.model_download, app.model_download_entry)
                {
                    let (bytes, fraction) = download.progress(entry.size);
                    ui.add(egui::ProgressBar::new(fraction).text(format!(
                        "{} / {} MB",
                        bytes / 1_000_000,
                        entry.size / 1_000_000
                    )));
                    if ui.button("Cancel download").clicked() {
                        download.cancel();
                    }
                } else {
                    match model_availability(&local_settings) {
                        ModelAvailability::Ready(path) => {
                            ui.small(format!("Model ready: {}", path.display()));
                        }
                        ModelAvailability::MissingConfiguredPath(path) => {
                            ui.small(format!("Missing at configured path: {}", path.display()));
                        }
                        ModelAvailability::NotDownloaded(_) => {
                            ui.small(format!("Model '{}' is not downloaded", self.local_model));
                            if let Some(entry) = manifest_entry(&self.local_model).copied() {
                                if ui.button("Download").clicked() {
                                    let destination = model_path("whisper", entry.id);
                                    app.model_download =
                                        Some(ModelDownload::start(entry, destination));
                                    app.model_download_entry = Some(entry);
                                }
                            }
                        }
                        ModelAvailability::Downloading { .. } => {}
                    }
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Female voice");
                egui::ComboBox::from_id_salt("settings_female_voice")
                    .selected_text(FEMALE_VOICES[self.female_voice_index].label)
                    .show_ui(ui, |ui| {
                        for (idx, voice) in FEMALE_VOICES.iter().enumerate() {
                            ui.selectable_value(&mut self.female_voice_index, idx, voice.label);
                        }
                    });
                if ui.button("Play").clicked() {
                    let voice_id = FEMALE_VOICES[self.female_voice_index].id;
                    app.preview_voice(voice_id);
                }
            });

            ui.horizontal(|ui| {
                ui.label("Male voice");
                egui::ComboBox::from_id_salt("settings_male_voice")
                    .selected_text(MALE_VOICES[self.male_voice_index].label)
                    .show_ui(ui, |ui| {
                        for (idx, voice) in MALE_VOICES.iter().enumerate() {
                            ui.selectable_value(&mut self.male_voice_index, idx, voice.label);
                        }
                    });
                if ui.button("Play").clicked() {
                    let voice_id = MALE_VOICES[self.male_voice_index].id;
                    app.preview_voice(voice_id);
                }
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Save").clicked() {
                    self.persist(app);
                    keep_open = false;
                }
                if ui.button("Cancel").clicked() {
                    keep_open = false;
                }
            });
        });
        keep_open
    }

    fn persist(&self, app: &mut DictaiteApp) {
        let backend_changed = app.settings.backend != self.backend;
        if backend_changed && app.is_recording {
            app.stop_recording();
        }
        let mut settings = app.settings.clone();
        settings.backend = self.backend;
        settings.default_language = if self.language_index == 0 {
            None
        } else {
            Some(LANGUAGES[self.language_index].code.to_string())
        };
        settings.translate_by_default = self.translate_default;
        settings.default_target_language = if self.target_index == 0 {
            None
        } else {
            Some(LANGUAGES[self.target_index].code.to_string())
        };
        settings.female_voice = FEMALE_VOICES[self.female_voice_index].id.to_string();
        settings.male_voice = MALE_VOICES[self.male_voice_index].id.to_string();
        settings.local.engine = "whisper".to_string();
        settings.local.model = self.local_model.clone();
        settings.local.model_path = self.local_model_path.clone();
        settings.local.device = self.local_device.clone();

        if let Err(err) = save_settings(&settings) {
            app.error_text = Some(err.to_string());
        } else {
            app.error_text = None;
        }
        app.settings = settings;
        app.apply_settings_defaults();
    }
}

fn time_display(duration: Duration) -> String {
    let secs = duration.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

fn live_state_text(state: &str) -> String {
    match state {
        "session.created" | "session.updated" | "connecting" => {
            "Connected to live session".to_string()
        }
        "audio.capture.stopped" => "Audio capture stopped".to_string(),
        "local.loading" => "Loading local model...".to_string(),
        "local.listening" => "Listening locally...".to_string(),
        "local.processing" => "Transcribing locally...".to_string(),
        "disconnected" => "Disconnected".to_string(),
        other => format!("Live state: {other}"),
    }
}

fn language_index(code: Option<&str>) -> usize {
    if let Some(code) = code {
        let lower = code.trim().to_ascii_lowercase();
        for (idx, lang) in LANGUAGES.iter().enumerate() {
            if lang.code.eq_ignore_ascii_case(&lower) {
                return idx;
            }
        }
    }
    0
}

fn voice_index(list: &[crate::constants::VoiceOption], value: &str) -> usize {
    let value = value.trim().to_ascii_lowercase();
    list.iter()
        .position(|voice| voice.id.eq_ignore_ascii_case(&value))
        .unwrap_or(0)
}

fn voice_label_for(voice_id: &str) -> String {
    let id = voice_id.trim().to_ascii_lowercase();
    FEMALE_VOICES
        .iter()
        .chain(MALE_VOICES.iter())
        .find(|voice| voice.id.eq_ignore_ascii_case(&id))
        .map(|voice| voice.label.to_string())
        .unwrap_or_else(|| voice_id.to_string())
}
