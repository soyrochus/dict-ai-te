use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealtimeEvent {
    SourceDelta {
        item_id: Option<String>,
        text: String,
    },
    SourceCompleted {
        item_id: Option<String>,
        text: String,
    },
    TranslationDelta {
        text: String,
    },
    TranslatedAudioDelta,
    SessionState {
        state: String,
    },
    Error {
        message: String,
    },
    Unknown {
        event_type: Option<String>,
    },
}

#[derive(Deserialize)]
struct RawEvent {
    #[serde(rename = "type")]
    event_type: Option<String>,
    item_id: Option<String>,
    delta: Option<String>,
    transcript: Option<String>,
    text: Option<String>,
    error: Option<serde_json::Value>,
}

pub fn parse_event(value: &serde_json::Value) -> RealtimeEvent {
    let raw: RawEvent = match serde_json::from_value(value.clone()) {
        Ok(raw) => raw,
        Err(_) => return RealtimeEvent::Unknown { event_type: None },
    };
    match raw.event_type.as_deref() {
        Some("conversation.item.input_audio_transcription.delta") => RealtimeEvent::SourceDelta {
            item_id: raw.item_id,
            text: raw.delta.unwrap_or_default(),
        },
        Some("conversation.item.input_audio_transcription.completed") => {
            RealtimeEvent::SourceCompleted {
                item_id: raw.item_id,
                text: raw.transcript.or(raw.text).unwrap_or_default(),
            }
        }
        Some("session.input_transcript.delta") => RealtimeEvent::SourceDelta {
            item_id: None,
            text: raw.delta.unwrap_or_default(),
        },
        Some("session.output_transcript.delta")
        | Some("response.output_text.delta")
        | Some("response.output_audio_transcript.delta") => RealtimeEvent::TranslationDelta {
            text: raw.delta.unwrap_or_default(),
        },
        Some("session.output_audio.delta")
        | Some("response.audio.delta")
        | Some("response.output_audio.delta") => RealtimeEvent::TranslatedAudioDelta,
        Some("error") => RealtimeEvent::Error {
            message: raw
                .error
                .and_then(|error| {
                    error
                        .get("message")
                        .and_then(|message| message.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "Realtime error".to_string()),
        },
        Some(other) if other.starts_with("session.") || other.starts_with("response.") => {
            RealtimeEvent::SessionState {
                state: other.to_string(),
            }
        }
        other => RealtimeEvent::Unknown {
            event_type: other.map(str::to_string),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_known_and_unknown_events() {
        let event = parse_event(&json!({
            "type": "conversation.item.input_audio_transcription.delta",
            "item_id": "a",
            "delta": "Hi"
        }));
        assert_eq!(
            event,
            RealtimeEvent::SourceDelta {
                item_id: Some("a".into()),
                text: "Hi".into()
            }
        );
        assert!(matches!(
            parse_event(&json!({"type": "new.event"})),
            RealtimeEvent::Unknown { .. }
        ));
    }

    #[test]
    fn parses_error_and_ignores_translated_audio_delta() {
        let error = parse_event(&json!({
            "type": "error",
            "error": {"message": "bad request"}
        }));
        assert_eq!(
            error,
            RealtimeEvent::Error {
                message: "bad request".into()
            }
        );
        assert_eq!(
            parse_event(&json!({"type": "response.audio.delta", "delta": "secret-audio"})),
            RealtimeEvent::TranslatedAudioDelta
        );
    }

    #[test]
    fn parses_ga_realtime_translation_text_delta() {
        let event = parse_event(&json!({
            "type": "response.output_text.delta",
            "delta": "Hola"
        }));
        assert_eq!(
            event,
            RealtimeEvent::TranslationDelta {
                text: "Hola".into()
            }
        );
    }
}
