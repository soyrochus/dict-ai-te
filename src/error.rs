use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("OpenAI API key is not configured")]
    MissingApiKey,
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Audio error: {0}")]
    Audio(String),
    #[error("Text-to-speech error: {0}")]
    Tts(String),
    #[error("{0}")]
    Message(String),
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Message(err.to_string())
    }
}
