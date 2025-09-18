"""Service layer abstractions shared by every UI."""

from .stt import (
    ALLOWED_MIME_TYPES,
    MAX_AUDIO_DURATION_SECONDS,
    TranscriptionError,
    prepare_wav,
    transcribe,
)
from .translate import translate
from .tts import synthesize_speech

__all__ = [
    "ALLOWED_MIME_TYPES",
    "MAX_AUDIO_DURATION_SECONDS",
    "TranscriptionError",
    "prepare_wav",
    "transcribe",
    "translate",
    "synthesize_speech",
]
