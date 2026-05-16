## Why

The active `realtime-transcription-translation` change has implemented most Python live transcription/translation behavior, but the Rust egui app still runs the old record-then-transcribe flow and the Realtime API contract has not been revalidated against current OpenAI documentation. This change finishes the remaining implementation and verification work so the product is actually live-first across all supported interfaces.

## What Changes

- Complete the Rust live microphone pipeline so `cpal` capture streams mono 24 kHz PCM16 chunks through bounded channels instead of buffering a full clip before transcription.
- Wire the Rust egui UI to Tokio realtime transcription and translation sessions, with source and translated transcript panes, connection state, stop/start lifecycle, and preserved edit/copy/save behavior.
- Verify and update OpenAI Realtime model names, endpoints, session update payloads, event names, audio format fields, and close semantics against current official documentation before treating the transport as complete.
- Add mocked/unit coverage for Rust realtime session state, audio streaming boundaries, event forwarding, unknown event safety, and transcript assembly behavior.
- Add a manual acceptance checklist covering live transcription, live translation, repeated stop/start, microphone unavailable, network disconnect, invalid API key, and API contract drift.
- Keep obsolete batch code only where still needed for non-live features such as text-to-speech playback; remove or quarantine user-facing batch transcription paths after live paths are verified.

## Capabilities

### New Capabilities

- `rust-live-realtime-integration`: Rust egui live transcription/translation behavior, microphone streaming, UI event forwarding, and transcript preservation.
- `realtime-api-contract-validation`: Verification and maintenance of OpenAI Realtime endpoint, model, payload, event, and audio-format assumptions before implementation completion.
- `live-manual-acceptance`: Required manual verification scenarios for live transcription, live translation, lifecycle reliability, and error handling.

### Modified Capabilities

- None.

## Impact

- Rust app: `src/app.rs`, `src/audio/*`, `src/realtime/*`, `src/openai.rs` only where API key/settings reuse is needed, and `Cargo.toml`/`Cargo.lock` for runtime/channel/resampling dependencies.
- Tests: Rust unit tests and any isolated integration tests that can run without OpenAI network access.
- OpenSpec/docs: update task tracking and architecture notes to reflect the verified Realtime API contract and manual acceptance evidence.
- Runtime behavior: Rust egui becomes live-first like Python web and GTK; user-facing record/upload/post-recording transcription status is removed from the Rust path.
