use std::fs;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use eframe::App;
use egui::{self, Align, Color32, Context, Frame, Layout, RichText, ScrollArea, Ui, Vec2};

use crate::audio::{AudioClip, AudioPlayer, Recorder};
use crate::constants::{FEMALE_VOICES, LANGUAGES, MALE_VOICES, VOICE_SAMPLE_TEXT};
use crate::error::AppError;
use crate::openai::OpenAiClient;
use crate::settings::{load_settings, save_settings, Settings};

pub struct DictaiteApp {
    recorder: Recorder,
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

    preferred_gender: VoiceGender,

    recorded_clip: Option<AudioClip>,
    tts_clip: Option<AudioClip>,
    tts_voice_id: Option<String>,

    transcription_task: Option<BackgroundTask<TranscriptionOutcome>>,
    tts_task: Option<BackgroundTask<TtsOutcome>>,

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

        let mut app = Self {
            recorder: Recorder::new(),
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
            preferred_gender: VoiceGender::Female,
            recorded_clip: None,
            tts_clip: None,
            tts_voice_id: None,
            transcription_task: None,
            tts_task: None,
            status_text: "Press to start recording".to_string(),
            error_text: None,
            copy_feedback_until: None,
        };
        app.apply_settings_defaults();
        app.maybe_warn_api_key();
        app
    }

    fn apply_settings_defaults(&mut self) {
        self.origin_language_index = language_index(self.settings.default_language.as_deref());
        self.translate_enabled = self.settings.translate_by_default;
        let target_idx = language_index(self.settings.default_target_language.as_deref()).max(1);
        self.target_language_index = target_idx;
    }

    fn maybe_warn_api_key(&mut self) {
        if self.openai.is_none() {
            self.error_text = Some("OPENAI_API_KEY not configured".to_string());
        }
    }

    fn start_recording(&mut self) {
        if self.transcription_task.is_some() {
            self.transcription_task = None;
        }
        self.tts_task = None;
        self.is_recording = true;
        self.record_started_at = Some(Instant::now());
        self.status_text = "Recording...".to_string();
        self.error_text = None;
        match self.recorder.start() {
            Ok(()) => {
                self.recorded_clip = None;
                self.tts_clip = None;
                self.tts_voice_id = None;
                self.raw_transcript = None;
                self.transcript.clear();
            }
            Err(err) => {
                self.is_recording = false;
                self.record_started_at = None;
                self.status_text = "Press to start recording".to_string();
                self.error_text = Some(err.to_string());
            }
        }
    }

    fn stop_recording(&mut self) {
        self.is_recording = false;
        self.record_started_at = None;
        match self.recorder.stop() {
            Ok(Some(clip)) => {
                self.status_text = "Transcribing...".to_string();
                self.begin_transcription(clip);
            }
            Ok(None) => {
                self.status_text = "No audio captured".to_string();
            }
            Err(err) => {
                self.error_text = Some(err.to_string());
            }
        }
    }

    fn show_record_controls(&mut self, ui: &mut Ui, ctx: &Context) {
        let available_width = ui.available_width();
        let frame_margin = egui::Margin::same(12.0);
        Frame::group(ui.style())
            .inner_margin(frame_margin)
            .rounding(egui::Rounding::same(8.0))
            .show(ui, |ui| {
                let content_width =
                    (available_width - frame_margin.left - frame_margin.right).max(0.0);
                ui.set_width(content_width);
                ui.add_space(8.0);

                let button_label = if self.is_recording {
                    "Stop Recording"
                } else {
                    "Start Recording"
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
                        .unwrap_or_else(|| self.recorder.elapsed());
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

    fn begin_transcription(&mut self, mut clip: AudioClip) {
        let Some(client) = self.openai.clone() else {
            self.error_text = Some("OpenAI client unavailable".to_string());
            return;
        };
        let Ok(wav_arc) = clip.wav_bytes() else {
            self.error_text = Some("Failed to prepare audio clip".to_string());
            return;
        };
        let wav_bytes = (*wav_arc).clone();
        let translate = self.translate_enabled && self.target_language_index > 0;
        let language_code = if self.origin_language_index == 0 {
            None
        } else {
            Some(LANGUAGES[self.origin_language_index].code.to_string())
        };
        let target_language = if translate {
            Some(LANGUAGES[self.target_language_index].name.to_string())
        } else {
            None
        };

        self.recorded_clip = Some(clip);
        self.tts_clip = None;
        self.tts_voice_id = None;
        self.transcription_task = Some(BackgroundTask::spawn(move || {
            let transcript = client.transcribe(&wav_bytes, language_code.as_deref())?;
            let mut translated = None;
            let mut translation_error = None;
            let mut translation_lang = None;
            if let Some(target) = target_language {
                match client.translate(&transcript, &target) {
                    Ok(result) => {
                        translated = Some(result);
                        translation_lang = Some(target);
                    }
                    Err(err) => {
                        translation_error = Some(err.to_string());
                    }
                }
            }
            Ok(TranscriptionOutcome {
                transcript,
                translated,
                translation_lang,
                translation_error,
            })
        }));
    }

    fn poll_transcription(&mut self, ctx: &Context) {
        if let Some(task) = &mut self.transcription_task {
            if let Some(result) = task.try_take() {
                self.transcription_task = None;
                match result {
                    Ok(outcome) => {
                        self.raw_transcript = Some(outcome.transcript.clone());
                        if let Some(text) = outcome.translated {
                            self.transcript = text;
                            if let Some(lang) = outcome.translation_lang {
                                self.status_text = format!("Translated to {lang}");
                            } else {
                                self.status_text = "Translation complete".to_string();
                            }
                        } else {
                            self.transcript = outcome.transcript;
                            self.status_text = "Transcription complete".to_string();
                        }
                        if let Some(err) = outcome.translation_error {
                            self.error_text = Some(format!("Translation failed: {err}"));
                        } else {
                            self.error_text = None;
                        }
                    }
                    Err(err) => {
                        self.error_text = Some(err.to_string());
                        self.status_text = "Transcription failed".to_string();
                    }
                }
            } else {
                ctx.request_repaint();
            }
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
            let clip = AudioClip::from_wav_bytes(audio).map_err(AppError::from)?;
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

    fn copy_transcript(&mut self) {
        if self.transcript.trim().is_empty() {
            return;
        }
        match Clipboard::new() {
            Ok(mut clipboard) => {
                if clipboard.set_text(self.transcript.clone()).is_ok() {
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
        if self.transcript.trim().is_empty() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Save Transcript")
            .set_file_name("transcript.txt")
            .save_file()
        {
            if let Err(err) = fs::write(&path, self.transcript.as_bytes()) {
                self.error_text = Some(format!("Failed to save file: {err}"));
            } else {
                self.status_text = format!("Transcript saved to {}", path.display());
                self.error_text = None;
            }
        }
    }

    fn play_transcript_audio(&mut self) {
        let text = self.transcript.trim();
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
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_transcription(ctx);
        self.poll_tts(ctx);
        if let Some(player) = &mut self.player {
            player.refresh();
        }

        egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("dict-ai-te").heading());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Settings").clicked() {
                        self.settings_modal = Some(SettingsModal::from(&self.settings));
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // let top_button_label = if self.is_recording {
            //     "Stop Recording"
            // } else {
            //     "Start Recording"
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
                self.recorder.current_level()
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
            self.show_record_controls(ui, ctx);

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Origin language");
                ui.separator();
            });
            egui::ComboBox::from_id_source("origin_lang")
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
            ui.horizontal(|ui| {
                ui.label("Translate to");
                let mut flag = self.translate_enabled;
                if ui.toggle_value(&mut flag, "").changed() {
                    self.translate_enabled = flag;
                    if !flag {
                        if let Some(original) = &self.raw_transcript {
                            self.transcript = original.clone();
                        }
                    }
                }
            });

            ui.add_enabled_ui(self.translate_enabled, |ui| {
                egui::ComboBox::from_id_source("target_lang")
                    .selected_text(LANGUAGES[self.target_language_index].name)
                    .show_ui(ui, |ui| {
                        for (idx, lang) in LANGUAGES.iter().enumerate() {
                            if idx == 0 {
                                continue;
                            }
                            ui.selectable_value(&mut self.target_language_index, idx, lang.name);
                        }
                    });
            });

            ui.add_space(10.0);
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                let width = ui.available_width();
                ui.set_width(width);
                let response = ui.add_sized(
                    Vec2::new(width, ui.spacing().interact_size.y * 10.0),
                    egui::TextEdit::multiline(&mut self.transcript)
                        .hint_text("Transcribed text will appear here…"),
                );
                if response.changed() {
                    self.raw_transcript = Some(self.transcript.clone());
                }
            });

            ui.add_space(8.0);
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
                if ui.button(play_label).clicked() {
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

        if let Some(mut modal) = self.settings_modal.take() {
            let mut open = true;
            let mut keep_modal = true;
            egui::Window::new("Settings")
                .collapsible(false)
                .resizable(false)
                .default_size(Vec2::new(380.0, 360.0))
                .open(&mut open)
                .show(ctx, |ui| {
                    keep_modal = modal.show(ui, self);
                });
            if open && keep_modal {
                self.settings_modal = Some(modal);
            }
        }

        if self.is_recording || self.transcription_task.is_some() {
            ctx.request_repaint();
        }
    }
}

struct TranscriptionOutcome {
    transcript: String,
    translated: Option<String>,
    translation_lang: Option<String>,
    translation_error: Option<String>,
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
        let Some(rx) = self.receiver.as_ref() else {
            return None;
        };
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
    language_index: usize,
    translate_default: bool,
    target_index: usize,
    female_voice_index: usize,
    male_voice_index: usize,
}

impl SettingsModal {
    fn from(settings: &Settings) -> Self {
        Self {
            language_index: language_index(settings.default_language.as_deref()),
            translate_default: settings.translate_by_default,
            target_index: language_index(settings.default_target_language.as_deref()).max(1),
            female_voice_index: voice_index(FEMALE_VOICES, &settings.female_voice),
            male_voice_index: voice_index(MALE_VOICES, &settings.male_voice),
        }
    }

    fn show(&mut self, ui: &mut Ui, app: &mut DictaiteApp) -> bool {
        ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
        let mut keep_open = true;

        ui.vertical(|ui| {
            ui.label("Default language");
            egui::ComboBox::from_id_source("settings_default_language")
                .selected_text(LANGUAGES[self.language_index].name)
                .show_ui(ui, |ui| {
                    for (idx, lang) in LANGUAGES.iter().enumerate() {
                        ui.selectable_value(&mut self.language_index, idx, lang.name);
                    }
                });

            ui.horizontal(|ui| {
                ui.label("Translate by default");
                ui.toggle_value(&mut self.translate_default, "");
            });

            ui.add_enabled_ui(self.translate_default, |ui| {
                egui::ComboBox::from_id_source("settings_target_language")
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

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Female voice");
                egui::ComboBox::from_id_source("settings_female_voice")
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
                egui::ComboBox::from_id_source("settings_male_voice")
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
        let mut settings = app.settings.clone();
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
