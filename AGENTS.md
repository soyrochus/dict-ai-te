**Purpose**
- This file guides future contributors and agents on how dict‑ai‑te is structured, how to add features, and how to work safely across both the GTK desktop UI and the NiceGUI web UI.

**Core Requirements**
- Transcription model: `gpt-4o-transcribe` (not Whisper).
- Translation model: `gpt-5-mini-2025-08-07` (not gpt‑3.5‑turbo).
- Always format output text: clear spacing, correct punctuation, blank lines between paragraphs.
- Settings: default origin language, default target language, translation enabled flag, female/male voice values with preview buttons.
- Config file: `$HOME/.config/dict-ai-te/dict-ai-te_config.toml` (TOML). Create parents/file if missing; ignore read errors; use defaults.

**Architecture Overview**
- Backend API: `dictaite/api.py`
  - `get_openai_client()`: loads API key from env/.env.
  - `transcribe_file(file, language)`: uses `gpt-4o-transcribe`.
  - `translate_text(text, src_name, tgt_name)`: uses `gpt-5-mini-2025-08-07`.
  - `format_structured_text(text)`: normalizes whitespace and paragraphs.
  - `synthesize_speech_wav(text, voice)`: returns WAV bytes via OpenAI TTS.
- Shared constants: `dictaite/constants.py`
  - `LANGUAGES`, `LANGUAGE_NAME`, `FEMALE_VOICES`, `MALE_VOICES`, `VOICE_SAMPLE_TEXT`.
- Configuration: `dictaite/config.py`
  - Dataclass with defaults; `load()` ignores missing/invalid files; `save()` writes TOML at the XDG path above.
- GTK UI: `dictaite/__main__.py`
  - Desktop app (GTK 4). Uses shared constants and backend API functions.
- Web UI (NiceGUI): `dictaite/web.py`
  - Alternative web interface. Optional dependency (`pip install nicegui`).
  - Reuses the same backend API and config.

**CLI Entrypoint**
- Binary script: `bin/dictaite` runs the module (`uv run -m dictaite`).
- Launch GTK (default): `python -m dictaite` or `bin/dictaite`.
- Launch Web (NiceGUI): `python -m dictaite --web [--host HOST --port PORT]`.
- The `--web` flag is optional; if NiceGUI is missing, the app prints an install hint.

**GTK Frontend Expectations**
- Translate control is a compact `Gtk.Switch` aligned start (no stretching).
- Settings dialog includes:
  - Default origin language and default target language.
  - “Translate by default” switch enabling/disabling the target selector.
  - Female and Male voice selects, each with a “Play” preview button.
- When translation is enabled in the main window, the target language combo becomes sensitive.

**NiceGUI Frontend Expectations**
- Controls mirror GTK: origin language, translate switch, target language, female/male voice selects with Play.
- Transcript area with buttons to Copy, Download, and Play Transcript (TTS).
- Upload audio (accept `audio/*`). Optional in‑browser recording via MediaRecorder that feeds the upload flow.
- Settings can be saved, persisting to the same TOML config.

**Text Formatting Guarantee**
- Always run transcription and translation results through `format_structured_text()`.
- Ensure output contains blank lines between paragraphs and normalized spacing/punctuation.
- Any new feature producing user-visible text must call the formatter before rendering or saving.

**Configuration Details**
- Path: `$HOME/.config/dict-ai-te/dict-ai-te_config.toml`.
- Defaults: `default_language='default'`, `default_target_language='en'`, `translation_enabled=false`, `female_voice='nova'`, `male_voice='onyx'`.
- On `load()`: if the file is missing, unreadable, or invalid TOML, silently fall back to defaults.
- On `save()`: create parent directories if needed; write TOML.

**Models and Prompts**
- Transcription model: `gpt-4o-transcribe` with a prompt that encourages clear paragraphs and punctuation.
- Translation model: `gpt-5-mini-2025-08-07` with a prompt to return only translated text, formatted in paragraphs separated by blank lines.
- Keep translation temperature conservative (e.g., `0.2`) for fidelity.

**Development Guidelines**
- Reuse backend helpers in `dictaite/api.py` instead of duplicating logic in UIs.
- Reuse `LANGUAGES` and voice lists from `dictaite/constants.py`; don’t redefine.
- Keep NiceGUI dependency optional and guarded; do not break GTK when NiceGUI isn’t installed.
- Respect config defaults and ignore read errors; never crash on missing config.
- Keep UI code minimal and focused on wiring; avoid embedding business logic in UI layers.

**Testing Suggestions**
- Add unit tests for:
  - `format_structured_text()` (whitespace normalization, paragraph splitting).
  - Config load/save round‑trip (with missing/invalid files).
  - Web upload handler (inject small audio fixtures; assert formatted output).
- Favor small, direct tests near the code they validate; avoid broad integration tests unless necessary.

**Runbook**
- GTK: `bin/dictaite`
- Web: `bin/dictaite --web --host 127.0.0.1 --port 8080`
- Install optional web dep: `uv add nicegui`

**Notes for Future Changes**
- If adding new language/voice options, update `dictaite/constants.py` and ensure both UIs pick them up.
- Any new feature producing text must pass results through `format_structured_text()` before display or save.
- Keep the `--web` flag backward compatible; never break the default GTK path.

