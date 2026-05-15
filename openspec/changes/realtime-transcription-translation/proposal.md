## Why

The current app is built around a batch recording workflow: record audio, stop, upload/process the completed file, then show final transcription and optional translation. The desired product experience is live dictation and live translation, where text appears while the user is still speaking.

## What Changes

- **BREAKING**: Remove the user-facing classic record/stop/upload transcription workflow from Python web, Python GTK, and Rust egui UIs.
- Add live transcription as the default product behavior using OpenAI Realtime and `gpt-realtime-whisper`.
- Add optional live translation using OpenAI Realtime translation sessions and `gpt-realtime-translate`.
- Replace browser `MediaRecorder` upload flow with microphone PCM streaming over a server-owned WebSocket bridge.
- Add shared realtime models, audio conversion, OpenAI WebSocket clients, and event normalization under `dictaite_core`.
- Add Rust realtime modules, async WebSocket runtime support, and non-blocking microphone streaming.
- Preserve edit/copy/save workflows for accumulated source and translated text.
- Update settings and documentation around live-first behavior while migrating existing translation defaults safely.

## Capabilities

### New Capabilities

- `live-realtime-transcription`: Live microphone transcription with realtime transcript deltas, finalized segment reconciliation, and editable accumulated text.
- `live-realtime-translation`: Optional live speech translation with source and translated transcript streams from a dedicated realtime translation session.
- `realtime-audio-streaming`: Shared microphone audio capture, mono 24 kHz PCM16 conversion, chunking, WebSocket forwarding, and safe session shutdown.
- `live-first-ui`: User-facing Python web, Python GTK, and Rust egui interfaces centered on Start Listening/Stop, live translation toggle, connection status, and editable transcript panes.

### Modified Capabilities

- None. No existing OpenSpec capabilities are present.

## Impact

- Python core: add `dictaite_core/realtime` modules for models, audio conversion, OpenAI WebSocket transport, normalized events, and transcript assembly.
- Python web: replace active `/api/transcribe` and `/api/record/finalize` UI paths with WebSocket live streaming endpoints; add an explicit Flask-compatible WebSocket server dependency or chosen ASGI alternative.
- Python GTK: replace record-complete workflow with live listening controls and background realtime event handling on the GTK main thread.
- Rust: add `src/realtime` modules, Tokio runtime/WebSocket dependencies, PCM conversion/resampling, and egui live transcript state.
- Settings: migrate `translate_by_default`, `default_target_language`, and `default_language` into live settings without breaking existing user configuration.
- Tests: add mocked realtime event parsing, audio conversion/chunking, transcript assembly, WebSocket bridge, state transition, and unknown-event safety coverage.
- Docs: rewrite README and architecture notes to describe the live-first product and remove batch transcription as the primary behavior.
