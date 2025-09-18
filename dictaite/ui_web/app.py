"""Flask web UI for dict-ai-te."""

from __future__ import annotations

import io
import logging
import threading
import time
import uuid
from dataclasses import dataclass, field
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

from ..ui_common import FEMALE_VOICES, LANGUAGES, MALE_VOICES, VOICE_SAMPLE_TEXT

LOGGER = logging.getLogger(__name__)
BASE_DIR = Path(__file__).resolve().parent
TEMPLATES = Path(__file__).with_suffix("").parent / "templates"
STATIC = Path(__file__).with_suffix("").parent / "static"
RECORDINGS_DIR = BASE_DIR / "tmp" / "recordings"


@dataclass
class RecordingSession:
    """Represents an in-progress browser recording upload."""

    path: Path
    mime_type: str
    mode: str = "transcribe"
    language: str | None = None
    target_lang: str | None = None
    expected_seq: int = 0
    chunk_count: int = 0
    total_bytes: int = 0
    created_at: float = field(default_factory=time.time)
    finalizing: bool = False


RECORDING_SESSIONS: dict[str, RecordingSession] = {}
RECORDING_LOCK = threading.Lock()

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

    mimetype = _normalize_mime_type(audio_file.mimetype)
    if mimetype not in ALLOWED_MIME_TYPES:
        return json_error("unsupported_type", f"Unsupported audio type: {mimetype}", 400)

    raw_data = audio_file.read()
    if not raw_data:
        return json_error("empty_audio", "Uploaded audio file is empty", 400)

    try:
        wav_bytes, duration_seconds = _prepare_wav_and_duration(raw_data, mimetype)
    except TranscriptionError as exc:
        return json_error("invalid_audio", str(exc), 400)
    except Exception as exc:  # pragma: no cover - unexpected decode failure
        LOGGER.exception("Failed to decode upload")
        return json_error("decode_error", "Could not decode the uploaded audio", 500)

    language = _normalize_language(request.form.get("language"))
    should_translate = _parse_bool(request.form.get("translate"))
    target_lang = _normalize_language(request.form.get("target_lang"))

    if language == "default":
        language = None

    try:
        transcript, backend_latency = _run_transcription(wav_bytes, language)
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
            try:
                translated = translate(transcript, code)
                result["translatedText"] = translated
            except Exception as exc:  # pragma: no cover - OpenAI failure
                LOGGER.exception("Translation call failed")
                return json_error("translation_failed", str(exc), 502)

    LOGGER.info(
        "Handled /api/transcribe upload: duration=%.2fs, backend_latency=%.2fs, translate=%s",
        duration_seconds,
        backend_latency,
        should_translate,
    )

    return jsonify(result)


@API.post("/record/start")
def api_record_start() -> Response:
    payload = request.get_json(silent=True) or {}
    mime_type = _normalize_mime_type(payload.get("mime_type")) or "audio/webm"
    mode = str(payload.get("mode") or "transcribe").lower()
    if mode not in {"transcribe", "translate"}:
        mode = "transcribe"
    language = _normalize_language(payload.get("language"))
    target_lang = _normalize_language(payload.get("target_lang"))

    if mime_type not in ALLOWED_MIME_TYPES:
        return json_error("unsupported_type", f"Unsupported audio type: {mime_type}", 400)

    session_id = uuid.uuid4().hex
    path = RECORDINGS_DIR / f"{session_id}.webm"
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("wb"):
            pass
    except OSError as exc:
        LOGGER.exception("Unable to initialise recording session directory")
        return json_error("storage_unavailable", "Unable to initialise recording session", 500)

    session = RecordingSession(
        path=path,
        mime_type=mime_type,
        mode=mode,
        language=language,
        target_lang=target_lang,
    )
    with RECORDING_LOCK:
        RECORDING_SESSIONS[session_id] = session

    LOGGER.info("Started recording session %s (mode=%s, mime=%s)", session_id, mode, mime_type)
    return jsonify({"session_id": session_id, "mode": mode})


@API.post("/record/append")
def api_record_append() -> Response:
    form = request.form
    session_id = (form.get("session_id") or "").strip()
    chunk_file = request.files.get("chunk")
    seq_value = form.get("seq")

    if not session_id:
        return json_error("missing_session", "Missing session identifier", 400)
    if chunk_file is None:
        return json_error("missing_chunk", "Missing audio chunk upload", 400)
    try:
        seq = int(seq_value)
    except (TypeError, ValueError):
        return json_error("invalid_sequence", "Chunk sequence must be an integer", 400)

    data = chunk_file.read()
    if not data:
        return json_error("empty_chunk", "Uploaded chunk was empty", 400)

    with RECORDING_LOCK:
        session = RECORDING_SESSIONS.get(session_id)
        if session is None:
            return json_error("unknown_session", "Unknown recording session", 404)
        expected = session.expected_seq
        if seq != expected:
            return json_error("out_of_order", f"Expected chunk {expected}, received {seq}", 409)
        try:
            with session.path.open("ab") as handle:
                handle.write(data)
        except OSError as exc:
            LOGGER.exception("Failed to append chunk for session %s", session_id)
            return json_error("write_failed", "Could not persist audio chunk", 500)

        session.expected_seq += 1
        session.chunk_count += 1
        session.total_bytes += len(data)

    return jsonify({"ok": True, "next_seq": seq + 1})


@API.post("/record/finalize")
def api_record_finalize() -> Response:
    payload = request.get_json(silent=True) or {}
    session_id = str(payload.get("session_id") or "").strip()
    if not session_id:
        return json_error("missing_session", "Missing session identifier", 400)

    with RECORDING_LOCK:
        session = RECORDING_SESSIONS.get(session_id)
        if session is None:
            return json_error("unknown_session", "Unknown recording session", 404)
        if session.finalizing:
            return json_error("already_finalizing", "Recording session is already being processed", 409)
        session.finalizing = True

    def fail(code: str, message: str, status: int, *, cleanup: bool = True):
        if cleanup:
            _cleanup_recording_file(session)
            with RECORDING_LOCK:
                RECORDING_SESSIONS.pop(session_id, None)
        else:
            with RECORDING_LOCK:
                session.finalizing = False
        return json_error(code, message, status)

    if session.chunk_count == 0 or session.total_bytes == 0:
        return fail("empty_recording", "No audio data was received", 400)

    mode = str(payload.get("mode") or session.mode or "transcribe").lower()
    if mode not in {"transcribe", "translate"}:
        mode = "transcribe"
    language = _normalize_language(payload.get("language")) or session.language
    target_lang = _normalize_language(payload.get("target_lang")) or session.target_lang

    try:
        raw_data = session.path.read_bytes()
    except OSError as exc:  # pragma: no cover - unexpected filesystem error
        LOGGER.exception("Failed to reassemble recording %s", session_id)
        return fail("missing_file", "Recording data was not found on the server", 500)

    if not raw_data:
        return fail("empty_recording", "No audio data was received", 400)

    try:
        normalized_mime = _normalize_mime_type(session.mime_type) or "audio/webm"
        wav_bytes, duration_seconds = _prepare_wav_and_duration(raw_data, normalized_mime)
    except TranscriptionError as exc:
        return fail("invalid_audio", str(exc), 400)
    except Exception as exc:  # pragma: no cover - unexpected decode failure
        LOGGER.exception("Failed to decode assembled recording %s", session_id)
        return fail("decode_error", "Could not decode the recorded audio", 500)

    try:
        transcript, backend_latency = _run_transcription(wav_bytes, language)
    except TranscriptionError as exc:
        return fail("transcription_failed", str(exc), 400)
    except Exception as exc:  # pragma: no cover - OpenAI failure
        LOGGER.exception("Transcription call failed for session %s", session_id)
        return fail("transcription_failed", str(exc), 502, cleanup=False)

    result: dict[str, Any] = {
        "session_id": session_id,
        "mode": mode,
        "text": transcript,
        "durationMs": int(duration_seconds * 1000),
    }

    if mode == "translate":
        settings = current_settings()
        code = target_lang or settings.default_target_language
        if code:
            try:
                translated = translate(transcript, code)
                result["translatedText"] = translated
            except Exception as exc:  # pragma: no cover - OpenAI failure
                LOGGER.exception("Translation call failed for session %s", session_id)
                return fail("translation_failed", str(exc), 502, cleanup=False)

    LOGGER.info(
        "Finalized recording session %s: mode=%s, chunks=%d, bytes=%d, duration=%.2fs, backend_latency=%.2fs",
        session_id,
        mode,
        session.chunk_count,
        session.total_bytes,
        duration_seconds,
        backend_latency,
    )

    _cleanup_recording_file(session)
    with RECORDING_LOCK:
        RECORDING_SESSIONS.pop(session_id, None)
    return jsonify(result)


@API.post("/record/cancel")
def api_record_cancel() -> Response:
    payload = request.get_json(silent=True) or {}
    session_id = (
        str(payload.get("session_id") or "").strip()
        or request.args.get("session_id", type=str, default="").strip()
        or (request.form.get("session_id") or "").strip()
    )
    if not session_id:
        return json_error("missing_session", "Missing session identifier", 400)

    with RECORDING_LOCK:
        session = RECORDING_SESSIONS.pop(session_id, None)

    if session is None:
        return jsonify({"ok": True, "status": "not_found"})

    _cleanup_recording_file(session)
    LOGGER.info(
        "Cancelled recording session %s after %d chunks (%d bytes)",
        session_id,
        session.chunk_count,
        session.total_bytes,
    )
    return jsonify({"ok": True})


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


def _normalize_mime_type(value: Any) -> str:
    mimetype = str(value or "").strip().lower()
    if not mimetype:
        return ""
    if ";" in mimetype:
        mimetype = mimetype.split(";", 1)[0].strip()
    return mimetype


def _prepare_wav_and_duration(raw_audio: bytes, mimetype: str) -> tuple[bytes, float]:
    wav_bytes = prepare_wav(raw_audio, mimetype)
    duration_seconds = _duration_seconds(wav_bytes)
    if duration_seconds > MAX_AUDIO_DURATION_SECONDS:
        raise TranscriptionError("Audio duration exceeds the 2 minute limit")
    return wav_bytes, duration_seconds


def _run_transcription(wav_bytes: bytes, language: str | None) -> tuple[str, float]:
    start = time.perf_counter()
    transcript = transcribe(wav_bytes, "audio/wav", language)
    latency = time.perf_counter() - start
    return transcript, latency


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


def _cleanup_recording_file(session: RecordingSession) -> None:
    try:
        session.path.unlink(missing_ok=True)
    except OSError:  # pragma: no cover - best effort cleanup
        LOGGER.warning("Failed to remove temporary recording file: %s", session.path)


if __name__ == "__main__":  # pragma: no cover - manual execution
    logging.basicConfig(level=logging.INFO)
    app = create_app({"ENV": "production"})
    app.run(host="0.0.0.0", port=8080)
