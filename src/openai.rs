use std::env;
use std::io::Cursor;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use reqwest::blocking::{
    multipart::{Form, Part},
    Client, Response,
};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use rodio::{Decoder as RodioDecoder, Source};
use serde::Deserialize;
use serde_json;
use serde_json::Value;

use crate::error::AppError;
use crate::text_utils::format_structured_text;

const BASE_URL: &str = "https://api.openai.com/v1";
const TRANSCRIBE_MODEL: &str = "gpt-4o-transcribe";
const TRANSCRIBE_PROMPT: &str = "Transcribe the audio and return well-structured paragraphs. Use blank lines to separate paragraphs and fix simple punctuation errors.";
const TRANSLATE_MODEL: &str = "gpt-5-mini-2025-08-07";
const TTS_MODEL: &str = "tts-1";
const TTS_RESPONSE_FORMAT: &str = "mp3";

#[derive(Clone)]
pub struct OpenAiClient {
    http: Client,
    api_key: String,
}

impl OpenAiClient {
    pub fn from_env() -> Result<Self, AppError> {
        dotenvy::dotenv().ok();
        let api_key = env::var("OPENAI_API_KEY").map_err(|_| AppError::MissingApiKey)?;
        Self::with_api_key(api_key)
    }

    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self, AppError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(AppError::MissingApiKey);
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .context("Failed to initialise HTTP client")
            .map_err(AppError::from)?;
        Ok(Self { http, api_key })
    }

    pub fn transcribe(&self, wav_bytes: &[u8], language: Option<&str>) -> Result<String, AppError> {
        let file_part = Part::bytes(wav_bytes.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .context("Failed constructing multipart payload")
            .map_err(AppError::from)?;

        let mut form = Form::new()
            .text("model", TRANSCRIBE_MODEL.to_string())
            .text("prompt", TRANSCRIBE_PROMPT.to_string())
            .part("file", file_part);
        if let Some(lang) = language {
            if !lang.is_empty() {
                form = form.text("language", lang.to_string());
            }
        }

        let url = format!("{BASE_URL}/audio/transcriptions");
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .context("Failed sending transcription request")
            .map_err(AppError::from)?;

        let payload: TranscriptionResponse =
            parse_response(response, |body| AppError::Transcription(body))?;
        Ok(format_structured_text(&payload.text))
    }

    pub fn translate(&self, text: &str, target_language: &str) -> Result<String, AppError> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        let request = ChatCompletionRequest {
            model: TRANSLATE_MODEL.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Translate the following text to {target}. Format the translation into clear paragraphs separated by blank lines. Return only the translated text.\n\n{text}",
                    target = target_language,
                    text = text.trim()
                ),
            }],
        };

        let url = format!("{BASE_URL}/chat/completions");
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .context("Failed sending translation request")
            .map_err(AppError::from)?;

        let payload: ChatCompletionResponse =
            parse_response(response, |body| AppError::Translation(body))?;
        let translated = payload
            .choices
            .get(0)
            .and_then(|choice| choice.message.content.as_deref())
            .unwrap_or_default();
        Ok(format_structured_text(translated))
    }

    pub fn text_to_speech(&self, text: &str, voice: &str) -> Result<Vec<u8>, AppError> {
        let clean = text.trim();
        if clean.is_empty() {
            return Err(AppError::Tts(
                "Cannot generate speech for empty text".into(),
            ));
        }

        let payload = TtsRequest {
            model: TTS_MODEL.to_string(),
            input: clean.to_string(),
            voice: voice.to_string(),
            response_format: TTS_RESPONSE_FORMAT.to_string(),
        };

        let url = format!("{BASE_URL}/audio/speech");
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .header(
                ACCEPT,
                match TTS_RESPONSE_FORMAT {
                    "mp3" => "audio/mpeg",
                    "wav" => "audio/wav",
                    "ogg" => "audio/ogg",
                    format => format,
                },
            )
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .context("Failed sending text-to-speech request")
            .map_err(AppError::from)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .unwrap_or_else(|_| "Unable to decode error response".to_string());
            return Err(AppError::Tts(format!("{status}: {body}")));
        }

        let is_json = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|ty| ty.contains("json"))
            .unwrap_or(false);

        if is_json {
            let envelope: Value = response
                .json()
                .context("Failed to parse TTS JSON response")
                .map_err(AppError::from)?;
            decode_tts_json(envelope)
        } else {
            response
                .bytes()
                .map(|b| b.to_vec())
                .context("Failed reading TTS response body")
                .map_err(AppError::from)
        }
    }
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

#[derive(serde::Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(serde::Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(serde::Serialize)]
struct TtsRequest {
    model: String,
    input: String,
    voice: String,
    #[serde(rename = "response_format")]
    response_format: String,
}

#[derive(Default)]
struct TtsPayloadInfo {
    chunks: Vec<String>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    mime_type: Option<String>,
    format: Option<String>,
}

fn decode_tts_json(value: Value) -> Result<Vec<u8>, AppError> {
    let mut info = TtsPayloadInfo::default();
    collect_tts_payload(&value, &mut info);
    if info.chunks.is_empty() {
        return Err(AppError::Tts(
            "No audio content in TTS response".to_string(),
        ));
    }

    let mut pcm_samples: Vec<i16> = Vec::new();
    let mut sample_rate = info.sample_rate;
    let mut channels = info.channels;

    for chunk in info.chunks {
        let Some(bytes) = decode_base64_chunk(&chunk) else {
            continue;
        };
        match chunk_to_pcm(&bytes, sample_rate, channels) {
            Ok((mut samples, sr, ch)) => {
                if sample_rate.map_or(false, |existing| existing != sr) {
                    continue;
                }
                if channels.map_or(false, |existing| existing != ch) {
                    continue;
                }
                sample_rate = sample_rate.or(Some(sr));
                channels = channels.or(Some(ch));
                pcm_samples.append(&mut samples);
            }
            Err(_) => continue,
        }
    }

    if pcm_samples.is_empty() {
        return Err(AppError::Tts(
            "TTS response did not contain playable audio".to_string(),
        ));
    }

    let sr = sample_rate.unwrap_or(24_000);
    let ch = channels.unwrap_or(1);
    encode_pcm_to_wav(&pcm_samples, sr, ch)
}

fn collect_tts_payload(value: &Value, info: &mut TtsPayloadInfo) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                match key.as_str() {
                    "b64_json" | "audio" => {
                        if let Some(s) = val.as_str() {
                            info.chunks.push(s.to_string());
                        }
                    }
                    "sample_rate" | "sampling_rate" => {
                        if info.sample_rate.is_none() {
                            if let Some(rate) = val.as_u64() {
                                info.sample_rate = Some(rate as u32);
                            }
                        }
                    }
                    "channels" | "num_channels" => {
                        if info.channels.is_none() {
                            if let Some(ch) = val.as_u64() {
                                info.channels = Some(ch as u16);
                            }
                        }
                    }
                    "mime_type" | "content_type" | "format" | "audio_format" => {
                        if info.mime_type.is_none() {
                            if let Some(s) = val.as_str() {
                                info.mime_type = Some(s.to_string());
                            }
                        }
                        if key == "format" || key == "audio_format" {
                            if info.format.is_none() {
                                if let Some(s) = val.as_str() {
                                    info.format = Some(s.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            for val in map.values() {
                collect_tts_payload(val, info);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_tts_payload(item, info);
            }
        }
        _ => {}
    }
}

fn looks_like_wav(data: &[u8]) -> bool {
    data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE"
}

fn decode_base64_chunk(chunk: &str) -> Option<Vec<u8>> {
    let trimmed = chunk.trim();
    if trimmed.is_empty() {
        return None;
    }
    let payload = if let Some(idx) = trimmed.find("base64,") {
        &trimmed[idx + "base64,".len()..]
    } else {
        trimmed
    };
    BASE64_STANDARD.decode(payload).ok()
}

fn chunk_to_pcm(
    bytes: &[u8],
    sample_rate_hint: Option<u32>,
    channels_hint: Option<u16>,
) -> Result<(Vec<i16>, u32, u16), AppError> {
    if looks_like_wav(bytes) {
        return wav_to_pcm(bytes);
    }
    if let Ok(decoder) = RodioDecoder::new(Cursor::new(bytes.to_vec())) {
        return rodio_to_pcm(decoder);
    }
    raw_pcm_to_samples(bytes, sample_rate_hint, channels_hint)
}

fn raw_pcm_to_samples(
    bytes: &[u8],
    sample_rate_hint: Option<u32>,
    channels_hint: Option<u16>,
) -> Result<(Vec<i16>, u32, u16), AppError> {
    let sample_rate = sample_rate_hint
        .ok_or_else(|| AppError::Tts("Missing sample rate for PCM audio chunk".to_string()))?;
    let channels = channels_hint
        .ok_or_else(|| AppError::Tts("Missing channel count for PCM audio chunk".to_string()))?;
    if bytes.len() % 2 != 0 {
        return Err(AppError::Tts(
            "Odd byte length in PCM audio chunk".to_string(),
        ));
    }
    let mut samples = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok((samples, sample_rate, channels))
}

fn wav_to_pcm(bytes: &[u8]) -> Result<(Vec<i16>, u32, u16), AppError> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut reader = hound::WavReader::new(cursor)
        .context("Failed to parse WAV chunk")
        .map_err(AppError::from)?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|res| float_to_i16(res.unwrap_or(0.0)))
            .collect(),
        hound::SampleFormat::Int => match spec.bits_per_sample {
            8 => reader
                .samples::<i8>()
                .map(|res| ((res.unwrap_or(0) as f32 / i8::MAX as f32) * i16::MAX as f32) as i16)
                .collect(),
            16 => reader
                .samples::<i16>()
                .map(|res| res.unwrap_or(0))
                .collect(),
            24 | 32 => reader
                .samples::<i32>()
                .map(|res| {
                    let val = res.unwrap_or(0) as f32 / i32::MAX as f32;
                    float_to_i16(val)
                })
                .collect(),
            other => {
                return Err(AppError::Tts(
                    format!("Unsupported WAV bit depth: {other}",),
                ))
            }
        },
    };
    Ok((samples, sample_rate, channels))
}

fn rodio_to_pcm(decoder: RodioDecoder<Cursor<Vec<u8>>>) -> Result<(Vec<i16>, u32, u16), AppError> {
    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels() as u16;
    let samples: Vec<i16> = decoder.convert_samples::<f32>().map(float_to_i16).collect();
    Ok((samples, sample_rate, channels))
}

fn float_to_i16(sample: f32) -> i16 {
    let clamped = sample.clamp(-1.0, 1.0);
    (clamped * i16::MAX as f32) as i16
}

fn encode_pcm_to_wav(
    samples: &[i16],
    sample_rate: u32,
    channels: u16,
) -> Result<Vec<u8>, AppError> {
    use hound::{WavSpec, WavWriter};
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec)
            .context("Failed to create WAV writer")
            .map_err(AppError::from)?;
        for sample in samples {
            writer
                .write_sample(*sample)
                .context("Failed writing WAV sample")
                .map_err(AppError::from)?;
        }
        writer
            .finalize()
            .context("Failed finalising WAV payload")
            .map_err(AppError::from)?;
    }
    Ok(cursor.into_inner())
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
    code: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

fn parse_response<T, F>(response: Response, map_err: F) -> Result<T, AppError>
where
    T: for<'de> Deserialize<'de>,
    F: Fn(String) -> AppError,
{
    if response.status().is_success() {
        response
            .json::<T>()
            .context("Failed decoding API response")
            .map_err(AppError::from)
    } else {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        let message = if let Ok(envelope) = serde_json::from_str::<ErrorEnvelope>(&body) {
            let mut msg = envelope
                .error
                .message
                .unwrap_or_else(|| "Unknown error".into());
            if let Some(code) = envelope.error.code {
                msg = format!("{msg} ({code})");
            }
            if let Some(kind) = envelope.error.kind {
                msg = format!("{msg} [{kind}]");
            }
            msg
        } else if body.trim().is_empty() {
            format!("HTTP {status}")
        } else {
            format!("HTTP {status}: {body}")
        };
        Err(map_err(message))
    }
}
