# Architecture Guide

This document is a deep-dive into how **dict-ai-te** works. It treats the **Rust/egui implementation as the reference** — it is the most complete and the most recent — and documents the Python GTK and Flask/web implementations by showing where they mirror that reference and where they still diverge.

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Repository Layout](#2-repository-layout)
3. [The Three Implementations at a Glance](#3-the-three-implementations-at-a-glance)
4. [Audio Pipeline](#4-audio-pipeline)
5. [OpenAI Realtime Sessions](#5-openai-realtime-sessions)
6. [Event Parsing and Normalization](#6-event-parsing-and-normalization)
7. [Transcript Assembly](#7-transcript-assembly)
8. [Text-to-Speech and Playback](#8-text-to-speech-and-playback)
9. [Settings and Configuration](#9-settings-and-configuration)
10. [UI Layer — Rust (egui/eframe)](#10-ui-layer--rust-eguieframe)
11. [UI Layer — Python GTK 4](#11-ui-layer--python-gtk-4)
12. [UI Layer — Python Flask/Web](#12-ui-layer--python-flaskweb)
13. [End-to-End Data Flow](#13-end-to-end-data-flow)
14. [Python vs. Rust — Remaining Differences](#14-python-vs-rust--remaining-differences)

---

## 1. System Overview

dict-ai-te is a **live dictation and live speech-translation** application. The user presses a button, speaks into the microphone, and sees transcribed (and optionally translated) text accumulate in real time. When done, the transcript can be saved, copied, or read aloud via TTS.

All three implementations share the same external dependency: the **OpenAI Realtime API**, a WebSocket endpoint that accepts raw PCM16 audio and streams back transcription and translation events as JSON. There is no local speech recognition model; the entire STT and translation pipeline runs on OpenAI's infrastructure.

```
┌─────────────────────────────────────────────────────┐
│              dict-ai-te application                 │
│                                                     │
│  Microphone ──► Audio pipeline ──► Base64 PCM16    │
│                                         │           │
│                              WebSocket (OpenAI)     │
│                                         │           │
│                          JSON events ◄──┘           │
│                               │                     │
│                    Event parser / normalizer         │
│                               │                     │
│                    Transcript assembler              │
│                               │                     │
│                         UI / display                │
└─────────────────────────────────────────────────────┘
```

The app runs in two **session modes**, selected before recording starts:

- **Transcription** — connects to `wss://api.openai.com/v1/realtime?intent=transcription` using `gpt-4o-transcribe`. Returns source-language transcript deltas.
- **Translation** — connects to `wss://api.openai.com/v1/realtime?model=gpt-realtime` using `gpt-realtime`. Returns translated text deltas. Both the source transcript and the translation accumulate simultaneously.

---

## 2. Repository Layout

```
dict-ai-te/
├── src/                        ← Rust implementation
│   ├── main.rs                 ← entry point, egui/eframe bootstrap
│   ├── app.rs                  ← UI state machine (DictaiteApp)
│   ├── constants.rs            ← language list, voice list
│   ├── error.rs                ← AppError enum
│   ├── settings.rs             ← settings load/save (JSON + legacy TOML)
│   ├── openai.rs               ← blocking TTS HTTP client
│   ├── text_utils.rs           ← whitespace normalizer
│   └── audio/
│       ├── mod.rs
│       ├── clip.rs             ← AudioClip (in-memory decoded audio)
│       ├── live_capture.rs     ← microphone capture + PCM pipeline
│       ├── player.rs           ← rodio-based audio output
│       └── recorder.rs         ← (unused; kept for reference)
│   └── realtime/
│       ├── mod.rs
│       ├── audio.rs            ← PCM conversion helpers
│       ├── events.rs           ← JSON event parser
│       ├── state.rs            ← LiveState enum
│       ├── transcript.rs       ← TranscriptAssembler
│       └── transport.rs        ← WebSocket session functions
│
├── dictaite_core/              ← Python shared library
│   ├── __init__.py             ← re-exports: Settings, load_settings, synthesize_speech
│   ├── config.py               ← Settings dataclass + load/save
│   ├── realtime/
│   │   ├── __init__.py
│   │   ├── audio.py            ← PCM conversion helpers (mirrors realtime/audio.rs)
│   │   ├── events.py           ← JSON event parser (mirrors realtime/events.rs)
│   │   ├── transcript.py       ← TranscriptAssembler (mirrors realtime/transcript.rs)
│   │   └── transport.py        ← async WebSocket client (mirrors realtime/transport.rs)
│   └── services/
│       ├── __init__.py
│       ├── _client.py          ← OpenAI SDK client factory
│       ├── text_utils.py       ← whitespace normalizer (mirrors text_utils.rs)
│       └── tts.py              ← TTS via OpenAI SDK (mirrors openai.rs TTS path)
│
├── dictaite/                   ← Python UI packages
│   ├── __init__.py
│   ├── __main__.py
│   ├── api.py
│   ├── config.py
│   ├── ui_common.py            ← shared language/voice lists (mirrors constants.rs)
│   ├── ui_gtk/
│   │   ├── app.py              ← GTK 4 main window (mirrors app.rs)
│   │   └── live.py             ← GtkLiveSession (audio capture adapter)
│   └── ui_web/
│       └── app.py              ← Flask app with WebSocket routes
│
├── Cargo.toml                  ← Rust crate manifest
└── pyproject.toml              ← Python project manifest
```

---

## 3. The Three Implementations at a Glance

| Aspect | Rust (egui) | Python GTK 4 | Python Flask/web |
|---|---|---|---|
| UI framework | egui + eframe (immediate mode) | GTK 4 via PyGObject | Flask + flask-sock + TailwindCSS |
| Audio capture | cpal (via `LiveCapture`) | sounddevice (via `GtkLiveSession`) | Browser MediaRecorder → WebSocket |
| Realtime session | tokio async (native WebSocket) | asyncio (websockets library) | asyncio (websockets library) |
| TTS | reqwest blocking HTTP | OpenAI Python SDK | OpenAI Python SDK |
| Playback | rodio (`AudioPlayer`) | sounddevice + soundfile | Browser `<audio>` element |
| Settings file | `~/.dictaite/settings.json` | `~/.dictaite/settings.json` | `~/.dictaite/settings.json` |
| Translation | ✅ live (Realtime API) | ✅ live (Realtime API) | ✅ live (Realtime API) |
| Transcription | ✅ live | ✅ live | ✅ live |

All three read from and write to the **same settings file**, so switching between them preserves your language and voice preferences.

---

## 4. Audio Pipeline

### 4.1 Rust — `src/audio/live_capture.rs`

The Rust audio pipeline is fully threaded. cpal owns a platform audio stream on one thread; a separate worker thread does conversion and sends chunks to the async Tokio runtime.

```
cpal stream callback (audio thread)
        │
        │  []f32 samples (native sample rate & channel count)
        ▼
  mpsc::SyncSender<Vec<f32>>  (bounded, capacity = 8)
        │
        ▼
audio_worker thread
  1. downmix_to_mono(&samples, channels)
  2. resample_linear(&mono, native_rate → 24 000 Hz)
  3. accumulate in 'pending' buffer
  4. when pending ≥ chunk_samples (40 ms worth):
       pcm16_le(&pending) → raw i16 bytes
       chunk_pcm16(bytes, 24_000, 40 ms) → Vec<Vec<u8>>
       base64_pcm16(chunk) → String
       tokio_mpsc::Sender<String>.blocking_send(chunk)
        │
        ▼
  tokio async runtime
  (sends String chunks to the WebSocket session)
```

Level metering is done atomically: the cpal callback stores `f32::to_bits(max_amplitude)` in an `Arc<AtomicU32>`, which the UI reads on each repaint frame without locking.

Audio format negotiation (`choose_input_config`):
1. Prefer mono at exactly 24 000 Hz.
2. Fall back to any channel count at exactly 24 000 Hz.
3. Fall back to mono at the device's maximum rate.
4. Fall back to any configuration.

### 4.2 Python GTK — `dictaite/ui_gtk/live.py`

```python
sounddevice.InputStream(samplerate=24_000, channels=1, callback=_on_audio)
        │
        │  numpy float32 frames already at 24 kHz mono
        ▼
_on_audio callback:
  normalize_audio(indata, 24_000)  # resamples if needed (usually a no-op at 24 kHz)
  float_samples_to_pcm16(mono)     # → bytes
  base64_pcm16(pcm)                # → str
  queue.Queue.put_nowait(chunk)    # bounded, capacity = 8; drops on full
        │
        ▼
asyncio event loop (worker thread via asyncio.run)
  async generator yields chunks from queue via asyncio.to_thread
  OpenAIRealtimeClient.run(chunks, emit)
```

The GTK adapter captures directly at 24 kHz mono, so resampling is usually a no-op. Level metering is done inside the GTK audio callback and pushed to a `Gtk.LevelBar` via `GLib.idle_add`.

### 4.3 Python Flask/Web — browser side

The web client uses the browser's `MediaRecorder` API. Audio is captured in the browser, converted to mono PCM16 at 24 kHz in JavaScript, chunked, base64-encoded, and sent to the server over a WebSocket:

```
Browser
  MediaRecorder / AudioContext
        │  raw PCM32 float samples
        ▼
  downsample to 24 kHz (linear interpolation in JS)
  float → i16 (32768.0 / 32767.0 split — matches Rust)
  base64-encode
  WebSocket.send(JSON { type: "audio", audio: "<base64>" })
        │
        ▼
Flask server (_run_live_socket)
  forwards audio chunks → OpenAIRealtimeClient
```

The server never touches the audio samples directly; it acts as a trusted relay that forwards base64 PCM strings to the OpenAI WebSocket and normalised events back to the browser.

---

## 5. OpenAI Realtime Sessions

Both Rust and Python implement two session functions with identical semantics.

### 5.1 Transcription session

**Endpoint**: `wss://api.openai.com/v1/realtime?intent=transcription`  
**Model**: `gpt-4o-transcribe`  
**Auth**: `Authorization: Bearer <key>` header only (no legacy `OpenAI-Beta` header)

Session update sent immediately after connect:

```json
{
  "type": "session.update",
  "session": {
    "type": "transcription",
    "audio": {
      "input": {
        "format": { "type": "audio/pcm", "rate": 24000 },
        "transcription": { "model": "gpt-4o-transcribe" },
        "turn_detection": {
          "type": "server_vad",
          "threshold": 0.5,
          "prefix_padding_ms": 300,
          "silence_duration_ms": 500
        }
      }
    }
  }
}
```

`source_language` is added under `transcription.language` when the user selects a specific language.

Audio chunks are sent as:
```json
{ "type": "input_audio_buffer.append", "audio": "<base64-pcm16>" }
```

On stop, a commit message is sent and the socket closed:
```json
{ "type": "input_audio_buffer.commit" }
```

Lifecycle events emitted by the session function (not by the server):
- `SESSION_STATE { state: "connecting" }` — after sending the session update
- `SESSION_STATE { state: "disconnected" }` — after the socket closes

### 5.2 Translation session

**Endpoint**: `wss://api.openai.com/v1/realtime?model=gpt-realtime`  
**Model**: `gpt-realtime`

Session update:

```json
{
  "type": "session.update",
  "session": {
    "type": "realtime",
    "model": "gpt-realtime",
    "output_modalities": ["text"],
    "instructions": "You are a live speech translation engine. Translate the user's speech into <target>. Return only the translated text. Do not answer questions, add commentary, summarize, or describe the audio.",
    "audio": {
      "input": {
        "format": { "type": "audio/pcm", "rate": 24000 },
        "transcription": { "model": "gpt-4o-transcribe" },
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
}
```

The `target_language` is the **full language name** (e.g. `"French"`, `"Deutsch (German)"`), not an ISO code. This is the same string that appears in the UI language picker.

### 5.3 Rust implementation — `src/realtime/transport.rs`

```rust
pub async fn run_live_transcription(...) { run_verified_transcription_session(...).await }
pub async fn run_live_translation(...) { run_verified_translation_session(...).await }
```

Each function:
1. Builds the WebSocket request with the Authorization header.
2. Calls `tokio_tungstenite::connect_async(request)`.
3. Splits the socket into `write` + `read` halves.
4. Sends the session update JSON.
5. Emits `RealtimeEvent::SessionState { state: "connecting" }` via `event_tx`.
6. Enters a `tokio::select!` loop:
   - `stop_rx` fires → commit + close.
   - `audio_rx.recv()` → send `input_audio_buffer.append` message.
   - `read.next()` → parse JSON, filter events, forward via `event_tx`.
7. Emits `RealtimeEvent::SessionState { state: "disconnected" }`.

The translation session additionally filters for `TranslationDelta` and `TranslatedAudioDelta` events (not present in the transcription filter).

### 5.4 Python implementation — `dictaite_core/realtime/transport.py`

```python
class OpenAIRealtimeClient:
    async def run(self, audio_chunks, on_event): ...
    async def run_translation(self, audio_chunks, on_event): ...
```

Mirrors the Rust session functions using `websockets.connect` and `asyncio`. `run` handles transcription; `run_translation` handles translation. Both emit `NormalizedEvent(SESSION_STATE, state="connecting")` after the session update and `NormalizedEvent(SESSION_STATE, state="disconnected")` after close.

**Key difference from earlier versions**: the `LiveMode.TRANSLATE` branch no longer raises `RealtimeClientError`. Both modes are fully implemented.

---

## 6. Event Parsing and Normalization

The OpenAI Realtime API sends many event types. Both implementations normalize the raw JSON into a small, stable set of variants.

### 6.1 Rust — `src/realtime/events.rs`

```rust
pub enum RealtimeEvent {
    SourceDelta      { item_id: Option<String>, text: String },
    SourceCompleted  { item_id: Option<String>, text: String },
    TranslationDelta { text: String },
    TranslatedAudioDelta,
    SessionState     { state: String },
    Error            { message: String },
    Unknown          { event_type: Option<String> },
}
```

Mapping table:

| Raw `type` field | Variant |
|---|---|
| `conversation.item.input_audio_transcription.delta` | `SourceDelta` |
| `conversation.item.input_audio_transcription.completed` | `SourceCompleted` |
| `session.input_transcript.delta` | `SourceDelta` |
| `session.output_transcript.delta` | `TranslationDelta` |
| `response.output_text.delta` | `TranslationDelta` |
| `response.output_audio_transcript.delta` | `TranslationDelta` |
| `session.output_audio.delta` | `TranslatedAudioDelta` |
| `response.audio.delta` | `TranslatedAudioDelta` |
| `response.output_audio.delta` | `TranslatedAudioDelta` |
| `error` | `Error` |
| any `session.*` or `response.*` not matched above | `SessionState` |
| anything else | `Unknown` |

### 6.2 Python — `dictaite_core/realtime/events.py`

```python
class RealtimeEventType(StrEnum):
    SOURCE_DELTA = "source_delta"
    SOURCE_COMPLETED = "source_completed"
    TRANSLATION_DELTA = "translation_delta"
    TRANSLATED_AUDIO_DELTA = "translated_audio_delta"
    SESSION_STATE = "session_state"
    ERROR = "error"
    UNKNOWN = "unknown"

@dataclass(frozen=True, slots=True)
class NormalizedEvent:
    type: RealtimeEventType
    text: str = ""
    item_id: str | None = None
    state: str | None = None
    error: str | None = None
```

`parse_realtime_event(payload)` implements the same mapping table as Rust. The `SESSION_STATE` catch-all matches any `event_type` that starts with `"session."` or `"response."` — identical to the Rust `other if other.starts_with("session.") || other.starts_with("response.")` arm.

The Python representation is slightly richer than Rust (it carries `item_id`, `state`, and `error` as optional string fields in a single struct), which simplifies the web bridge that serialises events back to the browser as JSON.

---

## 7. Transcript Assembly

Both implementations maintain a segment-based transcript that handles out-of-order deltas and completions, as well as anonymous (item-id-less) text fragments.

### 7.1 Rust — `src/realtime/transcript.rs`

```rust
pub struct TranscriptAssembler {
    order: Vec<String>,           // insertion order of item_ids
    segments: BTreeMap<String, Segment>,
    anonymous: Vec<String>,
}

struct Segment { text: String, final_text: bool }
```

- `add_delta(item_id, text)` — appends text to a segment's buffer unless it is already finalised. Anonymous deltas are pushed to `anonymous`.
- `complete(item_id, text)` — replaces the segment's text with the final text and marks it final. Anonymous completions are pushed unconditionally.
- `text()` — joins all non-empty segment texts in insertion order, followed by anonymous fragments, space-separated.

Note: `BTreeMap` maintains key order but `order: Vec` is kept separately to preserve **insertion order** (the order segments first appeared), since BTreeMap orders by key alphabetically.

### 7.2 Python — `dictaite_core/realtime/transcript.py`

```python
class TranscriptAssembler:
    _segments: OrderedDict[str, _Segment]   # insertion-ordered
    _anonymous: list[str]
```

Functionally identical to Rust. `OrderedDict` provides insertion-order iteration natively, removing the need for a separate order list.

`complete` appends anonymous completions unconditionally (no deduplication), matching Rust exactly.

---

## 8. Text-to-Speech and Playback

### 8.1 Rust — `src/openai.rs` + `src/audio/`

TTS uses a blocking `reqwest::Client`:

```
OpenAiClient.text_to_speech(text, voice)
  POST /v1/audio/speech
  { model: "tts-1", input: text, voice: voice, response_format: "mp3" }
        │
        ▼
  response bytes (MP3 normally, JSON fallback with b64_json)
        │
        ▼
  decode_tts_json  (if Content-Type is JSON)
  OR
  raw bytes        (MP3/WAV binary)
        │
        ▼
  AudioClip::from_wav_bytes(bytes)
    → hound WavReader (primary)
    → rodio Decoder   (fallback)
        │
        ▼
  AudioPlayer.play(clip)
    → rodio Sink
```

`AudioClip` caches its WAV bytes lazily via `Arc<Vec<u8>>`. If the same clip is played a second time with the same voice, `DictaiteApp.play_transcript_audio` skips the TTS call and re-uses the cached `AudioClip`.

Level feedback during playback comes from `AudioClip.level_at(elapsed)`, which scans a 120 ms window around the current playback position and returns the peak amplitude. This drives the `ProgressBar` in the UI on every repaint.

`BackgroundTask<T>` is a tiny wrapper around `std::thread::spawn` + `mpsc::channel` that lets the UI thread poll for a result on each frame without blocking.

### 8.2 Python GTK — `dictaite_core/services/tts.py` + `dictaite/ui_gtk/app.py`

```python
synthesize_speech(text, voice)
  client.audio.speech.create(model="tts-1", voice=voice, response_format="wav")
        │ WAV bytes
        ▼
  soundfile.read(BytesIO(wav_bytes))  → (float32 samples, sample_rate)
        │
        ▼
  sounddevice.OutputStream (callback-based playback)
  level feedback via GLib.idle_add(level_bar.set_value, amplitude)
```

TTS is run in a `threading.Thread` to avoid blocking the GTK main loop.

### 8.3 Python Flask/Web

TTS is requested by the browser via `POST /api/tts-test`, which returns `audio/wav` bytes. The browser plays the response using a standard `<audio>` element. The server-side call is identical to the GTK path.

---

## 9. Settings and Configuration

### 9.1 Shared format

All three implementations use the same on-disk format:

**File**: `~/.dictaite/settings.json`

```json
{
  "default_language": null,
  "translate_by_default": false,
  "default_target_language": "en",
  "female_voice": "nova",
  "male_voice": "onyx"
}
```

Voice names are stored **lowercase**. `default_language` is `null` for auto-detect; `default_target_language` defaults to `"en"`.

A legacy TOML file at `~/.config/dict-ai-te/dict-ai-te_config.toml` is transparently migrated to the JSON format on first load and left in place (not deleted).

The `DICTAITE_HOME` environment variable overrides `~/.dictaite` for the config directory.

### 9.2 Rust — `src/settings.rs`

```rust
pub struct Settings {
    pub default_language: Option<String>,
    pub translate_by_default: bool,
    pub default_target_language: Option<String>,
    pub female_voice: String,
    pub male_voice: String,
}
```

`load_settings()` calls `load_settings_from_path(None)`, which:
1. Reads `settings.json`.
2. If missing, attempts to parse the legacy TOML.
3. Applies `fill_defaults` (normalises voice names to lowercase, fills blanks with defaults).
4. If JSON is present but a field is `"default"` or `""`, `deserialize_optional_lang` returns `None`.

`save_settings_to_path` serialises with `serde_json::to_string_pretty`.

### 9.3 Python — `dictaite_core/config.py`

```python
@dataclass(slots=True)
class Settings:
    default_language: str | None = None
    translate_by_default: bool = False
    default_target_language: str | None = "en"
    female_voice: str = "nova"
    male_voice: str = "onyx"
```

`load_settings()` mirrors the Rust function: read JSON, fall back to TOML migration, apply `fill_defaults`. The Python `fill_defaults` normalises voice names to lowercase and strips whitespace, mirroring `src/settings.rs:fill_defaults`.

`save_settings` serialises with `json.dumps(indent=2, sort_keys=True)`.

---

## 10. UI Layer — Rust (egui/eframe)

### Overview

The Rust UI is an **immediate-mode** application: every frame the entire UI is re-evaluated from `DictaiteApp.update()`. There is no retained widget tree; instead the app struct holds all state explicitly.

### `DictaiteApp` struct (`src/app.rs`)

Key state fields and their purpose:

| Field | Type | Purpose |
|---|---|---|
| `is_recording` | `bool` | guards `start_recording` / `stop_recording` |
| `live_capture` | `Option<LiveCapture>` | owns the cpal stream + worker thread |
| `live_runtime` | `Option<tokio::Runtime>` | Tokio runtime for WebSocket async tasks |
| `live_event_tx/rx` | `mpsc::Sender/Receiver<RealtimeEvent>` | bridge from async tasks to UI thread |
| `live_stop_tx` | `Option<oneshot::Sender<()>>` | signals the WebSocket session to close |
| `live_state` | `LiveState` | Disconnected / Transcribing / Translating / Error |
| `source_assembler` | `TranscriptAssembler` | builds the source transcript from deltas |
| `source_transcript` | `String` | displayed in the source pane |
| `translated_transcript` | `String` | displayed in the translated pane (translation mode) |
| `transcript` | `String` | "active" transcript for TTS/copy/save |
| `raw_transcript` | `Option<String>` | original source, used to restore when translation is toggled off |
| `tts_task` | `Option<BackgroundTask<TtsOutcome>>` | non-blocking TTS fetch |
| `tts_clip` | `Option<AudioClip>` | last synthesised clip (for replay caching) |
| `player` | `Option<AudioPlayer>` | audio output |
| `settings_modal` | `Option<SettingsModal>` | in-frame settings window |

### Recording start/stop

`start_recording`:
1. Resets all transcript and clip state.
2. Creates a `tokio::sync::mpsc::channel` for audio chunks and a `oneshot` stop channel.
3. Spawns `run_live_transcription` or `run_live_translation` on the Tokio runtime.
4. Calls `LiveCapture::start(audio_tx, event_tx)` — starts cpal.
5. Sets `is_recording = true` and updates status text.

`stop_recording`:
1. Sets `is_recording = false`.
2. Calls `capture.stop()` (drops cpal stream, joins worker thread).
3. Sends on `live_stop_tx` (signals WebSocket to close).

### Event loop

`poll_live_events(ctx)` is called every frame. It drains `live_event_rx` using `try_recv` in a loop:

```
SourceDelta     → source_assembler.add_delta → update source_transcript + transcript
SourceCompleted → source_assembler.complete → update source_transcript + transcript
TranslationDelta→ append to translated_transcript + set transcript = translated_transcript
SessionState    → update status_text; on "disconnected" → stop recording state
Error           → set error_text; stop recording state
Unknown         → ignore
```

After draining, it checks `live_capture.take_error()` for microphone errors.

### Settings modal

`SettingsModal` is an egui `Window` rendered inside `update()`. It carries its own temporary indices for language and voice combos. On "Save", `persist()` writes new values to `app.settings` and calls `save_settings`.

### TTS caching

Before calling TTS, `play_transcript_audio` checks:
1. Is `tts_clip` Some and `tts_voice_id` eq the current `voice_id`?
2. If yes, replay `tts_clip` directly through `player`.
3. If no, launch a `BackgroundTask` to call `OpenAiClient.text_to_speech`.

`poll_tts(ctx)` is called every frame. When the task completes, `AudioClip` is stored in `tts_clip` and played.

### Layout

```
TopPanel: "dict-ai-te"  [Settings ▶]

CentralPanel:
  ProgressBar (level meter)
  [Start/Stop Listening]  status  timer
  Origin language ▾
  ☐ Translate Live
  (if translate_enabled):
    Target language ▾
  Source transcript (TextEdit, multiline)
  (if translate_enabled):
    Translated transcript (TextEdit, multiline)

BottomPanel:
  [⬇ Save] [⧉ Copy] [▶ Play / ■ Stop]  ○ Female  ○ Male
  (error text | copy feedback)
```

---

## 11. UI Layer — Python GTK 4

### Overview

The Python GTK UI is a retained-mode application built with GTK 4 via PyGObject. `DictaiTeWindow` subclasses `Gtk.ApplicationWindow`. Long-running operations (audio playback, settings preview) run in daemon threads that post results back to GTK's main loop via `GLib.idle_add`.

### Key differences from Rust

**No TTS caching.** Every "Play" button press calls `synthesize_speech` and generates a new audio file. There is no `tts_clip` cache.

**Level metering.** The GTK level bar is updated by the audio callback via `GLib.idle_add(level.set_value, amplitude)`. Rust uses an atomic float updated in the cpal callback and read by the UI repaint loop.

**Timer.** A background `threading.Thread` wakes every second to update the timer label via `GLib.idle_add`. Rust computes elapsed time from `Instant::now()` every repaint frame.

**Voice gender toggle.** GTK uses `Gtk.CheckButton` radio group. Rust uses `ui.radio_value`.

**Live session adapter.** `GtkLiveSession` owns a `sounddevice.InputStream`, an `asyncio` event loop (in a worker thread), and a `queue.Queue` bridging the two. The async client reads from the queue via `asyncio.to_thread(queue.get)`.

### Window layout (mirrors Rust)

```
Header bar: [Settings]
LevelBar
[Mic icon / Stop icon] status_label timer_label
Origin language ▾   ○ Translate to  Target language ▾
Source transcript (scrolled TextView)
Translated transcript (scrolled TextView, hidden when translate off)
[⬇ Save] [⧉ Copy] [▶ Play]  ○ Female  ○ Male
```

### Event handling

`on_live_event(event)` is called via `GLib.idle_add` from the worker thread:

```python
SOURCE_DELTA / SOURCE_COMPLETED → source_assembler.apply(event) → text_view buffer
TRANSLATION_DELTA               → translated_text += event.text → translated_text_view buffer
SESSION_STATE                   → update status_label; on "disconnected" → stop state
ERROR                           → show_error dialog
```

---

## 12. UI Layer — Python Flask/Web

### Overview

The Flask app is a thin server. It owns the OpenAI API key, proxies audio from the browser to OpenAI Realtime, and streams normalised events back. All audio capture and playback happen in the browser.

### Routes

| Route | Method | Description |
|---|---|---|
| `/` | GET | Renders `index.html` — the main app page |
| `/settings` | GET | Renders `settings.html` — settings form |
| `/api/settings` | GET / POST | Read / write settings JSON |
| `/api/health` | GET | Readiness probe |
| `/api/tts-test` | POST | TTS preview: `{ gender, text, voice? }` → `audio/wav` |
| `/ws/live/transcribe` | WebSocket | Live transcription session |
| `/ws/live/translate` | WebSocket | Live translation session |

### WebSocket session flow

```
Browser                          Flask server                       OpenAI Realtime
  │── WS connect ────────────────►│                                        │
  │── { type:"start",             │                                        │
  │    source_language,           │                                        │
  │    target_language } ────────►│                                        │
  │                               │── WS connect ─────────────────────────►│
  │                               │── session.update ──────────────────────►│
  │                               │◄── { type:"session.created" } ─────────│
  │◄── { type:"session_state",   │                                        │
  │     state:"connecting" } ─────│                                        │
  │── { type:"audio",            │── input_audio_buffer.append ──────────►│
  │    audio:"<base64>" } ───────►│                                        │
  │                               │◄── transcript delta ────────────────────│
  │◄── { type:"source_delta",    │                                        │
  │     text:"..." } ─────────────│                                        │
  │── { type:"stop" } ───────────►│── input_audio_buffer.commit ──────────►│
  │                               │── close ───────────────────────────────►│
  │◄── { type:"session_state",   │                                        │
  │     state:"disconnected" } ───│                                        │
```

### `_run_live_socket` implementation

```python
def _run_live_socket(ws, mode):
    # 1. Read start message for language params
    first = ws.receive()
    payload = json.loads(first)
    target_language = _normalize_language(payload.get("target_language"))
    source_language = _normalize_language(payload.get("source_language"))

    # 2. Create async generator that yields browser audio messages
    async def incoming():
        while True:
            message = ws.receive()
            if message is None: break
            yield json.loads(message)

    # 3. Create emit callback that forwards normalised events to browser
    async def send_event(payload):
        ws.send(json.dumps(payload))

    # 4. Run the realtime client
    client = OpenAIRealtimeClient(RealtimeClientConfig(
        mode=mode, target_language=target_language, source_language=source_language
    ))
    asyncio.run(client.run(audio_iter_from(incoming()), send_event))
```

Events are forwarded to the browser as JSON with the fields `{ type, text, item_id, state, error }`, matching the `NormalizedEvent` dataclass fields.

### Settings page

Settings are stored in `~/.dictaite/settings.json` (shared with GTK and Rust). The web UI reads them on startup into `current_app.config["DICTAITE_SETTINGS"]` and updates the in-memory copy on `POST /api/settings`. Changes are persisted to disk immediately.

The settings page lists the same voice and language options as the GTK app (`ui_common.py` mirrors `src/constants.rs`).

---

## 13. End-to-End Data Flow

### Transcription mode (all implementations)

```
Microphone samples
    │ (float32, native rate, native channels)
    ▼
downmix to mono
    │
resample to 24 000 Hz
    │
convert to PCM16 little-endian (i16 bytes)
    │
split into ~40 ms chunks
    │
base64-encode each chunk
    │
WebSocket: input_audio_buffer.append { audio: "<base64>" }
    │
[OpenAI server performs VAD + STT]
    │
conversation.item.input_audio_transcription.delta { delta: "word " }
    │
parse_event → SourceDelta { item_id, text }
    │
TranscriptAssembler.add_delta(item_id, text)
    │
assembler.text() → "Hello world"
    │
UI: source transcript pane updated
```

On voice-activity end, OpenAI sends `conversation.item.input_audio_transcription.completed` with the final transcript for the segment. The assembler replaces the segment text and marks it final.

### Translation mode (additional events)

The translation endpoint additionally produces:

```
response.output_text.delta { delta: "Hola " }
    │
parse_event → TranslationDelta { text: "Hola " }
    │
UI: translated_transcript += "Hola "
    │
translated transcript pane updated
```

### TTS and playback

```
User clicks Play
    │
transcript_for_actions() → text string
    │
[if cached clip + same voice → replay directly]
    │
POST /v1/audio/speech { model: "tts-1", voice, input: text }
    │
MP3/WAV bytes
    │
AudioClip::from_wav_bytes (Rust)  /  soundfile.read (Python)
    │
AudioPlayer.play(clip) / sounddevice.OutputStream
    │
level meter updated per frame / per callback
```

---

## 14. Python vs. Rust — Remaining Differences

After the `sync-python-to-rust-parity` change the implementations are closely aligned. The remaining differences are implementation-level — the behaviour is identical from the user's perspective.

| Area | Rust | Python | Notes |
|---|---|---|---|
| **Audio capture library** | cpal (multi-format native) | sounddevice (PortAudio wrapper) | cpal negotiates format/rate natively; sounddevice always opens at 24 kHz mono |
| **Async runtime** | Tokio (multi-thread) | asyncio (single-thread event loop per session) | Tokio runs on a dedicated `dictaite-realtime` thread pool; Python uses `asyncio.run` in a daemon thread |
| **WebSocket library** | tokio-tungstenite | websockets (Python) | Both use TLS; both support the same Authorization header |
| **TTS HTTP client** | reqwest blocking | OpenAI Python SDK | Both call the same `/v1/audio/speech` endpoint with `tts-1` |
| **Playback** | rodio (via AudioClip + AudioPlayer) | sounddevice (GTK) / `<audio>` element (web) | Rust caches the decoded clip; GTK re-decodes on each play |
| **UI paradigm** | Immediate mode (egui) | Retained mode (GTK 4 / Flask+HTML) | No practical behaviour difference |
| **Level meter source during recording** | `AtomicU32` in cpal callback | `GLib.idle_add` from sounddevice callback | Both show peak amplitude in the level bar |
| **Level meter source during playback** | `AudioClip.level_at(elapsed)` (pre-computed) | sounddevice callback peak | Rust level reflects what is actually playing; Python reflects the live output |
| **TTS clip caching** | Yes (`tts_clip` + `tts_voice_id`) | No (every Play re-fetches) | Flask: browser caches the last WAV via standard HTTP semantics |
| **`text_utils.format_structured_text`** | `src/text_utils.rs` | `dictaite_core/services/text_utils.py` | Used for TTS output cleaning; identical algorithm |
| **Error type** | `AppError` enum (thiserror) | Python exceptions / error strings | Python passes errors as strings through the event system to the UI |
| **Voice name casing** | lowercase internally | lowercase internally (normalised on load) | Both read from the same settings JSON; fill_defaults normalises on load |
