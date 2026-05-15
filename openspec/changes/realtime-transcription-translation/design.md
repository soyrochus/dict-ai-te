## Context

`dict-ai-te` currently supports Python GTK, Flask web, and Rust egui interfaces built around completed audio files. The Python shared services live in `dictaite_core`, while UI packages call into those services. The Rust app has its own OpenAI HTTP client and local microphone/audio pipeline.

OpenAI now provides distinct realtime paths for live transcription and live speech translation. Realtime transcription uses `gpt-realtime-whisper` and emits source transcript deltas. Realtime translation uses `/v1/realtime/translations` with `gpt-realtime-translate` and emits source transcript, translated transcript, and translated audio deltas.

## Goals / Non-Goals

**Goals:**

- Make live transcription the default user-facing flow in Python web, Python GTK, and Rust egui.
- Add optional live translation as a toggle that switches the realtime session type.
- Keep shared Python realtime logic in `dictaite_core`.
- Preserve existing user value: editable accumulated text, copy/save actions, language settings, and safe API key handling.
- Add mocked tests for audio conversion, event parsing, transcript assembly, WebSocket bridge behavior, state transitions, and failure cases.

**Non-Goals:**

- Do not play translated audio in the first implementation.
- Do not expose the standard OpenAI API key to browser JavaScript.
- Do not keep a user-facing classic recording mode.
- Do not implement live translation by repeatedly calling text translation after partial transcription.

## Decisions

1. Shared Python realtime code lives under `dictaite_core/realtime`.

   Rationale: the existing architecture already separates reusable services from UI packages. Putting realtime clients, event normalization, audio conversion, and transcript assembly in `dictaite_core` lets Flask and GTK share the same behavior.

   Alternative considered: `dictaite/realtime`. Rejected because it would put reusable service logic inside the UI package boundary.

2. The Flask web UI uses a server-owned WebSocket bridge.

   Rationale: Flask owns the OpenAI API key and forwards only normalized events to the browser. The browser captures microphone audio, converts it to mono 24 kHz PCM16, sends base64 chunks to Flask, and Flask streams those chunks to OpenAI.

   Alternative considered: direct browser WebRTC to OpenAI with ephemeral client secrets. This is simpler for browser media but introduces a different authentication/session path. The server bridge is preferred for parity with GTK/Rust server-side API key handling.

3. The project must choose an explicit Flask-compatible WebSocket server dependency before implementation.

   Rationale: the current dependency list includes Flask but no bidirectional WebSocket server. The implementation can use `Flask-Sock`/`simple-websocket`, `flask-socketio`, or migrate this surface to an ASGI framework such as Quart, but the choice must support streaming without blocking the UI path.

4. Translation mode uses the dedicated realtime translation session.

   Rationale: `gpt-realtime-translate` is designed to stream translated audio and transcript deltas while source audio is still arriving. Chaining live transcription into repeated text translation calls would add latency and create unstable partial translations.

5. Transcript assembly is segment-based for source transcription.

   Rationale: completion events can arrive out of order. The application tracks source transcript segments by `item_id`, preserves first-seen order, replaces partial segments with final transcripts, and avoids duplicate visible text.

6. Rust realtime uses Tokio and `tokio-tungstenite`.

   Rationale: the existing Rust OpenAI client uses blocking HTTP, which is unsuitable for bidirectional realtime streaming. A Tokio runtime owns WebSocket tasks, bounded audio channels, stop signaling, and UI event forwarding.

7. Existing settings migrate rather than disappear.

   Rationale: users may already have `translate_by_default`, `default_target_language`, and `default_language`. These map to `live_translation_enabled`, `target_language`, and `source_language` respectively. The existing default target language should remain unless a separate product decision changes it.

## Risks / Trade-offs

- Browser PCM conversion is fragile across devices → Use an `AudioWorklet` where possible, add browser capability checks, and test Chrome/Firefox with common input sample rates.
- Flask WebSocket bridge can block workers if implemented with the wrong server stack → Choose a streaming-capable WebSocket dependency and avoid long blocking work in request handlers.
- Realtime API event names or payloads can evolve → Normalize events at the boundary, ignore unknown events safely, and log only event type/minimal metadata.
- Source-language configuration for translation may not be supported → Treat source language as optional metadata unless the API documents a supported field.
- Partial transcript edits can conflict with user edits while streaming → Prefer a live buffer plus editable final text after stop if continuous editable merging becomes unreliable.
- Rust audio callbacks can underrun or block → Keep callbacks short, use bounded channels, and move resampling/chunking into worker tasks.

## Migration Plan

1. Add shared realtime models, event normalization, audio conversion, and mocked OpenAI WebSocket clients.
2. Add transcript assembly and settings migration tests.
3. Replace Flask recording/upload UI with Start Listening/Stop controls and server WebSocket streaming.
4. Replace Python GTK recording workflow with live listening controls and background realtime event handling.
5. Add Rust realtime modules, async dependencies, and egui live state.
6. Remove `/api/transcribe` and `/api/record/finalize` from active UI paths, leaving temporary compatibility only if needed during migration.
7. Update README and architecture docs.
8. Run unit tests and manual live checks for transcription, translation, stop/start, microphone denial, network disconnect, and invalid API key.

Rollback strategy: keep the existing batch services isolated until the live path has passing tests and manual verification. If the live migration fails late, restore the old UI route bindings while leaving unused realtime modules quarantined.

## Open Questions

- Which Flask WebSocket dependency should the implementation use?
- Should the live translation default target remain the current `"en"` or intentionally change to `"es"`?
- Is Python GTK still maintained enough to require full parity in the same implementation change?
- Which Rust resampling crate should be used for production audio quality?
