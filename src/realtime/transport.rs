use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, AUTHORIZATION};
use tokio_tungstenite::tungstenite::Message;

use crate::error::AppError;
use crate::realtime::events::{parse_event, RealtimeEvent};

// Verified against OpenAI Realtime GA docs on 2026-05-16:
// - WebSocket transcription sessions use the realtime endpoint with intent=transcription.
// - GA WebSocket authentication uses Authorization only; do not send the legacy
//   OpenAI-Beta: realtime=v1 header, which selects the retired beta API.
// - Audio chunks are base64-encoded mono PCM16 at 24 kHz via input_audio_buffer.append.
// - Current documented realtime transcription models include gpt-4o-transcribe,
//   gpt-4o-mini-transcribe, gpt-4o-transcribe-latest, and whisper-1.
pub const TRANSCRIPTION_URL: &str = "wss://api.openai.com/v1/realtime?intent=transcription";
pub const TRANSCRIPTION_MODEL: &str = "gpt-4o-transcribe";

#[derive(Debug, Clone)]
pub struct RealtimeSessionConfig {
    pub api_key: String,
    pub source_language: Option<String>,
}

pub async fn run_live_transcription(
    config: RealtimeSessionConfig,
    audio_rx: mpsc::Receiver<String>,
    event_tx: mpsc::Sender<RealtimeEvent>,
    stop_rx: oneshot::Receiver<()>,
) -> Result<(), AppError> {
    run_verified_transcription_session(config, audio_rx, event_tx, stop_rx).await
}

async fn run_verified_transcription_session(
    config: RealtimeSessionConfig,
    mut audio_rx: mpsc::Receiver<String>,
    event_tx: mpsc::Sender<RealtimeEvent>,
    mut stop_rx: oneshot::Receiver<()>,
) -> Result<(), AppError> {
    let mut request = TRANSCRIPTION_URL
        .into_client_request()
        .map_err(|err| AppError::Message(err.to_string()))?;
    request.headers_mut().insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", config.api_key.trim()))
            .map_err(|err| AppError::Message(err.to_string()))?,
    );
    let (socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|err| AppError::Message(format!("Realtime connection failed: {err}")))?;
    let (mut write, mut read) = socket.split();

    let mut session = json!({
        "type": "session.update",
        "session": {
            "type": "transcription",
            "audio": {
                "input": {
                    "format": {"type": "audio/pcm", "rate": 24000},
                    "transcription": {"model": TRANSCRIPTION_MODEL},
                    "turn_detection": {
                        "type": "server_vad",
                        "threshold": 0.5,
                        "prefix_padding_ms": 300,
                        "silence_duration_ms": 500
                    }
                }
            }
        }
    });
    if let Some(language) = config
        .source_language
        .as_deref()
        .filter(|lang| !lang.is_empty())
    {
        session["session"]["audio"]["input"]["transcription"]["language"] = json!(language);
    }
    write
        .send(Message::Text(session.to_string()))
        .await
        .map_err(|err| AppError::Message(format!("Realtime session update failed: {err}")))?;
    let _ = event_tx
        .send(RealtimeEvent::SessionState {
            state: "connecting".to_string(),
        })
        .await;

    loop {
        tokio::select! {
            _ = &mut stop_rx => {
                let _ = write.send(Message::Text(json!({"type": "input_audio_buffer.commit"}).to_string())).await;
                let _ = write.send(Message::Close(None)).await;
                break;
            }
            chunk = audio_rx.recv() => {
                match chunk {
                    Some(chunk) => {
                        let message = json!({"type": "input_audio_buffer.append", "audio": chunk});
                        write.send(Message::Text(message.to_string())).await
                            .map_err(|err| AppError::Message(format!("Realtime audio send failed: {err}")))?;
                    }
                    None => {
                        let _ = write.send(Message::Text(json!({"type": "input_audio_buffer.commit"}).to_string())).await;
                        let _ = write.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
            message = read.next() => {
                let Some(message) = message else { break; };
                let message = message.map_err(|err| AppError::Message(format!("Realtime receive failed: {err}")))?;
                if message.is_close() {
                    break;
                }
                if let Ok(text) = message.to_text() {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                        let event = parse_event(&value);
                        if matches!(event, RealtimeEvent::SessionState { .. } | RealtimeEvent::SourceDelta { .. } | RealtimeEvent::SourceCompleted { .. } | RealtimeEvent::Error { .. }) {
                            let _ = event_tx.send(event).await;
                        }
                    }
                }
            }
        }
    }

    let _ = event_tx
        .send(RealtimeEvent::SessionState {
            state: "disconnected".to_string(),
        })
        .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_ga_transcription_endpoint_without_beta_query() {
        assert_eq!(
            TRANSCRIPTION_URL,
            "wss://api.openai.com/v1/realtime?intent=transcription"
        );
        assert!(!TRANSCRIPTION_URL.contains("beta"));
    }
}
