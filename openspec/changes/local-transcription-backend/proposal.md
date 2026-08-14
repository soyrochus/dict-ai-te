## Why

Every transcription today requires an OpenAI API key and ships microphone audio to a third party, which rules out the application for private, offline, or air-gapped use and makes it unusable without a paid account. Adding an on-device speech-to-text backend removes both constraints while leaving the existing Realtime path — the only path with translation and TTS — fully intact.

## What Changes

- **Backend choice**: a new `backend` setting (`"openai"` | `"local"`) selectable in the Settings dialog. OpenAI Realtime remains the default and is behaviourally unchanged.
- **Local transcription-only mode**: when `backend == "local"`, translation is forced off and its controls disabled, no API key is required, the startup missing-key warning is suppressed, and no network call related to speech recognition is made.
- **Audio pipeline refactor**: the capture worker currently converts to PCM16 and base64-encodes before the audio leaves `live_capture.rs`, so no raw PCM stream exists to feed a local engine. Capture now emits mono `f32` frames sized by an `AudioSpec` the selected backend declares (24 kHz for OpenAI, 16 kHz for Whisper), and PCM16/base64 encoding moves into the OpenAI backend. The remote wire format is unchanged.
- **New event variant**: `RealtimeEvent::SourceUpdate` (replace segment text, keep it provisional) plus a matching `TranscriptAssembler::update`. Local engines re-decode a growing window and emit whole revised hypotheses, which the existing append-only `SourceDelta` and never-replaced anonymous-segment path would corrupt. Local backends must assign stable segment ids; emitting `None` is prohibited for them.
- **Whisper engine**: `whisper-rs` (whisper.cpp) plus Silero VAD on a dedicated inference thread, with onset/silence/force-cut segmentation producing provisional updates during speech and a final result after it. Behind a `local-whisper` cargo feature with `metal` / `cuda` sub-features, so the default build stays OpenAI-only and lightweight.
- **Model management**: a manifest embedded in the binary, in-app download with progress into `~/.dictaite/models/<engine>/<model-id>/`, SHA-256 verification, atomic `.part`-then-rename, plus a `local.model_path` override for user-supplied model files.
- **Settings robustness**: unknown keys in `settings.json` are preserved across a save instead of being dropped.
- **Test harness**: a fake local engine behind a test-only feature exercises the whole pipeline with no native dependencies, and the no-network guarantee is asserted structurally.

No breaking changes: existing settings files load with defaults, and the remote path's observable behaviour is unchanged.

## Capabilities

### New Capabilities

- `transcription-backend-selection`: Choosing between the OpenAI Realtime and local backends, the transcription-only constraint of local mode (forced-off translation, no API key required, TTS gating), language handling, and the guarantee that local mode performs no speech-related network traffic.
- `backend-audio-pipeline`: `AudioSpec`-driven microphone capture producing mono `f32` frames at a backend-declared rate and frame size, with PCM16/base64 encoding relocated into the OpenAI backend and the remote wire format preserved.
- `provisional-transcript-events`: The `SourceUpdate` event and assembler replace-without-seal semantics, sealing rules, and the requirement that local backends assign stable non-empty segment ids.
- `local-whisper-engine`: The pluggable `LocalEngine` trait and its Whisper implementation — model loading, device selection with CPU fallback, VAD-driven segmentation, latency targets, threading, and cancellation.
- `local-model-management`: Model manifest, availability states, in-app download with progress and cancellation, checksum verification, atomic installation, storage layout, and the manual `model_path` override.

### Modified Capabilities

<!-- None. No baseline specs exist under openspec/specs/; the realtime capabilities defined in prior
     unarchived changes keep their current requirements — the remote path's observable behaviour is
     unchanged by this work. -->

## Impact

- **Source**: `src/settings.rs` (schema, unknown-key preservation, forced-translate helper), `src/audio/live_capture.rs` (`AudioSpec`, `f32` frames), `src/realtime/audio.rs` (encoding helpers move to the OpenAI backend), `src/realtime/events.rs` and `src/realtime/transcript.rs` (`SourceUpdate`), `src/realtime/transport.rs` (encode before send), `src/app.rs` (backend branch at session start, settings UI, status mapping, TTS gating), plus a new `src/local/` module for the engine, VAD, segmentation, and model management.
- **Dependencies**: adds `whisper-rs` and a Silero VAD path (`ort` or a pure-Rust alternative), both behind the `local-whisper` feature; the default build gains nothing. Model download, file picking, and progress reuse the existing `reqwest`, `rfd`, and `poll-promise` dependencies.
- **Build**: feature-gated native toolchain requirement (cmake/whisper.cpp) for local builds only. A binary built without the feature disables the local option and explains why rather than failing at session start.
- **Settings file**: gains `backend` and `local`; existing files load unchanged. The deprecated Python applications share this file and drop unknown keys on save, which is why unknown-key preservation is included on the Rust side.
- **Data**: models are downloaded to `~/.dictaite/models/` (or under `DICTAITE_HOME`) and can be hundreds of MB.
- **Out of scope**: the deprecated Python GTK and Flask applications are not modified.
- **Source spec**: `specs/2026-08-14-local-transcription-backend.md` (v2.0).
