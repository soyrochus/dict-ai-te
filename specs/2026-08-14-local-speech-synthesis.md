# Specification: Local Speech Synthesis for dict-ai-te

**Version:** 1.0
**Status:** Draft for implementation
**Scope:** Make the existing `backend` setting an all-or-nothing choice by adding on-device text-to-speech, so that local mode never sends anything to a remote service.

> **Note on implementations:** This specification covers the Rust implementation only. The Python implementations (GTK desktop and Flask web) are deprecated and out of scope — they are not modified by this work.

**Companion document:** `specs/2026-08-14-local-transcription-backend.md` (v2.0), which introduced the `backend` setting, the local Whisper engine, and the model management infrastructure this change builds on.

---

### 1. Problem

The local transcription backend keeps audio on the machine, but the speak-transcript control still calls the OpenAI TTS endpoint whenever an API key is present. A user who selects local mode for privacy therefore keeps their audio on-device while the transcript — the distilled content of that same audio — is sent to a third party, with nothing in the interface indicating it.

The current specification only addresses the case where no API key is configured, in which the control is disabled. With a key present, the remote call happens silently.

### 2. Goals

- Make `backend` a genuine boundary: `"openai"` means every feature may use the network; `"local"` means none of them do.
- Provide on-device speech synthesis of comparable convenience to the remote path — same control, same voices concept, same playback.
- Work equally well on macOS and Linux, with one code path rather than per-platform behaviour.
- Reuse the model management, download, and verification infrastructure already built for Whisper models.
- Add no new runtime dependency beyond what the local feature already pulls in.
- Keep the default build free of local support, as today.

### 3. Non-Goals (v1)

- Voices for languages other than English (see §4.4).
- Mixing backends — local transcription with remote speech, or the reverse (see §4.1).
- Streaming or incremental synthesis; synthesis is one-shot per request, as the remote path already is.
- Voice cloning, custom voices, or user-supplied model training.
- SSML, prosody controls, or speed/pitch adjustment.
- Replacing the OpenAI TTS path, which remains unchanged in remote mode.
- Any change to the deprecated Python tree.

---

### 4. User-Facing Behavior

#### 4.1 One switch governs everything

The `backend` setting becomes all-or-nothing. There is no per-feature backend choice, and no configuration in which one feature is remote while another is local.

| `backend` | Transcription | Translation | Speech |
| --- | --- | --- | --- |
| `"openai"` | OpenAI Realtime | OpenAI Realtime | OpenAI TTS |
| `"local"` | Whisper, on-device | unavailable | Piper, on-device |

A user who wants the higher-quality remote voice switches the backend, which makes the boundary explicit by construction rather than offering a setting that quietly leaks.

Because the setting now governs three features rather than one, the Settings control is relabelled from a transcription-backend selector to a processing-mode selector:

- **Cloud (OpenAI)** — transcription, translation, and speech via the OpenAI API.
- **On-device** — transcription and speech run locally; nothing leaves the machine.

#### 4.2 Behaviour change

In local mode, the speak-transcript control no longer calls the OpenAI TTS endpoint, regardless of whether an API key is configured. This removes an existing capability from local-mode users who have a key; the removal is the point of this change.

#### 4.3 Speaking a transcript locally

1. The user presses the speak control with a transcript present.
2. The application selects a local voice from the configured gender preference, exactly as it selects an OpenAI voice today.
3. Synthesis runs in the background; the status line reports progress.
4. The resulting audio plays through the existing player, with the same elapsed/duration display and the same clip caching.
5. No network request is made.

The voice preview in the Settings dialog behaves the same way, using the local engine when the backend is local.

#### 4.4 Language

- v1 ships English voices only.
- When the transcript's language is English, or the Origin language is English or auto-detect, synthesis proceeds normally.
- When the Origin language is set to a language with no installed local voice, the speak control is disabled with the message *"No local voice available for <language>. Switch to Cloud mode or select English."* The session and transcript are unaffected.
- The manifest and voice resolution are designed so that adding voices for further languages is a manifest change plus downloads, not a code change.

---

### 5. Configuration & Settings

The `backend` field is unchanged in name, type, and default; only its meaning widens. Speech configuration is nested under `local.speech`:

```json
{
  "backend": "local",
  "female_voice": "nova",
  "male_voice": "onyx",

  "local": {
    "engine": "whisper",
    "model": "base.en",
    "model_path": null,
    "device": "auto",

    "speech": {
      "engine": "piper",
      "female_voice": "en_US-amy-medium",
      "male_voice": "en_US-ryan-high",
      "voice_path": null
    }
  }
}
```

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `local.speech.engine` | string | `"piper"` | Speech engine id; extensible |
| `local.speech.female_voice` | string | `"en_US-amy-medium"` | Voice id from the voice manifest |
| `local.speech.male_voice` | string | `"en_US-ryan-high"` | Voice id from the voice manifest |
| `local.speech.voice_path` | string \| null | `null` | Absolute path to a voice model directory; overrides both voice ids |

**Rules:**

- The existing top-level `female_voice` / `male_voice` remain the OpenAI voice ids and are used only in remote mode. Local voice ids live under `local.speech` because the two sets share no identifiers.
- `VoiceGender` remains the user-facing concept in both modes. The gender preference selects which of the two configured voice ids is used for the active backend.
- A settings file written before this change loads with the defaults above and requires no migration. The nesting under `local` is deliberate: no shipped field is renamed.
- Unknown keys continue to be preserved across a save, as established by the transcription change.

---

### 6. Architecture

#### 6.1 Engine abstraction

Speech synthesis mirrors the structure the transcription work established: a backend choice expressed as a branch, and a trait at the level where engines are actually interchangeable.

```
   speak(text, gender)
          │
          ├── backend = openai ──► OpenAiClient::text_to_speech  (unchanged)
          │
          └── backend = local  ──► Box<dyn SpeechEngine>
                                        │
                                   PiperEngine  │  (future: other ONNX voices)
```

```rust
pub struct SpeechAudio {
    pub samples: Vec<f32>,   // mono
    pub sample_rate: u32,
}

pub trait SpeechEngine: Send {
    fn synthesize(&self, text: &str, voice: &ResolvedVoice) -> Result<SpeechAudio, AppError>;
}
```

#### 6.2 Integration point

The existing TTS request path already isolates the remote call in a single background closure that produces an `AudioClip`. Everything downstream — clip caching keyed by voice, the audio player, elapsed and duration reporting, the preview flow — is backend-agnostic and is not modified:

```
   text + voice ──► [ OpenAI  |  Piper ] ──► samples ──► AudioClip ──► AudioPlayer
                    └──── the only branch ────┘
```

Requirements:

- Synthesis runs on the existing background-task mechanism, not on the UI thread. Local synthesis is one-shot; no streaming, partial results, or new event variants are introduced.
- `AudioClip` gains a constructor from raw mono `f32` samples plus a sample rate, if it does not already have one. The existing WAV-bytes constructor continues to serve the remote path.
- The clip cache key includes the backend as well as the voice id, so a cached remote clip is never replayed for a local voice or the reverse.

#### 6.3 Engine choice

The engine is **Piper** (ONNX VITS voices), for reasons that follow from the constraints rather than from preference:

- **One code path on both platforms.** OS-native synthesis would mean AVSpeechSynthesizer on macOS and speech-dispatcher on Linux, where it is frequently absent and typically falls back to espeak-ng. Local mode would work well on one target platform and poorly on the other.
- **No new runtime.** The ONNX runtime (`ort`) is already a transitive dependency of the `silero` VAD crate under the existing local feature. Piper runs on it.
- **The model infrastructure exists.** Manifest, download with progress, SHA-256 verification, `.part`-then-atomic-rename, and the `~/.dictaite/models/<engine>/<id>/` layout were built for Whisper models and extend to voices unchanged. Voices are 20–60 MB, well under the size of the models that infrastructure already handles.

The cost accepted in exchange: Piper requires **espeak-ng** for text-to-phoneme conversion before inference. This is an additional native library, tolerable because the local feature already requires a native toolchain for whisper.cpp, and therefore does not change the class of build. It is a phonemizer only; synthesis itself is the ONNX model.

#### 6.4 Feature gating

Local speech sits behind its own cargo feature, independent of `local-whisper`:

- A build without the speech feature supports local transcription with no speak control, which is a coherent state rather than an error.
- A build with neither feature is the default OpenAI-only build, unchanged in weight and toolchain requirements.
- Acceleration sub-features follow the existing pattern where the runtime supports them.

---

### 7. Voice Management

Voices reuse the model management capability rather than introducing a parallel one.

- The embedded manifest gains voice entries, each carrying an id, display name, language, gender, download URLs for the ONNX model and its JSON configuration, byte sizes, SHA-256 checksums, and **license**.
- Only permissively licensed voices are listed. The license of each entry is recorded in the manifest and shown in the Settings dialog, so a download never places a user under terms they have not seen.
- Voices install to `~/.dictaite/models/piper/<voice-id>/`, honouring `DICTAITE_HOME`, with the same `.part`-then-verify-then-rename sequence used for transcription models.
- Voice availability states mirror model states: ready, downloading with progress, not downloaded, or missing at the configured path.
- `local.speech.voice_path` accepts a directory containing a voice model and its configuration, bypassing the manifest entirely.

---

### 8. UI Changes

- The backend selector is relabelled per §4.1 and its help text states that on-device mode covers transcription and speech.
- The Settings dialog gains a local voice section, shown when the backend is local: female voice, male voice, per-voice status with a download action, and the license of each listed voice.
- Voice preview works in both modes, using whichever engine the current backend selects.
- The speak control is disabled, with a specific explanation, when: the build lacks the speech feature; no voice is installed; or the transcript language has no available voice (§4.4). The reason is always stated rather than left to a generic failure.
- Error messages distinguish voice download and load failures from synthesis failures.

---

### 9. Error Handling

| Condition | Behaviour |
| --- | --- |
| No voice installed in local mode | Speak control disabled; message names the configured voice and offers the download action |
| Voice files present but fail to load | Error surfaced with the engine's message; voice marked not ready; no playback attempted |
| Phonemization fails for the given text | Error reported for that request; the transcript and session are unaffected |
| Synthesis fails mid-request | Error reported; no partial audio is played or cached |
| Language has no available voice | Speak control disabled with the §4.4 message |
| Build without the speech feature | Speak control hidden or disabled with an explanation; local transcription continues to work |
| API key present in local mode | Irrelevant: no remote call is made in local mode under any circumstances |

---

### 10. Testing Requirements

**Unit**

- Settings: round-trip of `local.speech`; a pre-change file loads with defaults; unknown-key preservation still holds.
- Voice resolution: gender preference selects the correct voice id per backend; `voice_path` takes precedence over manifest ids; a language with no voice resolves to unavailable rather than to a wrong voice.
- Clip cache: a cached clip is never reused across backends for the same voice id.
- Manifest: voice entries parse with their licenses; checksum mismatch and interrupted download are handled as for transcription models.

**Integration**

- A fake `SpeechEngine` returning scripted samples, with no native dependencies, exercises speak → synthesis → clip → player without a voice model present.
- **No-network guarantee, structurally**: assert that in local mode the speak path never constructs an OpenAI client and never calls `text_to_speech`, mirroring the equivalent assertion on the transcription path.
- Backend switching selects the corresponding engine for the next speak request.

**Manual smoke**

- Speech quality and latency for a representative transcript on macOS and on Linux, on CPU.
- Voice download, cancellation, checksum failure, and the `voice_path` override.
- The disabled-control paths: no voice installed, non-English origin language, build without the feature.

---

### 11. Implementation Path

| Phase | Content | Native deps |
| --- | --- | --- |
| 1 | Enforce the binary boundary: local mode never calls `text_to_speech`; speak control disabled with an explanation; selector relabelled | none |
| 2 | `SpeechEngine` trait, backend branch in the TTS request path, `AudioClip` from raw samples, backend-aware clip cache, fake engine and tests | none |
| 3 | Piper engine: ONNX inference via the existing runtime, espeak-ng phonemization, voice loading, behind the speech feature | yes (feature-gated) |
| 4 | Voice manifest entries with licenses, download and status UI reusing the model management capability | none |

Phase 1 closes the privacy gap on its own and is worth landing first even if the engine work slips; it removes a capability rather than adding one, so it should be released with a note in the changelog. Phases 2 and 4 require no native toolchain. Phase 3 carries the espeak-ng dependency and the platform verification burden.

---

### 12. Future Extensions (Out of Scope for v1)

- Voices for additional languages, driven by manifest entries and the Origin language selector.
- Additional local speech engines selectable through `local.speech.engine`.
- Streaming synthesis, so playback can begin before the full transcript is rendered.
- Speed, pitch, and prosody controls.
- Automatic voice selection from the detected transcript language rather than the configured origin language.
