"""Text-to-speech helpers."""

from __future__ import annotations

import logging
from typing import Final

from ._client import get_openai_client

LOGGER = logging.getLogger(__name__)

TTS_MODEL: Final[str] = "tts-1"
DEFAULT_VOICE: Final[str] = "Nova"


def synthesize_speech(text: str, voice: str | None = None) -> bytes:
    """Generate a WAV audio preview for *text* using the configured voice."""

    clean = text.strip()
    if not clean:
        raise ValueError("Cannot generate speech for empty text")

    target_voice = voice or DEFAULT_VOICE
    LOGGER.info("Generating speech using voice %s", target_voice)
    client = get_openai_client()
    response = client.audio.speech.create(
        input=clean,
        model=TTS_MODEL,
        voice=target_voice,
        response_format="wav",
    )
    if isinstance(response, (bytes, bytearray)):
        return bytes(response)
    if hasattr(response, "read"):
        return response.read()
    if hasattr(response, "content"):
        return bytes(response.content)
    if hasattr(response, "iter_bytes"):
        return b"".join(response.iter_bytes())
    raise TypeError(f"Unsupported response type: {type(response)}")


__all__ = ["synthesize_speech"]
