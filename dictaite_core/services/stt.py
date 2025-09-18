"""Speech-to-text helper functions."""

from __future__ import annotations

import io
import logging
from typing import Final

import soundfile as sf

from ._client import get_openai_client
from .text_utils import format_structured_text

LOGGER = logging.getLogger(__name__)

ALLOWED_MIME_TYPES: Final[set[str]] = {
    "audio/wav",
    "audio/x-wav",
    "audio/webm",
    "audio/ogg",
}
MAX_AUDIO_DURATION_SECONDS: Final[int] = 120
TRANSCRIBE_MODEL: Final[str] = "gpt-4o-transcribe"
TRANSCRIBE_PROMPT: Final[str] = (
    "Transcribe the audio and return well-structured paragraphs. "
    "Use blank lines to separate paragraphs and fix simple punctuation errors."
)


class TranscriptionError(RuntimeError):
    """Raised when a transcription request fails validation."""


def transcribe(audio: bytes, mimetype: str, language: str | None) -> str:
    """Transcribe audio bytes using OpenAI Whisper."""

    LOGGER.info("Transcribing audio blob (mimetype=%s, language=%s)", mimetype, language)
    wav_bytes = prepare_wav(audio, mimetype)
    duration = _duration_seconds(wav_bytes)
    if duration > MAX_AUDIO_DURATION_SECONDS:
        raise TranscriptionError("Audio duration exceeds the 2 minute limit")

    buffer = io.BytesIO(wav_bytes)
    buffer.name = "audio.wav"  # type: ignore[attr-defined]

    client = get_openai_client()
    kwargs: dict[str, object] = {
        "model": TRANSCRIBE_MODEL,
        "file": buffer,
        "prompt": TRANSCRIBE_PROMPT,
    }
    if language:
        kwargs["language"] = language

    response = client.audio.transcriptions.create(**kwargs)
    LOGGER.debug("Received transcription response")
    return format_structured_text(response.text)


def prepare_wav(audio: bytes, mimetype: str) -> bytes:
    if mimetype not in ALLOWED_MIME_TYPES:
        raise TranscriptionError(f"Unsupported audio mimetype: {mimetype}")
    if mimetype in {"audio/wav", "audio/x-wav"}:
        return audio
    try:
        from pydub import AudioSegment
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dependency
        raise TranscriptionError("pydub is required to decode non-WAV uploads") from exc

    format_hint = "webm" if mimetype == "audio/webm" else "ogg"
    segment = AudioSegment.from_file(io.BytesIO(audio), format=format_hint)
    LOGGER.debug("Decoded %s audio via pydub (duration=%.2fs)", mimetype, segment.duration_seconds)
    mono = segment.set_channels(1).set_frame_rate(16000)
    wav_buffer = io.BytesIO()
    mono.export(wav_buffer, format="wav")
    return wav_buffer.getvalue()


def _duration_seconds(audio: bytes) -> float:
    with sf.SoundFile(io.BytesIO(audio)) as data:
        frames = len(data)
        samplerate = data.samplerate or 1
    return frames / samplerate


__all__ = [
    "ALLOWED_MIME_TYPES",
    "MAX_AUDIO_DURATION_SECONDS",
    "TranscriptionError",
    "prepare_wav",
    "transcribe",
]
