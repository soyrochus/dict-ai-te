"""Flask web UI for dict-ai-te."""

from __future__ import annotations

import io
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

from dictaite_core import Settings, load_settings, save_settings
from dictaite_core.services import (
    ALLOWED_MIME_TYPES,
    MAX_AUDIO_DURATION_SECONDS,
    TranscriptionError,
    prepare_wav,
    synthesize_speech,
    transcribe,
    translate,
)

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


@API.post("/transcribe")
def api_transcribe() -> Response:
    audio_file = request.files.get("audio")
    if audio_file is None:
        return json_error("missing_audio", "Missing audio upload", 400)

    mimetype = audio_file.mimetype or ""
    if mimetype not in ALLOWED_MIME_TYPES:
        return json_error("unsupported_type", f"Unsupported audio type: {mimetype}", 400)

    raw_data = audio_file.read()
    if not raw_data:
        return json_error("empty_audio", "Uploaded audio file is empty", 400)

    try:
        wav_bytes = prepare_wav(raw_data, mimetype)
    except TranscriptionError as exc:
        return json_error("invalid_audio", str(exc), 400)
    except Exception as exc:  # pragma: no cover - unexpected decode failure
        LOGGER.exception("Failed to decode upload")
        return json_error("decode_error", "Could not decode the uploaded audio", 500)

    duration_seconds = _duration_seconds(wav_bytes)
    if duration_seconds > MAX_AUDIO_DURATION_SECONDS:
        return json_error("too_long", "Audio duration exceeds the 2 minute limit", 400)

    language = _normalize_language(request.form.get("language"))
    should_translate = _parse_bool(request.form.get("translate"))
    target_lang = _normalize_language(request.form.get("target_lang"))

    if language == "default":
        language = None

    try:
        transcript = transcribe(wav_bytes, "audio/wav", language)
    except TranscriptionError as exc:
        return json_error("transcription_failed", str(exc), 400)
    except Exception as exc:  # pragma: no cover - OpenAI failure
        LOGGER.exception("Transcription call failed")
        return json_error("transcription_failed", str(exc), 502)

    result: dict[str, Any] = {
        "text": transcript,
        "durationMs": int(duration_seconds * 1000),
    }

    if should_translate:
        settings = current_settings()
        code = target_lang or settings.default_target_language
        if code:
            target_caption = LANGUAGE_NAME.get(code, code)
            try:
                translated = translate(transcript, target_caption)
                result["translatedText"] = translated
            except Exception as exc:  # pragma: no cover - OpenAI failure
                LOGGER.exception("Translation call failed")
                return json_error("translation_failed", str(exc), 502)

    return jsonify(result)


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


def _parse_bool(value: Any) -> bool:
    if isinstance(value, bool):
        return value
    if value is None:
        return False
    return str(value).lower() in {"1", "true", "yes", "on"}


def _normalize_language(value: Any) -> str | None:
    if value in (None, "", "default"):
        return None
    return str(value)


def _duration_seconds(audio: bytes) -> float:
    import soundfile as sf

    with sf.SoundFile(io.BytesIO(audio)) as data:
        frames = len(data)
        samplerate = data.samplerate or 1
    return frames / samplerate


if __name__ == "__main__":  # pragma: no cover - manual execution
    logging.basicConfig(level=logging.INFO)
    app = create_app({"ENV": "production"})
    app.run(host="0.0.0.0", port=8080)
