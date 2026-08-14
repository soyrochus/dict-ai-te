## 1. Phase 1 — Audio pipeline refactor (no native dependencies)

- [ ] 1.1 Add `AudioSpec { sample_rate: u32, frame_samples: usize }` in `src/realtime/audio.rs`, with constructors for the OpenAI spec (24 kHz, 960 samples) and a documented 16 kHz / 512-sample local spec
- [ ] 1.2 Change `LiveCapture::start` to take an `AudioSpec` and an audio sender of `Vec<f32>`; drive `choose_input_config` from `spec.sample_rate` instead of the `TARGET_SAMPLE_RATE` constant
- [ ] 1.3 Rewrite `audio_worker` to downmix, resample to `spec.sample_rate`, and emit fixed frames of exactly `spec.frame_samples`, buffering across device callbacks
- [ ] 1.4 Zero-pad and flush the trailing partial frame at stop, before the existing `audio.capture.stopped` session state event
- [ ] 1.5 Move PCM16 conversion, 40 ms chunking, and base64 encoding out of the capture worker and into the OpenAI transport, immediately before `input_audio_buffer.append`
- [ ] 1.6 Update `run_live_transcription` / `run_live_translation` signatures to receive `Vec<f32>` frames instead of base64 `String`s
- [ ] 1.7 Update `App::start_recording` to build the `AudioSpec` from the selected backend and pass it to `LiveCapture::start`
- [ ] 1.8 Add a test asserting the OpenAI backend's base64 output is byte-identical to the previous implementation for a known `f32` input sequence
- [ ] 1.9 Add tests for exact frame sizing across misaligned callback buffers, the zero-padded trailing flush, and downmix + resample to 16 kHz
- [ ] 1.10 Verify the full-queue path still drops without blocking and still emits the dropped-audio `Error` event
- [ ] 1.11 Run `cargo test` and manually verify a remote transcription session and a remote translation session still work end to end

## 2. Phase 2 — Events and assembler (no native dependencies)

- [ ] 2.1 Add `RealtimeEvent::SourceUpdate { item_id, text }` to `src/realtime/events.rs`; the OpenAI parser never produces it
- [ ] 2.2 Add `TranscriptAssembler::update(item_id, text)` in `src/realtime/transcript.rs`: replace segment text, register order on first sight, leave the segment provisional
- [ ] 2.3 Ensure a sealed (completed) segment ignores later `update` and `add_delta` calls
- [ ] 2.4 Handle `SourceUpdate` in `App::poll_live_events`, refreshing `source_transcript`, `transcript`, and `raw_transcript` as the delta and completed arms do
- [ ] 2.5 Add assembler tests: successive updates replace, revised words are reflected, update after completion is ignored, completion overwrites a provisional segment, ordering across interleaved segments is first-seen
- [ ] 2.6 Add a test asserting existing `SourceDelta` append behaviour and remote event parsing are unchanged

## 3. Phase 2 — Settings schema

- [ ] 3.1 Add a `Backend` enum (`openai` | `local`, serde lowercase, default `openai`) and a `LocalSettings` struct (`engine`, `model`, `model_path`, `device`) with serde defaults, to `src/settings.rs`
- [ ] 3.2 Add `backend` and `local` fields to `Settings`, defaulting so that a pre-change settings file loads unchanged
- [ ] 3.3 Add unknown-key capture to `Settings` so unrecognised keys survive a load-save round trip
- [ ] 3.4 Add a single `effective_translate(settings, …)` helper returning `false` whenever the backend is local, and route both the settings UI and session start through it
- [ ] 3.5 Ensure switching to local mode does not overwrite the persisted `translate_by_default` value
- [ ] 3.6 Add settings tests: defaults for a legacy file, round trip of `backend` and `local`, unknown-key preservation, `effective_translate` forcing, and preservation of the remote translate preference

## 4. Phase 2 — Backend dispatch and session lifecycle

- [ ] 4.1 Add a `LiveBackend` enum (`OpenAi`, `Local`) and branch `App::start_recording` on the effective backend
- [ ] 4.2 Define the `LocalEngine` trait (`audio_spec`, `start`, `feed_frame`, `stop`) in a new `src/local/` module, with a `LocalSessionConfig` carrying the language hint, model resolution, and device preference
- [ ] 4.3 Implement the local session runner: a dedicated thread that blocking-receives audio frames, drives the engine, and emits `RealtimeEvent`s directly on the existing std `mpsc` UI channel with no tokio runtime
- [ ] 4.4 Assign monotonic `local-N` segment ids in the local runner and assert no local event is emitted with an absent `item_id`
- [ ] 4.5 Emit the local session states `local.loading`, `connecting`, `local.listening`, `local.processing`, `disconnected`, and extend `live_state_text` to render each readably
- [ ] 4.6 Finalize any open segment on stop (final decode, `SourceCompleted`) before emitting `disconnected`; ensure stop returns promptly with an inference in flight
- [ ] 4.7 Bypass the OpenAI client requirement and the startup missing-key warning when the backend is local
- [ ] 4.8 Stop an active session cleanly when the backend setting changes mid-session, preserving the finalized transcript
- [ ] 4.9 Route recognition failures to an `Error` event that drops only the affected segment and leaves the session running

## 5. Phase 2 — UI

- [ ] 5.1 Add the backend radio group to the settings modal in `src/app.rs`, persisting to `settings.backend`
- [ ] 5.2 Disable the "Translate Live" checkbox and hide the target-language selector in local mode, showing the transcription-only explanatory note
- [ ] 5.3 Disable the speak-transcript control with an explanatory tooltip when the backend is local and no API key is configured; leave it enabled when a key is present
- [ ] 5.4 Add the local model section to the settings modal: engine (fixed to Whisper), model dropdown, device dropdown, and model status line
- [ ] 5.5 Warn and fall back to English when a non-English origin language is selected with an English-only model
- [ ] 5.6 Ensure the local option is shown as unavailable, with a reason, in a build without the local feature, and that such a build runs remote when settings select local

## 6. Phase 2 — Test harness

- [ ] 6.1 Add a scripted fake `LocalEngine` behind a test-only feature that emits predetermined `SourceUpdate` / `SourceCompleted` sequences with no native dependencies
- [ ] 6.2 Add an integration test driving settings → capture → local backend → events → assembler with the fake engine
- [ ] 6.3 Add a structural no-network test asserting the local path never constructs `RealtimeSessionConfig` and never calls `run_live_transcription` / `run_live_translation`
- [ ] 6.4 Add a test that backend selection survives a settings save and reload
- [ ] 6.5 Verify save, copy, and edit of a transcript behave identically for local and remote sessions

## 7. Phase 3 — Whisper engine (feature-gated)

- [ ] 7.1 Add the `local-whisper` cargo feature with `metal` / `cuda` sub-features; add `whisper-rs` as an optional dependency and confirm the default build gains no native dependencies
- [ ] 7.2 Decide and add the Silero VAD dependency (`ort` or a pure-Rust alternative), recording the choice and its rationale in `design.md`
- [ ] 7.3 Implement model loading with device selection: `auto` picks the best available for the build and machine
- [ ] 7.4 Fall back to CPU with a status-line warning when the requested device is unavailable, rather than failing the session
- [ ] 7.5 Implement the VAD frame loop: speech onset opens a segment, sustained silence closes it, silence is never submitted to the recognizer
- [ ] 7.6 Implement periodic re-decode of the open segment at the 500 ms partial interval, emitting `SourceUpdate` with the full current hypothesis
- [ ] 7.7 Skip rather than queue a partial decode when the previous one is still running
- [ ] 7.8 Implement the 600 ms silence finalization (final decode, `SourceCompleted`, seal) and the 15 s force-cut that finalizes and immediately opens a new segment
- [ ] 7.9 Pass the language hint through from the Origin selector, with `default` mapping to auto-detect
- [ ] 7.10 Add unit tests for the segmentation state machine driven by a synthetic VAD: onset, silence close, force-cut, skipped overlapping partials, no decode during silence

## 8. Phase 4 — Model management

- [ ] 8.1 Define the embedded model manifest (id, display name, URL, size, SHA-256) and choose the models to ship
- [ ] 8.2 Implement model path resolution under `~/.dictaite/models/<engine>/<model-id>/`, honouring `DICTAITE_HOME`, with `local.model_path` taking precedence over `local.model`
- [ ] 8.3 Implement model availability states: ready, downloading, not downloaded, missing at configured path
- [ ] 8.4 Implement download with `reqwest` on a `poll-promise` background task: write `.part`, report byte and percentage progress, support cancellation
- [ ] 8.5 Verify SHA-256 against the manifest, then atomically rename into place; discard the partial file on mismatch or cancellation
- [ ] 8.6 Wire download progress, cancellation, and errors into the settings modal model status line
- [ ] 8.7 Add an `rfd` file picker that sets `local.model_path` to a user-chosen model file and refreshes status
- [ ] 8.8 Refuse to start a local session when the model is missing, with a message naming the model and offering the download action; report load failures without starting
- [ ] 8.9 Add tests for path resolution, `DICTAITE_HOME` override, `model_path` precedence, checksum mismatch handling, and interrupted-download cleanup

## 9. Verification

- [ ] 9.1 Run `cargo test` for the default build and for `--features local-whisper`
- [ ] 9.2 Run `cargo clippy` and `cargo fmt --check` on both feature configurations
- [ ] 9.3 Manually verify remote transcription and remote translation are unchanged after the phase 1 refactor
- [ ] 9.4 Manually verify a local session with no API key configured: no startup warning, session starts, transcript appears, TTS control disabled
- [ ] 9.5 Manually verify latency and accuracy against known phrases on CPU and on Metal; record measurements and tune the segmentation defaults if warranted
- [ ] 9.6 Manually verify model download, cancellation, checksum failure, and the `model_path` override
- [ ] 9.7 Manually verify error paths: model deleted mid-session, microphone unavailable, backend switched during an active session, repeated stop/start
- [ ] 9.8 Confirm no outbound speech-related traffic during a local session by inspecting network activity
- [ ] 9.9 Update `README.md` and `architecture-guide.md` with the backend selection, local mode constraints, and model setup
