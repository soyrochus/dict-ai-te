## Context

The active `realtime-transcription-translation` change moved Python web and GTK toward live Realtime behavior, but the Rust egui app still records into an `AudioClip`, waits for stop, calls the blocking HTTP transcription endpoint, and optionally calls a separate text translation endpoint. A new `src/realtime` module exists, but it is not integrated with the Rust UI or microphone lifecycle.

The Realtime API assumptions in the current implementation also need a final verification pass before the Rust transport is considered complete. Model names, session update payload shapes, endpoint paths, event names, and close/commit behavior are external API details and must be checked against current official OpenAI documentation during implementation.

## Goals / Non-Goals

**Goals:**

- Make the Rust egui app live-first, matching the product behavior already targeted for Python web and GTK.
- Stream microphone audio continuously as mono 24 kHz PCM16 chunks through bounded channels.
- Run OpenAI Realtime transcription and translation sessions on a Tokio runtime without blocking egui or `cpal` callbacks.
- Surface source transcript deltas/completions and translated transcript deltas into editable egui text panes.
- Verify the Realtime API contract against current official OpenAI docs before finalizing model names, endpoint paths, payload fields, event parsing, and close semantics.
- Add offline automated tests for Rust audio conversion, chunking, event parsing, transcript assembly, state transitions, and UI event forwarding.
- Define manual acceptance evidence required before the original realtime migration can be considered complete.

**Non-Goals:**

- Do not play translated realtime audio in this completion change.
- Do not expose an OpenAI API key to browser JavaScript.
- Do not reintroduce a user-facing classic recording mode in Rust.
- Do not rely on live OpenAI calls in standard automated tests.
- Do not redesign the Python web or GTK implementations except where API-contract corrections must be shared.

## Decisions

1. Rust audio capture uses a new live capture path rather than adapting the batch `Recorder::stop() -> AudioClip` path.

   Rationale: the existing recorder is built around a shared in-memory sample buffer and final clip creation. Live Realtime requires backpressure, bounded queues, and continuous chunk delivery. Keeping the batch recorder for any remaining non-live utility avoids destabilizing old helper code while the UI moves to live capture.

   Alternative considered: make `Recorder` serve both batch and live modes. Rejected because dual-mode state would obscure callback constraints and make stop/start bugs harder to isolate.

2. `cpal` callbacks push small sample buffers into a bounded channel and return quickly; conversion/chunking happens on worker-side code.

   Rationale: audio callbacks must not perform network I/O, block on async runtimes, or do heavy resampling. Worker-side conversion lets the system apply backpressure and drop/report overflow in a controlled way.

   Alternative considered: convert to base64 PCM directly inside the callback. Rejected unless a later implementation proves the conversion is small enough and still non-blocking on target platforms.

3. The Rust app owns a Tokio runtime for Realtime sessions and bridges events back to egui through a non-async UI channel.

   Rationale: `tokio-tungstenite` requires async execution, while egui update code is synchronous. A runtime plus channels keeps WebSocket I/O separate from rendering and keeps the UI responsive.

   Alternative considered: spawn one OS thread per WebSocket and use blocking tungstenite. Rejected because the project already selected Tokio and realtime cancellation/backpressure are clearer with async channels.

4. API contract verification is a required implementation step, not optional documentation cleanup.

   Rationale: the current change refers to model and endpoint names that may not match current official docs. Completing the feature without reconciling those names risks producing a polished UI over a broken transport.

   Alternative considered: implement exactly what the previous OpenSpec says and fix API drift later. Rejected because manual live acceptance depends on the transport working against the current API.

5. Translation mode uses the dedicated realtime translation/session behavior only if current official docs still support that contract; otherwise implementation must update the design before completing.

   Rationale: the product requirement is low-latency live translation, not chained text translation after transcription. If the external API has changed, the implementation must preserve the product intent while using a supported API path.

   Alternative considered: fall back to streaming transcription plus repeated text translation. Rejected because it violates the original latency and stability requirement.

## Risks / Trade-offs

- Realtime API drift -> Mitigation: verify official docs before coding the transport, capture the verified contract in code comments or architecture notes, and keep event parsing tolerant of unknown event types.
- Audio callback backpressure -> Mitigation: use bounded channels, keep callbacks short, record/report dropped audio chunks where practical, and test stop/start lifecycle.
- Egui/threading bugs -> Mitigation: use one UI event receiver polled from `App::update`, avoid mutating egui state from background tasks, and request repaint while sessions are active.
- Transcript duplication or stale partial text -> Mitigation: keep segment-based source assembly keyed by `item_id`, replace partial segments on completion, and append anonymous deltas safely.
- Manual testing depends on microphone/network/API key availability -> Mitigation: keep automated tests offline and make manual acceptance explicit, including what could not be executed in the local environment.

## Migration Plan

1. Verify current OpenAI Realtime transcription and translation contracts from official docs.
2. Update Rust realtime models/transport payloads and event parsing to match the verified contract.
3. Add a live `cpal` capture path that streams bounded sample buffers into worker-side PCM conversion/chunking.
4. Integrate Tokio session startup, audio forwarding, stop signaling, and UI event forwarding into `DictaiteApp`.
5. Replace user-facing Rust batch transcription controls/status with live-first controls and source/translated transcript panes.
6. Preserve copy/save/edit/playback behavior for accumulated text, while keeping text-to-speech playback separate from live transcription.
7. Add mocked/offline tests.
8. Run automated tests and record manual acceptance results for the required scenarios.

Rollback strategy: keep the existing blocking OpenAI HTTP client and batch recorder isolated until Rust live acceptance passes. If the live Rust integration fails late, disable the new live launch path while leaving verified realtime modules and tests in place for a follow-up fix.

## Open Questions

- Which current OpenAI Realtime translation endpoint and model name should be treated as authoritative after docs verification?
- Should the existing Rust batch recorder remain as an internal helper for future non-live features, or be removed entirely after live acceptance?
- Should manual acceptance evidence live in the active change `tasks.md`, a dedicated notes file, or architecture docs?
