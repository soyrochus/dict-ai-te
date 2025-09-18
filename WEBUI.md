# Web UI guide

The Flask interface mirrors the GTK layout using TailwindCSS and vanilla JavaScript. It supports recording, live level metering,
Whisper transcription, optional translation, text-to-speech previews, download/copy helpers and keyboard shortcuts.

## Prerequisites

* Python 3.12+
* `ffmpeg` (required by `pydub` to transcode browser recordings to WAV)
* OpenAI API key exported as `OPENAI_API_KEY` or placed in a `.env`

Install the project and extras:

```bash
uv sync --extra ui-web
```

## Running the server

Use the convenience script or run the module directly:

```bash
bin/dictaite-web
# or
uv run -m dictaite.ui_web.app --host 0.0.0.0 --port 5000
```

Navigate to `http://localhost:5000`. The browser will prompt for microphone permissions when you start recording. MediaRecorder
produces `webm/ogg` blobs which are transcoded to 16 kHz WAV on the server before reaching Whisper.

## API overview

* `POST /api/transcribe` – multipart upload with `audio`, optional `language`, `translate`, `target_lang`. Returns JSON with
  `text`, `translatedText?`, `durationMs`.
* `POST /api/tts-test` – JSON `{ gender, text, voice? }`, returns `audio/wav` preview bytes.
* `POST /api/settings` – JSON payload to persist shared settings; `GET /api/settings` fetches current values.
* `GET /api/health` – simple readiness probe.

CORS is disabled by default. Enable it via `DICTAITE_ENABLE_CORS=true` and `DICTAITE_CORS_ORIGIN=...` in the Flask configuration
if embedding in another domain. A placeholder hook (`DICTAITE_RATE_LIMITER`) is left in `app.py` to plug in your preferred rate
limiter middleware.

## Browser controls

* `Space` – start/stop recording (ignored when the textarea is focused).
* `Ctrl/Cmd+C` – copy transcript.
* `Ctrl/Cmd+S` – download transcript as `.txt`.

The **Play** button synthesizes audio for the current transcript via `/api/tts-test` using the chosen voice gender.

## Settings synchronisation

Settings are stored in `~/.dictaite/settings.json` and shared with the GTK application. The web form uses the same voices and
language lists defined in `dictaite/ui_common.py`. Use the Play buttons to preview voice choices before saving.
