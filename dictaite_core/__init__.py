"""Core services and configuration for dict-ai-te."""

from .config import Settings, load_settings, save_settings
from .services.tts import synthesize_speech

__all__ = [
    "Settings",
    "load_settings",
    "save_settings",
    "synthesize_speech",
]
