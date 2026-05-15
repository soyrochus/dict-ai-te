# Architecture

The project is split into three major layers to enable multiple user interfaces to share the same business logic.

## Core layer (`dictaite_core`)

* **Configuration** – `dictaite_core.config.Settings` encapsulates shared preferences (default languages, voices, translation
  behaviour). `load_settings()` and `save_settings()` persist the JSON file at `~/.dictaite/settings.json`. Legacy TOML
  configuration is migrated automatically the first time the new code runs.
* **Services** – `dictaite_core.services` exposes pure functions:
  * `transcribe(audio: bytes, mimetype: str, language: str | None) -> str`
  * `translate(text: str, target_lang: str) -> str`
  * `synthesize_speech(text: str, voice: str | None) -> bytes`

  These helpers wrap the OpenAI SDK, hide MIME conversion, enforce limits (≤120 seconds, allowed MIME types) and return
  normalized text. They contain no GUI dependencies and can be reused from any environment.

## GTK UI (`dictaite.ui_gtk`)

The GTK4 desktop application imports only the services from `dictaite_core`. Recording/playback is still handled locally via
`sounddevice`, while all cloud interactions go through the service layer. `SettingsDialog` reads/writes the shared settings via
`load_settings()` / `save_settings()` so the desktop and web front-ends remain in sync.

## Flask web UI (`dictaite.ui_web`)

The Flask app is built with the application-factory pattern. `dictaite.ui_web.app:create_app()` registers two blueprints:

* `pages` for `GET /` (recorder) and `GET /settings`.
* `api` for JSON endpoints (`/api/transcribe`, `/api/tts-test`, `/api/settings`, `/api/health`).

The HTML lives in Jinja templates with TailwindCSS for layout. JavaScript (vanilla) handles MediaRecorder capture, waveform
metering, uploads and TTS playback. Settings persist by calling the shared configuration helpers.

## Adding another UI

1. Import what you need from `dictaite_core` (`load_settings`, `transcribe`, `translate`, `synthesize_speech`).
2. Keep the UI self-contained; never import `openai` directly.
3. Reuse `dictaite/ui_common.py` for language and voice lists to match the existing front-ends.
4. Persist settings using `save_settings()` so all experiences share the same defaults.
