"""Flask web UI for dict-ai-te."""

from __future__ import annotations

import io
import asyncio
import json
import logging
from pathlib import Path
from typing import Any

from flask import (
    Blueprint,
    Flask,
    Response,
    current_app,
    jsonify,
    render_template,
    request,
    send_file,
)
try:
    from flask_sock import Sock
except ImportError:  # pragma: no cover - dependency guard for old environments
    Sock = None  # type: ignore[assignment]

from dictaite_core import Settings, load_settings, save_settings
from dictaite_core.realtime import (
    LiveMode,
    OpenAIRealtimeClient,
    RealtimeClientConfig,
    RealtimeClientError,
    RealtimeEventType,
)
from dictaite_core.services import synthesize_speech

from ..ui_common import FEMALE_VOICES, LANGUAGES, LANGUAGE_NAME, MALE_VOICES, VOICE_SAMPLE_TEXT

LOGGER = logging.getLogger(__name__)
TEMPLATES = Path(__file__).with_suffix("").parent / "templates"
STATIC = Path(__file__).with_suffix("").parent / "static"

PAGES = Blueprint("pages", __name__)
API = Blueprint("api", __name__, url_prefix="/api")


def create_app(config: dict[str, Any] | None = None) -> Flask:
    app = Flask(
        __name__,
        template_folder=str(TEMPLATES),
        static_folder=str(STATIC),
    )
    app.config.update(
        MAX_CONTENT_LENGTH=20 * 1024 * 1024,  # generous safety limit (~20 MB)
        DICTAITE_ENABLE_CORS=False,
        DICTAITE_CORS_ORIGIN="*",
    )
    if config:
        app.config.update(config)

    app.register_blueprint(PAGES)
    app.register_blueprint(API)
    if Sock is not None:
        _register_live_websockets(app)
    else:
        LOGGER.warning("flask-sock is not installed; live WebSocket routes are unavailable")

    @app.before_request
    def apply_rate_limit() -> None:  # pragma: no cover - placeholder hook
        limiter = app.config.get("DICTAITE_RATE_LIMITER")
        if limiter:
            limiter()

    @app.after_request
    def apply_cors_headers(response: Response) -> Response:
        if app.config.get("DICTAITE_ENABLE_CORS"):
            response.headers.setdefault("Access-Control-Allow-Origin", app.config["DICTAITE_CORS_ORIGIN"])
            response.headers.setdefault("Access-Control-Allow-Headers", "Content-Type")
            response.headers.setdefault("Access-Control-Allow-Methods", "GET,POST,OPTIONS")
        return response

    with app.app_context():
        current_app.config["DICTAITE_SETTINGS"] = load_settings()
        LOGGER.info("Loaded settings for web UI")

    return app


def _register_live_websockets(app: Flask) -> None:
    sock = Sock(app)

    @sock.route("/ws/live/transcribe")
    def live_transcribe(ws) -> None:
        _run_live_socket(ws, LiveMode.TRANSCRIBE)

    @sock.route("/ws/live/translate")
    def live_translate(ws) -> None:
        _run_live_socket(ws, LiveMode.TRANSLATE)


def _run_live_socket(ws: Any, mode: LiveMode) -> None:
    target_language = None
    source_language = None

    first_message = ws.receive()
    if first_message:
        try:
            payload = json.loads(first_message)
        except json.JSONDecodeError:
            payload = {}
        if payload.get("type") == "start":
            target_code = _normalize_language(payload.get("target_language"))
            target_language = LANGUAGE_NAME.get(target_code, target_code) if target_code else None
            source_language = _normalize_language(payload.get("source_language"))
        else:
            ws.send(json.dumps({"type": "error", "error": "Expected start message"}))
            return

    client = OpenAIRealtimeClient(
        RealtimeClientConfig(
            mode=mode,
            target_language=target_language,
            source_language=source_language,
        )
    )

    async def run_session() -> None:
        queue: asyncio.Queue[str | None] = asyncio.Queue(maxsize=8)

        async def audio_iter():
            while True:
                chunk = await queue.get()
                if chunk is None:
                    break
                yield chunk

        async def browser_reader() -> None:
            while True:
                message = await asyncio.to_thread(ws.receive)
                if message is None:
                    break
                try:
                    payload = json.loads(message)
                except json.JSONDecodeError:
                    continue
                msg_type = payload.get("type")
                if msg_type == "audio":
                    audio = payload.get("audio")
                    if isinstance(audio, str) and audio:
                        await queue.put(audio)
                elif msg_type in {"stop", "close"}:
                    break
            await queue.put(None)

        async def emit(event) -> None:
            ws.send(
                json.dumps(
                    {
                        "type": event.type.value,
                        "text": event.text,
                        "item_id": event.item_id,
                        "state": event.state,
                        "error": event.error,
                    }
                )
            )

        await asyncio.gather(browser_reader(), client.run(audio_iter(), emit))

    try:
        asyncio.run(run_session())
    except RealtimeClientError as exc:
        ws.send(json.dumps({"type": RealtimeEventType.ERROR.value, "error": str(exc)}))
    except Exception as exc:  # pragma: no cover - network/session runtime path
        LOGGER.exception("Live WebSocket session failed")
        ws.send(json.dumps({"type": RealtimeEventType.ERROR.value, "error": str(exc)}))


@PAGES.get("/")
def index() -> str:
    settings = current_settings()
    return render_template(
        "index.html",
        languages=LANGUAGES,
        target_languages=LANGUAGES[1:],
        female_voices=FEMALE_VOICES,
        male_voices=MALE_VOICES,
        settings=settings,
    )


@PAGES.get("/settings")
def settings_page() -> str:
    settings = current_settings()
    return render_template(
        "settings.html",
        languages=LANGUAGES,
        target_languages=LANGUAGES[1:],
        female_voices=FEMALE_VOICES,
        male_voices=MALE_VOICES,
        settings=settings,
        footer_note="XXX",
        VOICE_SAMPLE_TEXT=VOICE_SAMPLE_TEXT,
    )


@API.get("/health")
def health() -> Response:
    return jsonify({"ok": True})


@API.get("/settings")
def api_get_settings() -> Response:
    settings = current_settings()
    return jsonify(settings.to_mapping())


@API.post("/settings")
def api_update_settings() -> Response:
    payload = request.get_json(silent=True) or {}
    settings = current_settings()

    default_language = settings.default_language
    if "default_language" in payload:
        default_language = _normalize_language(payload.get("default_language"))

    translate_by_default = settings.translate_by_default
    if "translate_by_default" in payload:
        translate_by_default = bool(payload.get("translate_by_default"))

    default_target_language = settings.default_target_language
    if "default_target_language" in payload:
        default_target_language = _normalize_language(payload.get("default_target_language"))

    female_voice = settings.female_voice
    if "female_voice" in payload:
        female_voice = str(payload.get("female_voice") or settings.female_voice)

    male_voice = settings.male_voice
    if "male_voice" in payload:
        male_voice = str(payload.get("male_voice") or settings.male_voice)

    updated = Settings(
        default_language=default_language,
        translate_by_default=translate_by_default,
        default_target_language=default_target_language,
        female_voice=female_voice,
        male_voice=male_voice,
    )
    save_settings(updated)
    current_app.config["DICTAITE_SETTINGS"] = updated
    return jsonify(updated.to_mapping())


@API.post("/tts-test")
def api_tts_test() -> Response:
    payload = request.get_json(silent=True) or {}
    gender = str(payload.get("gender", "female")).lower()
    text = str(payload.get("text", VOICE_SAMPLE_TEXT))
    voice_override = payload.get("voice")

    settings = current_settings()
    voice = str(voice_override) if voice_override else (
        settings.female_voice if gender == "female" else settings.male_voice
    )
    try:
        audio = synthesize_speech(text, voice)
    except Exception as exc:  # pragma: no cover - OpenAI failure
        LOGGER.exception("TTS call failed")
        return json_error("tts_failed", str(exc), 502)

    return send_file(
        io.BytesIO(audio),
        mimetype="audio/wav",
        as_attachment=False,
        download_name="preview.wav",
    )


def current_settings() -> Settings:
    settings = current_app.config.get("DICTAITE_SETTINGS")
    if isinstance(settings, Settings):
        return settings
    settings = load_settings()
    current_app.config["DICTAITE_SETTINGS"] = settings
    return settings


def json_error(code: str, message: str, status: int) -> Response:
    payload = {"error": {"code": code, "message": message}}
    return jsonify(payload), status


def _normalize_language(value: Any) -> str | None:
    if value in (None, "", "default"):
        return None
    return str(value)


if __name__ == "__main__":  # pragma: no cover - manual execution
    logging.basicConfig(level=logging.INFO)
    app = create_app({"ENV": "production"})
    app.run(host="0.0.0.0", port=8080)
