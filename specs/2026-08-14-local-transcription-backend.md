# Specification: Local Transcription Backend for dict-ai-te

**Version:** 2.0
**Status:** Draft — basis for an OpenSpec proposal
**Scope:** Add an optional local (on-device) speech-to-text backend to the Rust application while preserving the existing OpenAI Realtime path. Local mode is transcription-only.

> **Note on implementations:** This specification covers the Rust implementation only. The Python implementations (GTK desktop and Flask web) are deprecated and out of scope — they are not modified by this work.

---

### 1. Goals

- Allow users to choose between **remote** (OpenAI Realtime API) and **local** (on-device) transcription.
- When local mode is active:
  - Provide live / near-real-time transcription only.
  - Disable translation completely.
- Keep the remote path fully featured (transcription + translation + TTS) and behaviourally unchanged.
- Make the local engine pluggable so different engines can be added later.
- Preserve privacy: in local mode, audio never leaves the machine.
- Require no API key when local mode is selected.
- Keep the default build lightweight: heavy native dependencies are opt-in at compile time.

### 2. Non-Goals (v1)

- Local translation.
- Local TTS (OpenAI TTS remains available when an API key is present; see §3.4).
- Perfect sub-200 ms latency matching commercial streaming STT (local engines use VAD + chunked inference).
- Automatic model fine-tuning or custom vocabulary.
- Support for every possible local engine on day one.
- Visually distinguishing provisional from finalized text in the transcript pane (see §5.4).
- Any change to the deprecated Python tree.

---

### 3. User-Facing Behavior

#### 3.1 Backend Selection

The application exposes a clear choice in the Settings dialog:

- **OpenAI Realtime** (default) — full features (transcription + optional translation).
- **Local model** — transcription only.

When the user selects **Local model**:

- The "Translate Live" control is disabled and forced to `false`. The target-language selector is hidden, as it already is whenever translation is off.
- A short explanatory note is shown:
  *"Local mode supports transcription only. Translation requires the OpenAI Realtime API."*
- The OpenAI API key is no longer required for a session to start. The missing-API-key warning shown at startup must not appear when the selected backend is `local`.
- If a local model is not yet available, the application guides the user to download one (§7) or fails gracefully with a clear message.

When the user switches back to **OpenAI Realtime**:

- Translation becomes available again, restoring the previously persisted `translate_by_default` preference.

#### 3.2 Session Behavior (Local Mode)

1. User clicks Start Listening.
2. Microphone audio is captured using the existing `cpal` capture pipeline.
3. Audio is processed locally (VAD + recognition) on a dedicated inference thread.
4. Provisional and final transcript segments appear in the existing transcript area in near real time.
5. User can stop, edit, copy, or save the transcript exactly as today.
6. No network calls related to speech recognition are made.

#### 3.3 Language

- Local mode uses the existing **Origin language** selector; it does not introduce a second language setting.
- `default` (auto-detect) maps to the engine's auto-detect mode. All other entries pass their ISO code (`en`, `es`, `de`, …) from `LANGUAGES` to the engine.
- If the selected engine or model does not support the chosen language (for example an English-only Whisper model such as `base.en`), the application shows a warning and transcribes as English rather than failing the session.

#### 3.4 TTS in Local Mode

TTS remains an OpenAI feature. When the backend is `local` **and** no API key is configured, the "speak transcript" control is disabled with an explanatory tooltip rather than failing at click time. With an API key present, TTS behaves exactly as today.

---

### 4. Configuration & Settings

Settings continue to live at `~/.dictaite/settings.json` (or under `DICTAITE_HOME`).

**Extended schema** (existing field names preserved):

```json
{
  "default_language": null,
  "translate_by_default": false,
  "default_target_language": "en",
  "female_voice": "nova",
  "male_voice": "onyx",

  "backend": "openai",
  "local": {
    "engine": "whisper",
    "model": "base.en",
    "model_path": null,
    "device": "auto"
  }
}
```

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `backend` | `"openai" \| "local"` | `"openai"` | Selected transcription backend |
| `local.engine` | string | `"whisper"` | Engine id; extensible (§6) |
| `local.model` | string | `"base.en"` | Model id from the built-in manifest (§7) |
| `local.model_path` | string \| null | `null` | Absolute path override; when set, `local.model` is ignored |
| `local.device` | `"auto" \| "cpu" \| "metal" \| "cuda"` | `"auto"` | Acceleration preference; `auto` picks the best available for the build |

**Rules:**

- When `backend == "local"`, the effective translate flag is `false` regardless of `translate_by_default`. This must be enforced in one shared helper used by both the settings UI and session start, not duplicated at each call site.
- `translate_by_default` retains the user's remote-mode preference while local mode is active; it is not overwritten on the way in or out of local mode.
- Unknown or future keys present in `settings.json` must be preserved on save rather than dropped, so that settings written by another version of the application survive a round trip.
- An existing settings file without `backend` / `local` loads with the defaults above; no migration step is required.
- Changing `backend` takes effect on the next session start. Changing it while a session is active is handled per §9.
- Model cache directory: `~/.dictaite/models/` (or under `DICTAITE_HOME`).

---

### 5. Architecture

#### 5.1 Backend and Engine Split

Two different abstraction levels, deliberately not merged:

```
                 ┌──────────────────────────────────────────┐
                 │  LiveBackend  (enum — chosen at Start)    │
                 ├──────────────────────┬───────────────────┤
                 │  OpenAi              │  Local            │
                 │  async, tokio task   │  blocking thread  │
                 │  WebSocket transport │  Box<dyn LocalEngine>
                 └──────────────────────┴─────────┬─────────┘
                                                  │
                              ┌───────────────────┴──────────────────┐
                              │  trait LocalEngine (pluggable)       │
                              │  WhisperEngine │ (future: Sherpa, …) │
                              └──────────────────────────────────────┘
```

- The remote/local choice is an **enum**, not a trait object. The two paths share no implementation and run in different execution models (async task vs blocking thread); a common trait would force `async-trait` and boxing for no benefit at two variants.
- The **trait sits at the engine level**, which is where §1's pluggability requirement actually applies — the extension axis is whisper / sherpa / vosk, not remote / local.
- Each backend declares the audio format it needs:

  ```rust
  pub struct AudioSpec {
      pub sample_rate: u32,     // OpenAI: 24_000; Whisper: 16_000
      pub frame_samples: usize, // fixed frame size delivered to the backend
  }
  ```

  ```rust
  pub trait LocalEngine: Send {
      fn audio_spec(&self) -> AudioSpec;
      fn start(&mut self, cfg: &LocalSessionConfig) -> Result<(), AppError>;
      fn feed_frame(&mut self, frame: &[f32]);   // mono f32, matches audio_spec
      fn stop(&mut self);
      // events emitted via the std mpsc Sender<RealtimeEvent> given at construction
  }
  ```

#### 5.2 Audio Path (prerequisite refactor)

The capture worker currently converts to PCM16 **and base64-encodes** before the audio leaves `live_capture.rs`, sending `Vec<u8>`-derived `String`s over the channel. There is therefore no raw PCM stream to tap. Local mode requires moving that encoding down into the OpenAI backend:

```
  BEFORE                                  AFTER
  cpal callback                           cpal callback
    │ f32                                   │ f32
  capture worker                          capture worker
    downmix → resample 24k                  downmix → resample to AudioSpec.sample_rate
    → pcm16 → base64          ✗             → fixed frames of AudioSpec.frame_samples
    │                                       │
  Sender<String>                          Sender<Vec<f32>>
    │                          ┌────────────┴────────────┐
  OpenAI transport             ▼                          ▼
                        OpenAI backend             Local backend
                        pcm16 + base64 → WS        VAD + inference
```

Requirements:

- `LiveCapture::start` takes an `AudioSpec` and produces mono `f32` frames of exactly `frame_samples`, resampled from the device rate. The trailing partial frame at stop is zero-padded and flushed.
- Device configuration selection targets `AudioSpec.sample_rate` using the existing preference order (mono at target rate → any channel count at target rate → fallbacks).
- PCM16 conversion, chunking, and base64 encoding move into the OpenAI backend. The remote wire format is unchanged: 40 ms chunks of mono PCM16 at 24 kHz, base64-encoded.
- The audio channel remains a `tokio::sync::mpsc` channel for both paths. It requires no runtime: the capture worker already uses `blocking_send` from a plain thread, and the local inference thread uses `blocking_recv`. The local path therefore needs no tokio runtime at all.

#### 5.3 Event Model

Local engines re-decode a growing window and emit the **whole current hypothesis** each time, frequently revising earlier words. The existing events cannot express that:

- `SourceDelta` **appends** to a segment — repeated hypotheses concatenate (`"the qui"` + `"the quick"` → `"the quithe quick"`).
- A `None` `item_id` appends to an anonymous list that is never replaced — repeated hypotheses accumulate as separate fragments.

Therefore:

- Add one variant to `RealtimeEvent`:

  ```rust
  SourceUpdate { item_id: Option<String>, text: String }   // replaces segment text, stays provisional
  ```

- `TranscriptAssembler` gains a matching `update(item_id, text)`: it sets the segment text without sealing it. A sealed (completed) segment ignores later updates, mirroring the existing `add_delta` behaviour.
- Existing semantics are unchanged: `SourceDelta` appends, `SourceCompleted` replaces and seals.
- **Local backends must assign a stable, non-empty `item_id` to every segment** (`local-0`, `local-1`, …, monotonically increasing per session). Emitting `None` from a local backend is prohibited; `None` remains supported only for the remote events that lack an item id.
- The OpenAI path emits no `SourceUpdate` and is otherwise untouched.

#### 5.4 Transcript Rendering

The assembler continues to expose a single assembled `String`, and the transcript pane remains a plain editable text area. Provisional segments are not visually distinguished in v1 — the event model carries the distinction, so a later change can add it without another protocol change.

#### 5.5 Session Lifecycle and Threading

```
User clicks Start
  → read settings.backend
  → resolve AudioSpec from the chosen backend
  → if "openai" → verify API key → spawn tokio task (existing transport)
  → if "local"  → verify model available → spawn inference thread
  → LiveCapture::start(spec, audio_tx, event_tx)

  cpal callback ──SyncSender<Vec<f32>>──► capture worker ──Sender<Vec<f32>>──┐
                                                                              │
                     ┌────────────────────────────────────────────────────────┤
                     ▼ remote                                        local    ▼
             tokio task: pcm16+base64 → WebSocket        inference thread: VAD + engine
                     │                                                        │
                     └──────────► std mpsc Sender<RealtimeEvent> ◄────────────┘
                                             │
                                   egui poll_live_events (per frame)

User clicks Stop
  → stop capture → signal backend → drain final segment → SessionState "disconnected"
```

- Inference is blocking CPU/GPU work and must never run on the tokio runtime or the UI thread.
- The local backend sends `RealtimeEvent`s directly on the existing UI channel; the tokio→std bridge task used by the remote path is not involved.
- On stop, the local backend finalizes any open segment (runs a last inference on buffered audio, emits `SourceCompleted`) before emitting `disconnected`.

#### 5.6 Session States

Local mode emits the existing `SessionState { state }` event using these values, which the UI status mapper must render with human-readable text:

| State | Meaning |
| --- | --- |
| `local.loading` | Model is being loaded into memory |
| `connecting` | Reused for "engine ready, capture starting" so existing UI text applies |
| `local.listening` | Idle, waiting for speech (VAD silent) |
| `local.processing` | Inference in progress |
| `disconnected` | Session ended |

`LiveState` remains as-is: a local session is `LiveState::Transcribing` while running.

---

### 6. Local Engine Requirements

A local engine must:

- Operate fully offline once its model is present.
- Accept fixed-size mono f32 frames at its declared `AudioSpec`.
- Produce provisional and final text with the target latency in §6.2.
- Support a language hint plus auto-detect.
- Be cancellable: `stop()` returns promptly even if an inference is in flight.

#### 6.1 Engine Options

| Priority | Engine | Notes |
| --- | --- | --- |
| 1 | Whisper (`whisper-rs` / whisper.cpp) | Excellent accuracy, mature, Metal/CUDA acceleration. **v1 target.** |
| 2 | Sherpa-ONNX (streaming Zipformer) | True streaming, lower latency; official Rust bindings; ONNX runtime dependency |
| 3 | Vosk | Very light, true streaming, lower accuracy; third-party bindings |

v1 implements Whisper only; the trait and the `local.engine` setting make the others additive.

#### 6.2 Segmentation and VAD

Whisper is not a streaming model, so the backend supplies streaming behaviour around it using Silero VAD:

```
   frames ─► VAD ─┬─ speech onset ────► open segment (new item_id), start buffering
                  │
                  ├─ speech continues ─► every partial_interval_ms:
                  │                        re-decode the open segment → SourceUpdate
                  │                        (skip if an inference is still running)
                  │
                  ├─ silence ≥ silence_ms ─► final decode → SourceCompleted, seal segment
                  │
                  └─ segment ≥ max_segment_secs ─► force final decode → SourceCompleted,
                                                    immediately open a new segment
```

Tunable defaults:

| Parameter | Default | Rationale |
| --- | --- | --- |
| `partial_interval_ms` | 500 | Text appears during speech without saturating the CPU |
| `silence_ms` | 600 | Comparable to the remote path's `silence_duration_ms: 500` |
| `max_segment_secs` | 15 | Bounds memory and the O(n²) cost of re-decoding a growing window |
| `frame_samples` | 512 @ 16 kHz | 32 ms — the frame size Silero VAD expects |

Long silence must never be fed to the recognizer. Target latency: provisional text within ~1 s of speech starting, final text within ~0.5–2 s of speech ending.

---

### 7. Model Management

- The binary embeds a **model manifest**: for each entry, an id (`tiny.en`, `base.en`, `small`, …), display name, download URL, byte size, and SHA-256.
- On first use of local mode with a missing model, the application offers an in-app download. `reqwest`, `rfd`, and `poll-promise` are already dependencies, so download, file picking, and progress reporting need no new crates.
- Downloads are written to `~/.dictaite/models/<engine>/<model-id>/` as a `.part` file, verified against the manifest SHA-256, then atomically renamed. A failed or interrupted download leaves no file that could be mistaken for a valid model.
- Progress (bytes / total, cancellable) is shown in the Settings dialog.
- `local.model_path` lets the user point at an existing model file directly, bypassing the manifest and the download flow entirely.
- The application must never send audio to the network while in local mode. Model downloads are the only network traffic the local path may perform, and only on explicit user action.

---

### 8. UI Changes

All changes are in the Settings dialog except where noted.

- **Backend selector** — radio group: "OpenAI Realtime (transcription + translation)" / "Local model (transcription only)".
- When Local is selected, the dialog additionally shows:
  - Engine (fixed to Whisper in v1), model dropdown from the manifest, and device dropdown.
  - Model status: `Ready` / `Downloading… n%` / `Not downloaded` / `Missing at <path>`, with a Download button where applicable.
- **Main window**: the "Translate Live" checkbox is disabled and unchecked in local mode, with the §3.1 explanatory note beside it.
- **Status line**: local session states from §5.6 render as readable text ("Loading model…", "Listening…", "Transcribing…").
- Error messages must clearly distinguish network/API errors from local model/load errors.

---

### 9. Error Handling

| Condition | Behaviour |
| --- | --- |
| Model missing at session start | Refuse to start; actionable message naming the model and offering the download action |
| Model file corrupt / fails to load | `Error` event with the engine's message; session does not start; model marked not-ready |
| Requested device unavailable (no Metal/CUDA in this build or on this machine) | Fall back to CPU, emit a warning in the status line, continue |
| Binary built without the local feature (§11) | Local option is disabled in the selector with an explanation; if `settings.backend == "local"`, the app runs in remote mode and reports why rather than starting a broken session |
| Recognition failure mid-session | Emit `Error`; drop the affected segment; keep the session alive |
| Backend switched while a session is active | Cleanly stop the current session first, then apply the change |
| No API key while backend is `local` | Not an error: no warning at startup, session starts normally, TTS control disabled per §3.4 |

---

### 10. Testing Requirements

**Unit**

- Settings: round-trip of `backend` / `local`; defaults for a legacy file; preservation of unknown keys; the shared "effective translate" helper returns `false` whenever `backend == "local"`.
- Assembler: `update` replaces without sealing; `complete` replaces and seals; a sealed segment ignores later updates and deltas; ordering across interleaved segments.
- Audio: resampling to 16 kHz and exact frame sizing; zero-padded flush of the trailing partial frame; the OpenAI backend still produces identical base64 PCM16 chunks after the §5.2 refactor.
- Segmentation: onset/silence/force-cut transitions produce the expected event sequence, driven by a synthetic VAD.

**Integration**

- A **fake local engine** (scripted segments, no native dependencies, behind a test-only feature) exercises the full path — settings → capture → backend → events → assembler → UI state — and serves as the permanent stand-in for tests that must run in CI without models.
- **No-network guarantee, structurally**: assert that the local session path never constructs `RealtimeSessionConfig` and never calls `run_live_transcription` / `run_live_translation`.
- Backend selection survives an application restart.
- Save / copy / edit of the transcript behave identically in both modes.

**Manual smoke**

- Known-phrase accuracy and latency on CPU and on Metal.
- Repeated stop/start; microphone unavailable; model deleted mid-session.

---

### 11. Implementation Path

Heavy native dependencies are behind a cargo feature (e.g. `local-whisper`, with `metal` / `cuda` sub-features). The default build remains OpenAI-only and unchanged in weight; §9 covers the "feature absent" runtime state.

Suggested phasing — each phase is independently valuable and testable:

| Phase | Content | Native deps |
| --- | --- | --- |
| 1 | §5.2 audio-path refactor: `AudioSpec`, `Vec<f32>` channel, encoding moved into the OpenAI backend | none |
| 2 | §5.3 `SourceUpdate` + assembler semantics; §4 settings schema + forced-translate helper; §8 UI; fake local engine and its tests | none |
| 3 | `whisper-rs` + Silero VAD engine, segmentation (§6.2), device selection | yes (feature-gated) |
| 4 | §7 model manifest, download, progress, verification | none |

Phases 1 and 2 are worth landing regardless of engine progress: phase 1 is a straight improvement to the existing remote path, and phase 2 delivers a testable pipeline with no native build cost.

Expected difficulty concentrates in phase 3: model loading, GPU feature selection, and native library linking.

---

### 12. Future Extensions (Out of Scope for v1)

- Additional local engines selected at runtime via `local.engine`.
- Visual distinction between provisional and finalized transcript text.
- Local translation (local LLM or dedicated model).
- Local TTS.
- Custom vocabulary / hotwords.
- Speaker diarization.
- Continuous always-on listening with wake word.
