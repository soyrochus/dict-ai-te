"""Service layer abstractions shared by every UI."""

from .tts import synthesize_speech

__all__ = [
    "synthesize_speech",
]
