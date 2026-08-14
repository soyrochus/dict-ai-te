# dict-ai-te

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Open Source](https://img.shields.io/badge/Open%20Source-%E2%9D%A4-red?logo=github)](https://github.com/soyrochus/dict-ai-te)
[![FOSS Pluralism Manifesto](./badges/foss-pluralism-shield.svg)](./FOSS_PLURALISM_MANIFESTO.md)

**dict-ai-te** is a live dictation and live speech-translation application. Start listening, speak into your microphone, and see source text or translated text appear while the session is still running. It is available in three forms:

- A **native Rust desktop app** using egui/eframe — **the active implementation**
- A **Python GTK 4 desktop app** (Linux / macOS) — **DEPRECATED**
- A **Python Flask web app** — **DEPRECATED**

> **⚠️ The Python implementations are DEPRECATED**
>
> Both Python variants (GTK desktop and Flask web) are **deprecated and no longer developed**. They
> remain in the repository and still run, but they receive no new features and no fixes. All current
> and future work happens in the Rust application.
>
> They are OpenAI-only: on-device transcription exists solely in the Rust app. They also drop
> unknown keys when they save `~/.dictaite/settings.json`, so launching a Python variant after
> configuring the Rust app can silently discard the newer settings (`backend`, `local`). If you use
> on-device mode, do not run the Python apps against the same settings file.

All three read the same settings file (`~/.dictaite/settings.json`).

For a deep technical dive into how everything fits together, see the **[Architecture Guide](architecture-guide.md)**.

| Python GTK (Ubuntu) | Python GTK (macOS) |
| :----: | :---: |
| ![dict-ai-te on Ubuntu](img/dict-ai-te-ubuntu.png) | ![dict-ai-te on macOS](img/dict-ai-te-mac.png) |

| Python Web (Flask) | Rust (egui; shown on Ubuntu) |
| :----:      |:---:            |
| ![dict-ai-te on Web](img/dict-ai-te-web.png) |![dict-ai-te as Rust App](img/dict-ai-te-rust.png) |

## Features

- **Live transcription** — audio streams directly to OpenAI Realtime (`gpt-4o-transcribe`) and transcribed words appear as you speak.
- **On-device transcription (Rust, opt-in)** — build with `local-whisper` to run Whisper plus Silero VAD locally, with no API key and no speech-related network traffic. Requires CMake and libclang; see [Optional local transcription](#optional-local-transcription).
- **Live translation** — optionally route the session through the `gpt-realtime` translation endpoint; source and translated transcripts accumulate simultaneously.
- **TTS playback** — read back the transcript in a chosen voice via the OpenAI TTS API (`tts-1`).
- **Audio level meter** — visual level bar during both recording and playback.
- **Elapsed-time timer** — shows how long the current session has been running.
- **Origin and target language selection** — choose from 20+ languages; auto-detect is the default.
- **Save, copy, edit** — save the transcript as a `.txt` file, copy it to the clipboard, or edit it directly in the text area.
- **Shared settings** — default language, voice preferences, and translate-by-default are stored in `~/.dictaite/settings.json` and shared across all app variants.
- **Managed local models** — download a verified model in Settings or choose an existing whisper.cpp GGML model file.

## Quick Start

### Prerequisites

- A Rust toolchain (stable). Python 3.12+ only if you intend to run the deprecated Python variants.
- Linux or macOS (Windows supported for the Rust app).
- An OpenAI API key with access to the Realtime API — required for cloud mode, **not** required for
  on-device transcription.
- For on-device transcription only: **CMake**, a **C/C++ compiler**, and **libclang**. See
  [Optional local transcription](#optional-local-transcription).

### 1. Clone

```bash
git clone https://github.com/soyrochus/dict-ai-te.git
cd dict-ai-te
```

### 2. Set your API key

```bash
export OPENAI_API_KEY=your_key_here
# or create a .env file:
echo "OPENAI_API_KEY=your_key_here" > .env
```

### 3. Run

**Rust — the active implementation:**

```bash
cargo run --release
```

**Python GTK desktop** *(DEPRECATED — no longer developed)*:

```bash
uv sync
python -m dictaite          # or: bin/dictaite
```

**Python web UI** *(DEPRECATED — no longer developed)*:

```bash
uv sync
bin/dictaite-web            # then open http://localhost:8080
```

---

## Installation — Python (DEPRECATED)

> **This section covers the deprecated Python variants.** They are kept for reference and still run,
> but receive no further development. For on-device transcription and all current work, use the Rust
> app instead.

### System dependencies

**Linux (Ubuntu/Debian):**

```bash
sudo apt update
sudo apt install -y \
  libgtk-4-dev libgirepository-2.0-dev libcairo2-dev pkg-config \
  python3-dev python3-gi python3-gi-cairo gir1.2-gtk-4.0 \
  libportaudio2
```

**macOS:**

```bash
brew install gtk4 pygobject3 portaudio
```

### Python dependencies

```bash
uv venv .venv
source .venv/bin/activate
uv sync
```

### Running

```bash
# GTK desktop app
python -m dictaite
# or
bin/dictaite

# Web UI
bin/dictaite-web
# or
uv run -m dictaite.ui_web.app
```

Visit `http://localhost:8080` for the web UI.

---

## Installation — Rust

### System dependencies (Ubuntu 24.04+)

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config libssl-dev \
  libasound2-dev libjack-jackd2-dev \
  libx11-dev libxi-dev libxcb1-dev libxcb-render0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libwayland-dev libgl1-mesa-dev libudev-dev \
  libvulkan1 vulkan-tools mesa-vulkan-drivers libvulkan-dev \
  libgtk-3-dev \
  xclip wl-clipboard
```

**Why each dependency is needed:**

- `build-essential`, `pkg-config` — C compiler and metadata tooling
- `libssl-dev` — TLS (used by `reqwest` for HTTPS)
- `libasound2-dev`, `libjack-jackd2-dev` — audio I/O (`cpal` / `rodio`)
- X11/Wayland/GL stack — rendering (`eframe` / `wgpu`)
- `libudev-dev` — device enumeration (`wgpu`)
- Vulkan stack — GPU rendering backend for `wgpu`
- `libgtk-3-dev` — native file dialogs (`rfd`)
- `xclip`, `wl-clipboard` — clipboard integration (`arboard`)

### Build and run

```bash
cargo build --release
cargo run --release
# or
./target/release/dict_ai_te
```

### Optional local transcription

On-device transcription is opt-in at compile time, because `whisper-rs` (whisper.cpp) and Silero VAD
add native build dependencies that the default build does not need. Without these features the app
builds exactly as before and runs cloud-only.

#### Additional build requirements

| Requirement | Why |
| --- | --- |
| **CMake** | `whisper-rs-sys` compiles whisper.cpp through the `cmake` crate |
| **C/C++ compiler** | building whisper.cpp itself |
| **libclang** | `bindgen` generates the whisper.cpp FFI bindings at build time |
| **Network access on first build** | `ort-sys` (ONNX Runtime, used by Silero VAD) downloads prebuilt binaries |
| **CUDA toolkit** | only for `--features cuda`; provides `nvcc` |

**Linux (Ubuntu/Debian)** — in addition to the system dependencies listed above:

```bash
sudo apt update
sudo apt install -y cmake clang libclang-dev
```

**macOS:**

```bash
xcode-select --install     # C/C++ toolchain, if not already present
brew install cmake
```

Metal acceleration needs no further packages on macOS. The first build of a local feature compiles
whisper.cpp from source and downloads ONNX Runtime, so expect it to take noticeably longer than a
normal build.

#### Building

```bash
# CPU (Linux and macOS)
cargo run --release --features local-whisper

# Apple Metal (macOS only; other platforms fall back to CPU)
cargo run --release --features metal

# NVIDIA CUDA (requires the CUDA toolkit)
cargo run --release --features cuda
```

`metal` and `cuda` both imply `local-whisper`; pick one. If the requested accelerator is unavailable
at runtime, the app falls back to CPU and says so in the status line.

#### Using on-device mode

Open Settings, select **Local model**, and download `base.en` or `small` — or point the app at an
existing whisper.cpp GGML model file. Models are stored below
`~/.dictaite/models/whisper/<model-id>/`, and `DICTAITE_HOME` changes the application-home prefix.

On-device mode is transcription-only: live translation is disabled while it is active, and no API
key is required. TTS currently still uses the OpenAI API when `OPENAI_API_KEY` is present — see
[Planned](#planned) below.

### Planned

On-device speech synthesis is specified but not yet implemented. It will make the backend setting
all-or-nothing: in on-device mode, transcript playback will use a local voice and nothing will be
sent to a remote service. This removes today's behaviour where local mode still calls the OpenAI TTS
endpoint if a key is configured. See
[`specs/2026-08-14-local-speech-synthesis.md`](specs/2026-08-14-local-speech-synthesis.md).

---

## Web UI reference (DEPRECATED)

> **The Flask web app is deprecated and no longer developed.** This section documents its existing
> protocol for reference only.

The Flask interface mirrors the GTK and Rust layout using TailwindCSS and vanilla JavaScript. Audio is captured in the browser, converted to mono 24 kHz PCM16, base64-encoded, and sent to the server over a WebSocket. The server holds the API key and relays events back.

### WebSocket endpoints

| Endpoint | Description |
| --- | --- |
| `GET /ws/live/transcribe` | Live transcription session |
| `GET /ws/live/translate` | Live translation session |

**Session protocol:**

1. Connect to the endpoint.
2. Send `{ "type": "start", "source_language": "<code>", "target_language": "<name>" }`.
3. Send audio chunks: `{ "type": "audio", "audio": "<base64-pcm16>" }`.
4. Receive events: `{ "type": "source_delta" | "translation_delta" | "session_state" | "error", "text": "...", "state": "...", "error": "..." }`.
5. Send `{ "type": "stop" }` to end.

### REST endpoints

| Endpoint | Method | Description |
| --- | --- | --- |
| `/api/settings` | GET / POST | Read / write settings |
| `/api/tts-test` | POST | Generate voice preview (`{ gender, text, voice? }` → `audio/wav`) |
| `/api/health` | GET | Readiness probe |

### Browser shortcuts

| Key | Action |
| --- | --- |
| `Space` | Start / stop recording (when textarea is not focused) |
| `Ctrl/Cmd+C` | Copy transcript |
| `Ctrl/Cmd+S` | Save transcript as `.txt` |

---

## Configuration

Settings are stored in `~/.dictaite/settings.json`. You can also override the config directory with the `DICTAITE_HOME` environment variable.

The Rust app preserves keys it does not recognise when it saves. The deprecated Python variants do
not: running them against the same file discards the newer `backend` and `local` settings.

Legacy TOML configs at `~/.config/dict-ai-te/dict-ai-te_config.toml` are migrated automatically on first launch.

An OpenAI API key is required for the remote backend, translation, and TTS. It is not required for
local transcription. It can be set via:

- A `.env` file in the project root: `OPENAI_API_KEY=your_key_here`
- The environment variable `OPENAI_API_KEY`

---

## Architecture

See the **[Architecture Guide (markdown)](architecture-guide.md)** or **[Architecture Guide (HTML)](architecture-guide.html)** for a detailed technical walk-through covering:

- How the implementations relate to each other (note: the Python sections describe the deprecated variants)
- The audio capture pipeline (cpal / sounddevice / browser MediaRecorder)
- The OpenAI Realtime session protocol (transcription and translation endpoints)
- Event parsing and normalization
- The transcript assembler
- TTS and audio playback
- Settings storage and migration
- End-to-end data flow diagrams
- Remaining implementation differences between Rust and Python

---

## Principles of Participation

Everyone is invited and welcome to contribute: open issues, propose pull requests, share ideas, or help improve documentation.
Participation is open to all, regardless of background or viewpoint.

This project follows the [FOSS Pluralism Manifesto](./FOSS_PLURALISM_MANIFESTO.md), which affirms respect for people, freedom to critique ideas, and space for diverse perspectives.

---

## License and Copyright

Copyright (c) 2025, 2026 Iwan van der Kleijn

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
