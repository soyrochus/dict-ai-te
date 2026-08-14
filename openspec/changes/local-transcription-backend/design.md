## Context

The Rust application is live-first: `LiveCapture` streams microphone audio to an OpenAI Realtime WebSocket, the transport normalizes server messages into `RealtimeEvent`s, and `TranscriptAssembler` turns those into the text shown in the transcript pane. Three properties of that pipeline constrain this change:

1. **The audio channel is already base64 text.** `audio_worker` in `src/audio/live_capture.rs` downmixes, resamples to 24 kHz, converts to PCM16, and base64-encodes before sending `String`s over the channel. There is no raw PCM stream to tap.
2. **The sample rate is fixed at capture.** `TARGET_SAMPLE_RATE = 24_000` drives cpal's configuration selection. Whisper needs 16 kHz.
3. **The assembler is append-only.** `add_delta` concatenates into a segment; a `None` item id lands in an anonymous list that is never replaced. Whisper-class engines re-decode a growing window and emit whole revised hypotheses, which both paths would corrupt.

The Python GTK and Flask implementations share the settings file but are deprecated and out of scope. The source specification is `specs/2026-08-14-local-transcription-backend.md` (v2.0).

## Goals / Non-Goals

**Goals:**

- Transcription with no API key and no audio leaving the machine, selectable per user preference.
- The remote path's observable behaviour — wire format, events, UI, translation, TTS — unchanged.
- An extension point at the level where extension is actually expected: between local engines.
- A default build with no new native dependencies and no new toolchain requirements.
- A pipeline testable in CI without models or native builds.

**Non-Goals:**

- Local translation or local TTS.
- Visual distinction between provisional and finalized transcript text.
- True streaming latency; VAD-plus-chunked-inference latency is accepted.
- More than one local engine in this change.
- Any modification to the deprecated Python tree.

## Decisions

### Enum for remote/local, trait for engines

`LiveBackend` is an enum with `OpenAi` and `Local` variants; `LocalEngine` is a trait, and `Local` holds a `Box<dyn LocalEngine>`.

The source spec originally proposed a single `TranscriptionBackend` trait spanning remote and local. Rejected: the two share no implementation and run in different execution models — the remote path is a free `async fn` driven by channels on a tokio task, the local path is a blocking inference loop on a std thread. Unifying them would require `async-trait` and boxing to express a relationship that yields nothing at two variants, and would mean restructuring `transport.rs` for no functional gain. Meanwhile the pluggability the proposal actually calls for is between whisper / sherpa / vosk, which is exactly where the trait now sits.

### Encoding moves into the OpenAI backend; the channel carries `Vec<f32>`

Capture emits mono `f32` frames; PCM16 conversion, 40 ms chunking, and base64 encoding move into the OpenAI backend.

Alternative considered: leave encoding in place and have the local engine decode base64 back to `f32`. Rejected as pure waste and a correctness hazard — the local path would inherit the remote path's 24 kHz rate and quantization to PCM16 for no reason. The chosen direction is also an improvement to the remote path on its own: the wire format now lives with the code that owns the wire.

Byte-identical output after the refactor is an explicit requirement, tested against the current implementation's results, so the refactor is verifiable independently of any local work.

### `AudioSpec` declared by the backend

`AudioSpec { sample_rate, frame_samples }` is supplied to `LiveCapture::start`, replacing the module-level 24 kHz constant as the driver of device configuration.

`frame_samples` is part of the spec, not just the rate, because Silero VAD requires exactly 512 samples per frame at 16 kHz. Having capture produce correctly sized frames avoids a second buffering layer inside the engine. The OpenAI backend requests 24 kHz with 40 ms frames (960 samples), preserving today's chunking.

Alternative considered: always capture at the device's native rate and resample per backend. Rejected — it moves resampling into every backend and gives up the existing device-selection logic that already prefers a device configuration matching the target rate.

### `SourceUpdate` rather than reusing existing variants

A new `SourceUpdate { item_id, text }` replaces segment text and leaves the segment provisional; `TranscriptAssembler` gains a matching `update`.

Two alternatives were considered and rejected:

- **Emit `SourceCompleted` for every re-decode.** It happens to work, because `complete()` overwrites text unconditionally — but it labels provisional text as final, permanently foreclosing any UI that distinguishes the two, and makes "sealed" meaningless.
- **Diff in the backend and emit only the new suffix as a delta.** Whisper routinely revises earlier words ("recognise beach" → "recognise speech"); a suffix diff produces visible corruption whenever it does.

The cost of the chosen option is three files in one language (`events.rs`, `transcript.rs`, and the event match in `app.rs`). This was the expensive option when the Python implementations were in scope; with them deprecated it is the cheap and honest one.

### Local backends must assign segment ids

`item_id` stays `Option<String>` in the type, because remote events legitimately lack one, but local backends are required to always supply `local-N`. A `None` from a local backend would land in the assembler's anonymous list, which is never replaced, so repeated hypotheses would accumulate as duplicate fragments. This is a requirement rather than a type-level guarantee to avoid splitting the event enum by backend.

### Segmentation strategy

VAD opens a segment on speech onset; the open segment is re-decoded every 500 ms to emit `SourceUpdate`; sustained silence of 600 ms triggers a final decode and `SourceCompleted`; a segment reaching 15 s is force-cut and a new one opened immediately. A partial decode is skipped, never queued, if the previous one is still running.

The force-cut exists because re-decoding a growing window is quadratic in segment length; without a bound, a speaker who does not pause degrades until the machine cannot keep up. Skipping rather than queueing overlapping decodes has the same purpose: the system sheds load instead of accumulating a backlog it can never drain. The 600 ms silence threshold is chosen to sit near the remote path's `silence_duration_ms: 500` so the two backends feel similar. All four values are tunable defaults expected to be adjusted against real speech.

### No tokio runtime on the local path

The audio channel remains `tokio::sync::mpsc` — its channels are runtime-independent, `blocking_send` is already used from the capture worker's plain thread, and the inference thread can `blocking_recv`. Events go directly onto the existing std `mpsc` UI channel, so the tokio-to-std bridge task the remote path spawns is not involved. A local session therefore starts no runtime at all.

### Feature gating

`local-whisper` gates the engine, with `metal` / `cuda` sub-features for acceleration. This keeps the default build free of the whisper.cpp native toolchain, as the proposal requires — but it creates a runtime state the source spec did not anticipate: a binary without the feature reading a settings file that selects `local`. That state is handled explicitly rather than left to fail at session start: the selector disables the local option with an explanation and the session runs remote.

### Model management reuses existing dependencies

`reqwest` (download), `rfd` (file picker), and `poll-promise` (background task with UI polling) are already dependencies, so in-app download with progress adds no crates. Downloads write `.part`, verify SHA-256 from the embedded manifest, then rename atomically — an interrupted download must never leave a file that looks like a valid model, because a truncated model produces confusing engine-level load errors rather than an obvious "not downloaded" state.

### One language setting, not two

The source spec proposed a `local.language` field. Dropped: the Origin language selector already exists and feeds the remote path, and two sources of language truth would diverge. `default` maps to auto-detect, everything else passes its `LANGUAGES` ISO code. English-only models warn and transcribe as English rather than failing.

### Unknown settings keys are preserved

`Settings` gains an unknown-key capture so a save round-trips fields this build does not know. The immediate motivation is the deprecated Python applications, which share `~/.dictaite/settings.json` and drop unknown keys on save: a user who launches the frozen Python app once would otherwise silently lose `backend` and `local`. The Rust side cannot prevent that loss, but it can avoid being a second source of it, and the property is worth having permanently.

### Fake engine as the test strategy

A scripted `LocalEngine` implementation behind a test-only feature emits predetermined segments with no native dependencies, exercising settings, capture, events, assembler, and UI state in CI. The no-network guarantee is asserted structurally — the local path never constructs `RealtimeSessionConfig` and never calls the transport functions — rather than by observing sockets, which is both more reliable and cheaper. This matters more than usual here: the repository's only out-of-crate test suite is the Python one, which is deprecated, so the Rust side is where coverage has to live.

## Risks / Trade-offs

- **Quadratic re-decode cost on slow machines** → The 15 s force-cut bounds worst-case window length; overlapping partials are skipped rather than queued. If a machine still cannot keep up, provisional updates thin out while final results remain correct.
- **Whisper revising earlier words looks like flicker** → Accepted for v1. `SourceUpdate` carries the whole hypothesis, so the transcript is always self-consistent; the alternative (suffix diffing) produces corruption rather than flicker.
- **Native build complexity for local builds** → Confined behind `local-whisper`; the default build and CI path are unaffected. Contributors who do not opt in never install a native toolchain.
- **Large model downloads with no resume** → Downloads are cancellable and verified, and a failed download costs bandwidth but never corrupts state. Resume is deliberately deferred.
- **Latency and quality defaults are estimates** → The four segmentation parameters are settings-free constants in this change and expected to be tuned during manual smoke testing; making them user-visible before they are understood would harden guesses into an interface.
- **Accuracy will disappoint against the remote path** → A small local model is measurably worse than `gpt-4o-transcribe`. Model choice is exposed so users can trade size for accuracy, and local mode is opt-in with the remote path unchanged as the default.
- **Settings loss via the deprecated Python apps** → Unknown-key preservation on the Rust side limits the blast radius but cannot eliminate it. Accepted: the Python tree is out of scope by explicit decision.
- **Refactoring the audio path risks the working remote path** → Mitigated by the byte-identical output requirement, which is verifiable against the current implementation before any local code exists.

## Migration Plan

The change is additive and needs no data migration. Settings files without `backend` / `local` load with defaults.

Phasing, chosen so each stage is independently valuable and testable:

1. **Audio pipeline** — `AudioSpec`, `Vec<f32>` channel, encoding relocated. No native dependencies; verifiable as a pure refactor of the remote path.
2. **Events, settings, UI, fake engine** — `SourceUpdate` and assembler semantics, settings schema and forced-translate helper, backend selector, and the scripted test engine. Still no native dependencies; the full pipeline becomes testable.
3. **Whisper engine** — `whisper-rs`, Silero VAD, segmentation, device selection, behind the feature flag.
4. **Model management** — manifest, download, verification, progress UI.

Phases 1 and 2 are worth landing even if the engine work slips. Rollback is per phase: phases 3 and 4 disappear by disabling the cargo feature, and phases 1 and 2 are revertible independently because neither changes observable behaviour on the remote path.

## Open Questions

- The four segmentation defaults (500 ms partial, 600 ms silence, 15 s cut, 512-sample frames) are estimates pending measurement against real speech on target hardware.
- Which manifest models to ship: the tradeoff between `base.en` as a fast default and `small` as an accuracy default has not been settled, and depends on measured performance on a typical machine.
- Whether Silero VAD arrives via `ort` (ONNX runtime, a heavy dependency even when feature-gated) or a pure-Rust alternative. This is a phase 3 decision and does not block phases 1–2.
