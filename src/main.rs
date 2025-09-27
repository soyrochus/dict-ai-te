use std::time::{Duration, Instant};

use eframe::{App, Frame, NativeOptions};
use egui::*;

fn main() -> eframe::Result<()> {
    let opts = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([420.0, 760.0])
            .with_min_inner_size([360.0, 640.0])
            .with_resizable(true)
            .with_decorations(true),
        ..Default::default()
    };
    eframe::run_native(
        "dict-ai-te (Rust)",
        opts,
        Box::new(|_cc| Box::<DictAiTe>::default()),
    )
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Gender {
    Female,
    Male,
}

struct DictAiTe {
    // Main screen
    settings_open: bool,
    is_recording: bool,
    record_started_at: Option<Instant>,
    elapsed: Duration,
    origin_lang_ix: usize,
    translate_on: bool,
    dest_lang_ix: usize,
    transcript: String,
    gender: Gender,

    // Settings screen
    def_lang_ix: usize,
    translate_by_default: bool,
    def_target_lang_ix: usize,
    female_voice_ix: usize,
    male_voice_ix: usize,

    // Lists
    languages: Vec<&'static str>,
    voices: Vec<&'static str>,
}

impl Default for DictAiTe {
    fn default() -> Self {
        Self {
            settings_open: false,
            is_recording: false,
            record_started_at: None,
            elapsed: Duration::ZERO,
            origin_lang_ix: 0,
            translate_on: false,
            dest_lang_ix: 1, // English
            transcript: String::new(),
            gender: Gender::Female,

            def_lang_ix: 0,
            translate_by_default: false,
            def_target_lang_ix: 1, // English
            female_voice_ix: 0,    // Nova
            male_voice_ix: 1,      // Onyx

            languages: vec![
                "Default (Auto-detect)",
                "English",
                "Spanish",
                "French",
                "German",
                "Italian",
                "Portuguese",
                "Dutch",
            ],
            voices: vec!["Nova", "Onyx", "Verse", "Alloy", "Amber"],
        }
    }
}

impl App for DictAiTe {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        // Fake “timer”
        if self.is_recording {
            if let Some(start) = self.record_started_at {
                self.elapsed = start.elapsed();
            }
            ctx.request_repaint(); // keep ticking
        }

        TopBottomPanel::top("topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("dict-ai-te").heading());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Settings").clicked() {
                        self.settings_open = true;
                    }
                });
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            progress_bar(ui, self.is_recording);

            ui.add_space(8.0);
            mic_card(ui, self);

            ui.add_space(8.0);
            time_and_hint(ui, self);

            ui.add_space(10.0);
            lang_section(ui, self);

            ui.add_space(10.0);
            transcript_box(ui, self);

            ui.add_space(10.0);
            footer_controls(ui, self);
        });

        if self.settings_open {
            settings_window(ctx, self);
        }
    }
}

// ---------- UI sections ----------

fn progress_bar(ui: &mut Ui, is_recording: bool) {
    let fill = if is_recording { 0.5 } else { 0.0 };
    let bar = egui::widgets::ProgressBar::new(fill).show_percentage(); // no args in 0.27
    ui.add(bar);
}

fn mic_card(ui: &mut Ui, app: &mut DictAiTe) {
    egui::Frame::group(ui.style()) // <- egui::Frame, not eframe::Frame
        .inner_margin(Margin::same(12.0))
        .rounding(Rounding::same(8.0))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), 180.0),
                Layout::top_down(Align::Center),
                |ui| {
                    ui.add_space(8.0);

                    // Simple “mic” drawing
                    let (rect, _) = ui.allocate_exact_size(vec2(120.0, 120.0), Sense::hover());
                    let painter = ui.painter_at(rect);
                    let center = rect.center();
                    let radius = 32.0;

                    painter.rect_filled(
                        Rect::from_center_size(center, vec2(radius * 1.6, radius * 2.2)),
                        10.0,
                        ui.visuals().widgets.inactive.fg_stroke.color.linear_multiply(0.08),
                    );
                    for i in -3..=3 {
                        let y = center.y + i as f32 * 10.0;
                        painter.line_segment(
                            [pos2(center.x - radius * 0.7, y), pos2(center.x + radius * 0.7, y)],
                            ui.visuals().widgets.inactive.fg_stroke,
                        );
                    }
                    painter.line_segment(
                        [pos2(center.x, center.y + radius * 1.25), pos2(center.x, center.y + 70.0)],
                        ui.visuals().widgets.inactive.fg_stroke,
                    );
                    painter.line_segment(
                        [pos2(center.x - 20.0, center.y + 70.0), pos2(center.x + 20.0, center.y + 70.0)],
                        ui.visuals().widgets.inactive.fg_stroke,
                    );

                    ui.add_space(6.0);
                    let start_stop = if app.is_recording {
                        "Stop"
                    } else {
                        "Press to start recording"
                    };
                    if ui
                        .add(Button::new(start_stop).min_size(vec2(240.0, 28.0)))
                        .clicked()
                    {
                        app.is_recording = !app.is_recording;
                        if app.is_recording {
                            app.record_started_at = Some(Instant::now());
                            app.elapsed = Duration::ZERO;
                            if app.translate_by_default {
                                app.translate_on = true;
                            }
                        } else {
                            app.record_started_at = None;
                        }
                    }
                },
            );
        });
}

fn time_and_hint(ui: &mut Ui, app: &mut DictAiTe) {
    let t = app.elapsed;
    let hh = t.as_secs() / 3600;
    let mm = (t.as_secs() % 3600) / 60;
    let ss = t.as_secs() % 60;
    ui.vertical_centered(|ui| {
        ui.monospace(format!("{:02}:{:02}:{:02}", hh, mm, ss));
    });
}

fn lang_section(ui: &mut Ui, app: &mut DictAiTe) {
    ui.horizontal(|ui| {
        ui.label("Origin language");
        ui.separator();
    });
    ui.add_space(4.0);

    ComboBox::from_id_source("origin_lang")
        .selected_text(app.languages[app.origin_lang_ix])
        .show_ui(ui, |ui| {
            for (i, label) in app.languages.iter().enumerate() {
                ui.selectable_value(&mut app.origin_lang_ix, i, *label);
            }
        });

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label("Translate to");
        ui.toggle_value(&mut app.translate_on, ""); // Toggle → checkbox-ish
    });

    ui.add_enabled_ui(app.translate_on, |ui| {
        ui.label("Destination language");
        ComboBox::from_id_source("dest_lang")
            .selected_text(app.languages[app.dest_lang_ix])
            .show_ui(ui, |ui| {
                for (i, label) in app.languages.iter().enumerate() {
                    if i == 0 {
                        continue; // skip "Default" for target
                    }
                    ui.selectable_value(&mut app.dest_lang_ix, i, *label);
                }
            });
    });
}

fn transcript_box(ui: &mut Ui, app: &mut DictAiTe) {
    ui.add_space(6.0);
    let desired = [ui.available_width(), 260.0];
    ui.add_sized(desired, egui::widgets::TextEdit::multiline(&mut app.transcript).hint_text(
        "Transcribed text will appear here…",
    ));
}

fn footer_controls(ui: &mut Ui, app: &mut DictAiTe) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("⬇ Download").clicked() {}
        if ui.button("⧉ Copy").clicked() {}
        if ui.button("▶ Play").clicked() {}

        ui.add_space(16.0);
        ui.separator();

        ui.radio_value(&mut app.gender, Gender::Female, "Female");
        ui.radio_value(&mut app.gender, Gender::Male, "Male");
    });
}

// ---------- Settings window ----------

fn settings_window(ctx: &Context, app: &mut DictAiTe) {
    let mut open = true;
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_size([380.0, 340.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 12.0);

            ui.vertical(|ui| {
                ui.label("Default language");
                ComboBox::from_id_source("def_lang")
                    .selected_text(app.languages[app.def_lang_ix])
                    .show_ui(ui, |ui| {
                        for (i, label) in app.languages.iter().enumerate() {
                            ui.selectable_value(&mut app.def_lang_ix, i, *label);
                        }
                    });

                ui.horizontal(|ui| {
                    ui.label("Translate by default");
                    ui.toggle_value(&mut app.translate_by_default, "");
                });

                ui.add_enabled_ui(app.translate_by_default, |ui| {
                    ui.label("Default target language");
                    ComboBox::from_id_source("def_target_lang")
                        .selected_text(app.languages[app.def_target_lang_ix])
                        .show_ui(ui, |ui| {
                            for (i, label) in app.languages.iter().enumerate() {
                                if i == 0 {
                                    continue;
                                }
                                ui.selectable_value(&mut app.def_target_lang_ix, i, *label);
                            }
                        });
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Female voice");
                    ComboBox::from_id_source("female_voice")
                        .selected_text(app.voices[app.female_voice_ix])
                        .show_ui(ui, |ui| {
                            for (i, v) in app.voices.iter().enumerate() {
                                ui.selectable_value(&mut app.female_voice_ix, i, *v);
                            }
                        });
                    if ui.button("Play").clicked() {}
                });

                ui.horizontal(|ui| {
                    ui.label("Male voice");
                    ComboBox::from_id_source("male_voice")
                        .selected_text(app.voices[app.male_voice_ix])
                        .show_ui(ui, |ui| {
                            for (i, v) in app.voices.iter().enumerate() {
                                ui.selectable_value(&mut app.male_voice_ix, i, *v);
                            }
                        });
                    if ui.button("Play").clicked() {}
                });

                ui.add_space(10.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Save").clicked() {
                        app.settings_open = false;
                    }
                    if ui.button("Cancel").clicked() {
                        app.settings_open = false;
                    }
                });
            });
        });

    if !open {
        app.settings_open = false;
    }
}
