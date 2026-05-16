# Architecture

The project is split into three major layers to enable multiple user interfaces to share the same business logic.

## Core layer (`dictaite_core`)

* **Configuration** – `dictaite_core.config.Settings` encapsulates shared preferences (default languages, voices, translation
  behaviour). `load_settings()` and `save_settings()` persist the JSON file at `~/.dictaite/settings.json`. Legacy TOML
  configuration is migrated automatically the first time the new code runs.
* **Services** – `dictaite_core.services` exposes pure functions:
  * `transcribe(audio: bytes, mimetype: str, language: str | None) -> str`
  * `translate(text: str, target_lang: str) -> str`
  * `synthesize_speech(text: str, voice: str | None) -> bytes`

  These helpers wrap the OpenAI SDK, hide MIME conversion, enforce limits (≤120 seconds, allowed MIME types) and return
  normalized text. They contain no GUI dependencies and can be reused from any environment.
* **Realtime** – `dictaite_core.realtime` contains live-first modules shared by UI adapters:
  * `audio` downmixes, resamples, encodes PCM16, chunks, and base64-encodes microphone audio.
  * `events` normalizes OpenAI realtime transcription and translation events.
  * `transcript` assembles source transcript segments by `item_id` to avoid duplicate partial/final text.
  * `transport` owns async WebSocket sessions for `gpt-realtime-whisper` and `gpt-realtime-translate`.
  * `settings` maps legacy translation defaults into live translation mode, source language, and target language.

## GTK UI (`dictaite.ui_gtk`)

The GTK4 desktop application imports realtime and service helpers from `dictaite_core`. Microphone capture is handled locally via
`sounddevice`; live sessions push PCM chunks into a bounded queue, run the realtime client on a background asyncio loop, and use
`GLib.idle_add` to update GTK widgets on the main thread. `SettingsDialog` reads/writes the shared settings via `load_settings()` /
`save_settings()` so the desktop and web front-ends remain in sync.

## Flask web UI (`dictaite.ui_web`)

The Flask app is built with the application-factory pattern. `dictaite.ui_web.app:create_app()` registers two blueprints and, when
`flask-sock` is installed, live WebSocket routes:

* `pages` for `GET /` (recorder) and `GET /settings`.
* `api` for JSON endpoints (`/api/transcribe`, `/api/tts-test`, `/api/settings`, `/api/health`).
* `/ws/live/transcribe` and `/ws/live/translate` for browser microphone PCM streaming.

The HTML lives in Jinja templates with TailwindCSS for layout. JavaScript (vanilla) handles `AudioContext` microphone capture,
24 kHz PCM16 chunk streaming, level metering, transcript editing, copy/download actions and TTS playback. Settings persist by
calling the shared configuration helpers.

## Rust realtime layer (`src/realtime`)

The Rust app has a parallel realtime module set under `src/realtime`:

* `audio` provides mono conversion, linear resampling, PCM16 encoding, chunking, and base64 helpers.
* `events` normalizes realtime JSON events and ignores unknown events safely.
* `transcript` assembles source transcript segments without duplicating partial and final text.
* `transport` contains the Tokio/tokio-tungstenite realtime WebSocket session runner.
* `state` models live connection state transitions for tests and UI integration.

The Rust egui app owns a Tokio runtime for live sessions and a separate `cpal` live capture path under `src/audio/live_capture.rs`.
The audio callback only pushes sample buffers into a bounded channel; worker-side code downmixes, resamples to 24 kHz, converts
to PCM16, chunks, and base64-encodes audio before forwarding it to the realtime WebSocket task. UI updates are delivered back to
egui through a standard channel polled from `App::update`.

As of 2026-05-16, the verified official OpenAI Realtime contract used by Rust is transcription-only:

* WebSocket endpoint: `wss://api.openai.com/v1/realtime?intent=transcription`
* Authentication header: `Authorization: Bearer <OPENAI_API_KEY>`
* Legacy beta header: do not send `OpenAI-Beta: realtime=v1`; it selects the retired beta API
* Session update event: `session.update`
* Session type: `transcription`
* Input audio: `audio.input.format = {"type": "audio/pcm", "rate": 24000}`
* Transcription model: `gpt-4o-transcribe`
* Audio append event: `input_audio_buffer.append`
* Transcript events: `conversation.item.input_audio_transcription.delta` and `conversation.item.input_audio_transcription.completed`

No current official documentation was found for the older `gpt-realtime-translate`, `/v1/realtime/translations`, or
`translation_session.update` assumptions. Rust live translation therefore reports a clear unavailable error instead of falling
back to chained text translation.

## Adding another UI

1. Import what you need from `dictaite_core` (`load_settings`, realtime helpers, `transcribe`, `translate`, `synthesize_speech`).
2. Keep the UI self-contained; never import `openai` directly.
3. Reuse `dictaite/ui_common.py` for language and voice lists to match the existing front-ends.
4. Persist settings using `save_settings()` so all experiences share the same defaults.
