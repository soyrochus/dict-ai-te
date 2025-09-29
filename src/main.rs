mod app;
mod audio;
mod constants;
mod error;
mod openai;
mod settings;
mod text_utils;

use app::DictaiteApp;
use openai::OpenAiClient;
use std::path::Path;

fn configure_fonts(ctx: &egui::Context) {
    // Start with default fonts
    let mut fonts = egui::FontDefinitions::default();

    // Helper to try load a font file from disk (system or local project)
    let mut try_add_font = |name: &str, path: &str| {
        if Path::new(path).exists() {
            if let Ok(bytes) = std::fs::read(path) {
                fonts
                    .font_data
                    .insert(name.to_owned(), egui::FontData::from_owned(bytes));
                // Add as a fallback for proportional text
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push(name.to_owned());
                // And monospace too, just in case we display code/logs with these glyphs
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push(name.to_owned());
                log::info!("Loaded fallback font: {name} from {path}");
            }
        }
    };

    // Common system locations for Noto fonts on Linux (adjust as needed for other OSes)
    // We add several families to cover CJK, Arabic, Devanagari, Hebrew, Thai, etc.
    let candidates: &[(&str, &str)] = &[
        // Local project assets (if you choose to vendor fonts):
        ("NotoSans-Regular", "assets/fonts/NotoSans-Regular.ttf"),
        ("NotoSansCJK-Regular", "assets/fonts/NotoSansCJK-Regular.ttc"),
        ("NotoSansJP-Regular", "assets/fonts/NotoSansJP-Regular.otf"),
        ("NotoSansKR-Regular", "assets/fonts/NotoSansKR-Regular.otf"),
        ("NotoSansSC-Regular", "assets/fonts/NotoSansSC-Regular.otf"),
        ("NotoSansArabic-Regular", "assets/fonts/NotoSansArabic-Regular.ttf"),
        ("NotoSansDevanagari-Regular", "assets/fonts/NotoSansDevanagari-Regular.ttf"),
        ("NotoSansHebrew-Regular", "assets/fonts/NotoSansHebrew-Regular.ttf"),
        ("NotoSansThai-Regular", "assets/fonts/NotoSansThai-Regular.ttf"),

        // Typical Linux paths (Ubuntu/Debian):
        ("NotoSans-Regular", "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"),
        ("NotoSansArabic-Regular", "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf"),
        ("NotoSansDevanagari-Regular", "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf"),
        ("NotoSansHebrew-Regular", "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf"),
        ("NotoSansThai-Regular", "/usr/share/fonts/truetype/noto/NotoSansThai-Regular.ttf"),
        // CJK often installed as TTC/OTF in these dirs:
        ("NotoSansCJK-Regular", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        ("NotoSansJP-Regular", "/usr/share/fonts/opentype/noto/NotoSansJP-Regular.otf"),
        ("NotoSansKR-Regular", "/usr/share/fonts/opentype/noto/NotoSansKR-Regular.otf"),
        ("NotoSansSC-Regular", "/usr/share/fonts/opentype/noto/NotoSansSC-Regular.otf"),
    ];

    for (name, path) in candidates {
        try_add_font(name, path);
    }

    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let openai_client = match OpenAiClient::from_env() {
        Ok(client) => Some(client),
        Err(err) => {
            log::warn!("OpenAI client unavailable: {err}");
            None
        }
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([440.0, 780.0])
            .with_min_inner_size([360.0, 640.0])
            .with_transparent(false)
            .with_decorations(true)
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "dict-ai-te (Rust)",
        native_options,
        Box::new(move |cc| {
            // Ensure fonts cover non-Latin scripts used in language names
            configure_fonts(&cc.egui_ctx);
            let client = openai_client.clone();
            Box::new(DictaiteApp::new(client))
        }),
    )
}
