## 1. Shared Python Realtime Core

- [x] 1.1 Create `dictaite_core/realtime` module structure with models, events, audio, transcript assembly, and OpenAI transport modules.
- [x] 1.2 Implement realtime settings types and migration mapping from `translate_by_default`, `default_target_language`, and `default_language`.
- [x] 1.3 Implement PCM audio helpers for mono conversion, 24 kHz resampling, PCM16 encoding, chunking, and base64 conversion.
- [x] 1.4 Implement normalized event parsing for transcription delta/completed events and translation input/output/audio delta events.
- [x] 1.5 Implement source transcript segment assembly keyed by `item_id`, including out-of-order completions and duplicate prevention.
- [x] 1.6 Implement async OpenAI WebSocket clients for realtime transcription and translation with safe close and minimal logging.

## 2. Python Web UI

- [x] 2.1 Choose and add a Flask-compatible WebSocket server dependency or ASGI migration path.
- [x] 2.2 Add `/ws/live/transcribe` and `/ws/live/translate` server WebSocket endpoints that proxy browser audio to OpenAI.
- [x] 2.3 Replace active `MediaRecorder` upload JavaScript with microphone capture through `AudioContext`/`AudioWorklet` and PCM chunk streaming.
- [x] 2.4 Replace the web UI mode selector and recording/upload statuses with Start Listening/Stop, Translate Live, target language, connection state, and transcript panes.
- [x] 2.5 Remove `/api/transcribe` and `/api/record/finalize` from the active browser flow while keeping temporary compatibility only if needed.
- [x] 2.6 Preserve browser copy, save, clear, and editable transcript behavior for source and translated text.

## 3. Python GTK UI

- [x] 3.1 Replace record-complete controls with Start Listening/Stop, Translate Live, target language, connection state, and transcript editors.
- [x] 3.2 Feed microphone PCM chunks into an async realtime queue when listening starts.
- [x] 3.3 Run the realtime client on a background asyncio loop and dispatch normalized events to GTK.
- [x] 3.4 Ensure GTK widgets are updated only on the GTK main thread.
- [x] 3.5 Remove user-facing uploading/post-recording transcription phases.

## 4. Rust Realtime Implementation

- [x] 4.1 Add `tokio`, `tokio-tungstenite`, `futures-util`, and a selected audio resampling dependency to `Cargo.toml`.
- [x] 4.2 Add `src/realtime` module structure with models, audio conversion, OpenAI WebSocket transport, transcription, and translation modules.
- [x] 4.3 Implement non-blocking `cpal` microphone capture using bounded channels and worker-side PCM conversion/chunking.
- [x] 4.4 Implement `run_live_transcription` with verified OpenAI Realtime transcription contract, transcription event parsing, and UI event forwarding.
- [ ] 4.5 Implement `run_live_translation` with `gpt-realtime-translate`, translation event parsing, ignored audio deltas, and UI event forwarding.
- [x] 4.6 Replace Rust egui recording controls with live-first controls and transcript panes.
- [x] 4.7 Preserve Rust copy, save, clear, and editable transcript behavior.

## 5. Testing

- [x] 5.1 Add Python unit tests for PCM conversion, chunking, event parsing, unknown event safety, and transcript assembly.
- [x] 5.2 Add Python web tests for WebSocket message normalization and mocked OpenAI realtime traffic.
- [ ] 5.3 Add Python GTK-adjacent tests or isolated event-dispatch tests for state transitions where practical.
- [x] 5.4 Add Rust tests for sample conversion, chunking, JSON event parsing, unknown event safety, duplicate prevention, and state transitions.
- [x] 5.5 Ensure standard automated tests do not call OpenAI.

## 6. Documentation and Cleanup

- [x] 6.1 Rewrite README live-first sections and remove batch transcription as the primary product behavior.
- [x] 6.2 Update architecture docs to describe realtime core modules and UI adapters.
- [ ] 6.3 Remove or quarantine obsolete batch recording/upload code after live paths are verified.
- [ ] 6.4 Manually test live transcription, live translation to Spanish, repeated stop/start, microphone unavailable, network disconnect, and invalid API key.
