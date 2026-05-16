## Context

The Rust egui app is the canonical implementation: realtime-only, live-first, no batch fallback. Both Python UIs (GTK 4 and Flask/web) grew their own feature set independently and are now out of sync. The Rust codebase must not be changed. The Python `dictaite_core` library is the shared layer beneath both UIs and is where most of the alignment work sits.

Key structural differences today:
- Python `transport.py` raises an error for `LiveMode.TRANSLATE` instead of connecting to the translation endpoint.
- Python `events.py` only handles a subset of the GA OpenAI Realtime event types; the Rust parser handles `response.*` variants introduced in the GA API.
- The Flask web app has a parallel batch pipeline (`/api/transcribe`, `/api/record/*`) that the Rust app has no equivalent for.
- Python `services/stt.py` and `services/translate.py` exist purely to support the batch pipeline.
- The GTK UI has a `transcribe_audio` method that records locally then calls the batch STT service — not present in Rust.

## Goals / Non-Goals

**Goals:**
- Python GTK UI behavior is identical to the Rust egui UI: realtime recording starts immediately via OpenAI Realtime WebSocket, transcription and live translation both work, TTS playback uses OpenAI TTS, settings dialog is feature-identical.
- Python Flask web UI exposes the same live WebSocket routes as before (`/ws/live/transcribe`, `/ws/live/translate`) but with correct GA event handling and a clean session flow; all batch REST endpoints removed.
- `dictaite_core` realtime layer handles the full GA event surface: `response.output_text.delta`, `response.output_audio_transcript.delta`, `response.audio.delta`, `response.output_audio.delta`; `session.*`/`response.*` session-state catch-all.
- `run_live_translation` is implemented in Python transport, matching the Rust session config (target-language instruction, VAD settings, `create_response: true`).
- PCM16 conversion, resampling, and chunking numerics match Rust exactly.
- Voice defaults stored as lowercase in settings.

**Non-Goals:**
- Changing the Rust implementation in any way.
- Preserving backward compatibility of the removed batch REST endpoints.
- Adding any feature to Python that does not exist in Rust.
- Migrating the Flask web UI away from Flask or from WebSockets.

## Decisions

### D1 — Implement `run_live_translation` in transport.py by mirroring the Rust function exactly

The Rust `run_verified_translation_session` uses `wss://api.openai.com/v1/realtime?model=gpt-realtime`, sends a `session.update` with `type: "realtime"`, `output_modalities: ["text"]`, a translation instruction, and VAD with `create_response: true`. Python will do the same. The existing `OpenAIRealtimeClient.run` method will be extended or a parallel `run_translation` method added, gated by `LiveMode.TRANSLATE`.

**Alternative considered**: keep using the transcription endpoint and do post-hoc translation via the chat API. Rejected — this is the old batch approach and conflicts with the goal of being a 100% clone.

### D2 — Remove `LiveMode` enum and `RealtimeSettings` dataclass; use a plain `translate: bool` flag

The Rust app passes a `translate: bool` to distinguish modes internally. Python currently uses `LiveMode` (a `StrEnum`) and `RealtimeSettings`. Removing these simplifies the model layer and removes `models.py` and `settings.py` (realtime migration helpers), both of which are Python-only artifacts.

**Alternative considered**: keep `LiveMode` as a thin wrapper. Rejected — it adds indirection with no benefit now that the only two modes are transcribe and translate.

### D3 — Remove all batch services (`stt.py`, `translate.py`) and batch Flask endpoints

The Rust app has no batch pipeline. Removing these eliminates `pydub` and `soundfile` dependencies from the realtime path, reduces surface area, and removes the false impression that those paths are supported.

**Alternative considered**: keep batch endpoints for backwards compatibility. Rejected — the proposal explicitly states all Python features not in Rust must go.

### D4 — Align `events.py` to Rust by broadening the GA event catch-all

Instead of matching a fixed set of session-state event names, match any string starting with `session.` or `response.`. This mirrors `events.rs` exactly. Adding `response.output_text.delta`, `response.output_audio_transcript.delta`, `response.audio.delta`, and `response.output_audio.delta` to their respective variant handlers is a direct port of the Rust match arms.

### D5 — GTK UI: remove `transcribe_audio` batch path, align live session to match Rust

The GTK `DictaiTeWindow.transcribe_audio` method and all its call sites are deleted. The `GtkLiveSession` is updated to route translation mode through the new `run_live_translation`-equivalent in Python transport. The `on_live_event` handler in the GTK app gains handling for `TRANSLATION_DELTA` exactly as before, and gains handling for `SESSION_STATE` (`"connecting"` / `"disconnected"`) events.

### D6 — Flask UI: remove batch routes, keep only WebSocket live routes

All `@API.post` batch routes are removed. The `_run_live_socket` function is updated to use the new translation transport path. `bridge_websocket_messages` is removed from `transport.py`; the Flask handler calls into the realtime client directly without the bridge helper, mirroring how the Rust app drives the session from its native audio channel.

### D7 — PCM16 negative-sample multiplier: align to Rust's `32768.0`/`32767.0` split

The Python `float_samples_to_pcm16` currently uses a uniform `32767.0`. Rust uses `32768.0` for negative samples (full-range mapping) and `32767.0` for positive. Align Python to match.

### D8 — Voice defaults normalised to lowercase on load

Python `config.py` `Settings` defaults `female_voice="Nova"`, `male_voice="Onyx"`. Change to `"nova"` / `"onyx"` and apply a `fill_defaults` normalisation (lowercase, strip) on `load_settings`, mirroring Rust's `fill_defaults` function.

## Risks / Trade-offs

- [Batch endpoint removal breaks existing web clients] → Document the break clearly in the PR; no migration path is offered since these endpoints are being intentionally removed.
- [Translation WebSocket endpoint (`gpt-realtime`) may not be available to all API tiers] → Python will surface the connection error directly in the UI just as Rust does; no fallback.
- [Removing `pydub`/`soundfile` from the live path] → These were only used for batch WAV conversion. The live path never needed them. Verify `pyproject.toml` dependencies after removal.
- [Anonymous transcript deduplication removal] → The Rust impl does not deduplicate anonymous completions. Removing the Python check brings parity but may cause duplicate anonymous segments in edge cases. Accepted as the Rust behaviour is the target.

## Migration Plan

1. Update `dictaite_core` library first (events, transport, audio, config) — no UI changes yet.
2. Update GTK UI to remove the batch path and use the updated library.
3. Update Flask UI to remove batch routes and use the updated library.
4. Update `pyproject.toml` to remove `pydub` and `soundfile` if they are no longer used anywhere.
5. Run the existing test suite; update tests that cover removed functionality.

Rollback: the change is confined to the Python tree; the Rust binary is unaffected. Reverting the Python changes restores the old behaviour.

## Open Questions

- Does `flask-sock` correctly handle the async generator pattern needed for the new direct WebSocket session, or does the Flask side still need the `bridge_websocket_messages` bridge adapter? (Likely needs a thin synchronous adapter, not the full bridge.)
- Should `models.py` (`LiveMode`) be retained as a re-export for any downstream code importing it, or hard-deleted? (Proposal says hard-delete; confirm no external callers.)
