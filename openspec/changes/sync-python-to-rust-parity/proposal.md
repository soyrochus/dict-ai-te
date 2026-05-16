## Why

The Rust implementation (egui native desktop) is the reference implementation and is ahead of the Python codebase in every dimension: it supports live translation, handles the full set of GA OpenAI Realtime API event types, has TTS playback with caching, and operates in a single live-first mode with no batch fallback. The Python GTK and Flask/web UIs have diverged significantly — they retain dead code paths (batch upload-then-transcribe, post-hoc chat-based translation, web-specific chunked recording sessions) that the Rust version deliberately omits, while also missing the working live translation and several GA event-type fixes. This change brings both Python UIs to a 100% behavioural clone of the Rust app.

## What Changes

- **GTK UI** (`dictaite/ui_gtk/`): rewrite the live session layer to support translation mode (currently raises an error); align event handling to match Rust's full GA event set; remove the leftover `transcribe_audio` batch-recording path; align TTS playback, copy feedback, and status text to match Rust.
- **Flask/web UI** (`dictaite/ui_web/`): remove all batch endpoints (`/api/transcribe`, `/api/record/*`); remove the `bridge_websocket_messages` proxy in favour of a clean WebSocket handler that mirrors the Rust session flow (connect, stream PCM, receive events, disconnect); align event forwarding and error handling.
- **`dictaite_core/realtime/events.py`**: add missing GA event type aliases (`response.output_text.delta`, `response.output_audio_transcript.delta`, `response.audio.delta`, `response.output_audio.delta`); broaden the `SESSION_STATE` catch-all to match any `session.*` or `response.*` prefix.
- **`dictaite_core/realtime/transport.py`**: implement `run_live_translation` (remove the placeholder error); add `"connecting"` and `"disconnected"` lifecycle `SESSION_STATE` events; remove `bridge_websocket_messages` (absorbed into the web UI layer).
- **`dictaite_core/realtime/transcript.py`**: remove the anonymous `complete` deduplication check (Rust does not deduplicate).
- **`dictaite_core/realtime/audio.py`**: align PCM16 negative-sample multiplier to `32768.0`; add chunk-size guard matching Rust; remove `chunks_as_base64` helper.
- **`dictaite_core/config.py`**: normalise voice defaults to lowercase (`"nova"`, `"onyx"`); apply `fill_defaults` normalisation on load.
- **Remove entirely**: `dictaite_core/services/stt.py`, `dictaite_core/services/translate.py`, `dictaite_core/realtime/settings.py` (migration helper no longer needed), `dictaite_core/realtime/models.py` (LiveMode replaced by a simpler bool). **BREAKING**: the `/api/transcribe` and `/api/record/*` REST endpoints are removed.

## Capabilities

### New Capabilities

- `live-translation`: Live speech-to-translation via the OpenAI Realtime GA WebSocket endpoint, available in both GTK and web UIs (mirroring the Rust `run_live_translation` implementation).
- `ga-realtime-events`: Full GA OpenAI Realtime event-type coverage in the Python event parser, including `response.*` transcript and audio delta variants and a broad `session.*`/`response.*` session-state catch-all.

### Modified Capabilities

- `realtime-transcription`: The transcription WebSocket session gains `"connecting"` and `"disconnected"` lifecycle events; the anonymous-segment deduplication is removed; the PCM16 encoding aligns to the Rust numerics.

## Impact

- **Removed code**: `dictaite_core/services/stt.py`, `dictaite_core/services/translate.py`, `dictaite_core/realtime/settings.py`, `dictaite_core/realtime/models.py`, `bridge_websocket_messages` in transport, all batch REST endpoints in the web UI.
- **Affected APIs**: The Flask web app loses its `/api/transcribe` and `/api/record/*` endpoints. Any external client calling those will break.
- **Dependencies**: `pydub`, `soundfile` (used only in batch STT path) can be removed from requirements. The `openai` Python SDK chat-completions call in `translate.py` is gone.
- **Settings format**: voice names are now stored lowercase; the JSON settings file remains backward-compatible (the `fill_defaults` normalisation handles old capitalised values on load).
