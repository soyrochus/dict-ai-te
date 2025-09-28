mod app;
mod audio;
mod constants;
mod error;
mod openai;
mod settings;
mod text_utils;

use app::DictaiteApp;
use openai::OpenAiClient;

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
        Box::new(move |_cc| {
            let client = openai_client.clone();
            Box::new(DictaiteApp::new(client))
        }),
    )
}
