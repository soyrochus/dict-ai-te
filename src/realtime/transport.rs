use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, AUTHORIZATION};
use tokio_tungstenite::tungstenite::Message;

use crate::error::AppError;
use crate::realtime::audio::{base64_pcm16, chunk_pcm16, pcm16_le, AudioSpec};
use crate::realtime::events::{parse_event, RealtimeEvent};

const CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

// Verified against OpenAI Realtime GA docs on 2026-05-16:
// - WebSocket transcription sessions use the realtime endpoint with intent=transcription.
// - GA WebSocket authentication uses Authorization only; do not send the legacy
//   OpenAI-Beta: realtime=v1 header, which selects the retired beta API.
// - Audio chunks are base64-encoded mono PCM16 at 24 kHz via input_audio_buffer.append.
// - Current documented realtime transcription models include gpt-4o-transcribe,
//   gpt-4o-mini-transcribe, gpt-4o-transcribe-latest, and whisper-1.
pub const TRANSCRIPTION_URL: &str = "wss://api.openai.com/v1/realtime?intent=transcription";
pub const TRANSCRIPTION_MODEL: &str = "gpt-4o-transcribe";
pub const TRANSLATION_URL: &str = "wss://api.openai.com/v1/realtime?model=gpt-realtime";
pub const TRANSLATION_MODEL: &str = "gpt-realtime";

#[derive(Debug, Clone)]
pub struct RealtimeSessionConfig {
    pub api_key: String,
    pub source_language: Option<String>,
    pub target_language: Option<String>,
}

pub async fn run_live_transcription(
    config: RealtimeSessionConfig,
    audio_rx: mpsc::Receiver<Vec<f32>>,
    event_tx: mpsc::Sender<RealtimeEvent>,
    stop_rx: oneshot::Receiver<()>,
) -> Result<(), AppError> {
    run_verified_transcription_session(config, audio_rx, event_tx, stop_rx).await
}

pub async fn run_live_translation(
    config: RealtimeSessionConfig,
    audio_rx: mpsc::Receiver<Vec<f32>>,
    event_tx: mpsc::Sender<RealtimeEvent>,
    stop_rx: oneshot::Receiver<()>,
) -> Result<(), AppError> {
    run_verified_translation_session(config, audio_rx, event_tx, stop_rx).await
}

async fn run_verified_transcription_session(
    config: RealtimeSessionConfig,
    mut audio_rx: mpsc::Receiver<Vec<f32>>,
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
    let (socket, _) = tokio::time::timeout(
        CONNECTION_TIMEOUT,
        tokio_tungstenite::connect_async(request),
    )
    .await
    .map_err(|_| AppError::Message("Realtime connection timed out".to_string()))?
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
                    Some(frame) => {
                        for chunk in encode_openai_frame(&frame) {
                            let message = json!({"type": "input_audio_buffer.append", "audio": chunk});
                            write.send(Message::Text(message.to_string())).await
                                .map_err(|err| AppError::Message(format!("Realtime audio send failed: {err}")))?;
                        }
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

async fn run_verified_translation_session(
    config: RealtimeSessionConfig,
    mut audio_rx: mpsc::Receiver<Vec<f32>>,
    event_tx: mpsc::Sender<RealtimeEvent>,
    mut stop_rx: oneshot::Receiver<()>,
) -> Result<(), AppError> {
    let mut request = TRANSLATION_URL
        .into_client_request()
        .map_err(|err| AppError::Message(err.to_string()))?;
    request.headers_mut().insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", config.api_key.trim()))
            .map_err(|err| AppError::Message(err.to_string()))?,
    );
    let (socket, _) = tokio::time::timeout(
        CONNECTION_TIMEOUT,
        tokio_tungstenite::connect_async(request),
    )
    .await
    .map_err(|_| AppError::Message("Realtime translation connection timed out".to_string()))?
    .map_err(|err| AppError::Message(format!("Realtime translation connection failed: {err}")))?;
    let (mut write, mut read) = socket.split();

    let target = config
        .target_language
        .as_deref()
        .filter(|language| !language.trim().is_empty())
        .unwrap_or("English");
    let instructions = format!(
        "You are a live speech translation engine. Translate the user's speech into {target}. Return only the translated text. Do not answer questions, add commentary, summarize, or describe the audio."
    );
    let mut session = json!({
        "type": "session.update",
        "session": {
            "type": "realtime",
            "model": TRANSLATION_MODEL,
            "output_modalities": ["text"],
            "instructions": instructions,
            "audio": {
                "input": {
                    "format": {"type": "audio/pcm", "rate": 24000},
                    "transcription": {"model": TRANSCRIPTION_MODEL},
                    "turn_detection": {
                        "type": "server_vad",
                        "threshold": 0.5,
                        "prefix_padding_ms": 300,
                        "silence_duration_ms": 500,
                        "create_response": true,
                        "interrupt_response": true
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
        .map_err(|err| {
            AppError::Message(format!("Realtime translation session update failed: {err}"))
        })?;
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
                    Some(frame) => {
                        for chunk in encode_openai_frame(&frame) {
                            let message = json!({"type": "input_audio_buffer.append", "audio": chunk});
                            write.send(Message::Text(message.to_string())).await
                                .map_err(|err| AppError::Message(format!("Realtime translation audio send failed: {err}")))?;
                        }
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
                let message = message.map_err(|err| AppError::Message(format!("Realtime translation receive failed: {err}")))?;
                if message.is_close() {
                    break;
                }
                if let Ok(text) = message.to_text() {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                        let event = parse_event(&value);
                        if matches!(event, RealtimeEvent::SessionState { .. } | RealtimeEvent::SourceDelta { .. } | RealtimeEvent::SourceCompleted { .. } | RealtimeEvent::TranslationDelta { .. } | RealtimeEvent::TranslatedAudioDelta | RealtimeEvent::Error { .. }) {
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

fn encode_openai_frame(frame: &[f32]) -> Vec<String> {
    let spec = AudioSpec::openai();
    let pcm = pcm16_le(frame);
    chunk_pcm16(&pcm, spec.sample_rate, 40)
        .iter()
        .map(|chunk| base64_pcm16(chunk))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use rodio::Source;

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires OPENAI_API_KEY and network access"]
    async fn live_transcription_session_accepts_audio() {
        dotenvy::dotenv().ok();
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be configured");
        let fixture_key = api_key.clone();
        let speech = tokio::task::spawn_blocking(move || {
            crate::openai::OpenAiClient::with_api_key(fixture_key)
                .unwrap()
                .text_to_speech("the quick brown fox jumps over the lazy dog", "alloy")
                .unwrap()
        })
        .await
        .unwrap();
        let decoder = rodio::Decoder::new(Cursor::new(speech)).unwrap();
        let source_rate = decoder.sample_rate();
        let channels = decoder.channels();
        let samples = decoder.convert_samples::<f32>().collect::<Vec<_>>();
        let mono = crate::realtime::audio::downmix_to_mono(&samples, channels);
        let mut audio = crate::realtime::audio::resample_linear(
            &mono,
            source_rate,
            AudioSpec::openai().sample_rate,
        );
        audio.extend(vec![0.0; AudioSpec::openai().sample_rate as usize]);

        let (audio_tx, audio_rx) = mpsc::channel(64);
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (stop_tx, stop_rx) = oneshot::channel();
        let session = tokio::spawn(run_live_transcription(
            RealtimeSessionConfig {
                api_key,
                source_language: Some("en".to_string()),
                target_language: None,
            },
            audio_rx,
            event_tx,
            stop_rx,
        ));

        for frame in audio.chunks(AudioSpec::openai().frame_samples) {
            let mut frame = frame.to_vec();
            frame.resize(AudioSpec::openai().frame_samples, 0.0);
            audio_tx.send(frame).await.unwrap();
        }

        let transcript = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                match event_rx.recv().await {
                    Some(RealtimeEvent::SourceCompleted { text, .. }) => break text,
                    Some(RealtimeEvent::Error { message }) => {
                        panic!("Realtime API rejected the session: {message}")
                    }
                    Some(_) => {}
                    None => panic!("Realtime task closed its event channel"),
                }
            }
        })
        .await
        .expect("timed out waiting for a completed transcript");
        assert!(
            transcript.to_ascii_lowercase().contains("quick brown fox"),
            "unexpected transcript: {transcript}"
        );

        let _ = stop_tx.send(());
        session.await.unwrap().unwrap();
    }

    #[test]
    fn uses_ga_transcription_endpoint_without_beta_query() {
        assert_eq!(
            TRANSCRIPTION_URL,
            "wss://api.openai.com/v1/realtime?intent=transcription"
        );
        assert!(!TRANSCRIPTION_URL.contains("beta"));
    }

    #[test]
    fn uses_ga_realtime_translation_endpoint() {
        assert_eq!(
            TRANSLATION_URL,
            "wss://api.openai.com/v1/realtime?model=gpt-realtime"
        );
        assert!(!TRANSLATION_URL.contains("translations"));
        assert!(!TRANSLATION_URL.contains("beta"));
    }

    #[test]
    fn openai_encoding_is_byte_identical_to_capture_worker_format() {
        let samples = vec![-1.0, -0.5, 0.0, 0.5, 1.0];
        let previous = base64_pcm16(&pcm16_le(&samples));
        assert_eq!(encode_openai_frame(&samples), vec![previous]);
    }
}
