use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("OpenAI API key is missing. Set OPENAI_API_KEY or enter a key in Settings → OpenAI API key.")]
    MissingApiKey,
    #[error("OpenAI rejected the API key. Check OPENAI_API_KEY or the key stored in Settings; the stored value was not cleared.")]
    RejectedApiKey,
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

pub fn redact_secret(message: &str, secret: &str) -> String {
    let secret = secret.trim();
    if secret.is_empty() {
        message.to_string()
    } else {
        message.replace(secret, "[REDACTED]")
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Message(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_rejected_credentials_are_actionably_distinct() {
        let missing = AppError::MissingApiKey.to_string();
        let rejected = AppError::RejectedApiKey.to_string();
        assert!(missing.contains("OPENAI_API_KEY"));
        assert!(missing.contains("Settings"));
        assert_ne!(missing, rejected);
    }

    #[test]
    fn sentinel_secret_is_redacted() {
        let secret = "sk-sentinel-do-not-log";
        let message = redact_secret(&format!("Authorization: Bearer {secret}"), secret);
        assert!(!message.contains(secret));
        assert!(message.contains("[REDACTED]"));
    }
}
