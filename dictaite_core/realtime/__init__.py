"""Shared realtime transcription and translation helpers."""

from .audio import (
    TARGET_SAMPLE_RATE,
    base64_pcm16,
    chunk_pcm16,
    float_samples_to_pcm16,
    normalize_audio,
)
from .events import NormalizedEvent, RealtimeEventType, parse_realtime_event
from .models import LiveMode, RealtimeSettings
from .settings import realtime_settings_from_legacy
from .transcript import TranscriptAssembler
from .transport import (
    OpenAIRealtimeClient,
    RealtimeClientConfig,
    RealtimeClientError,
    bridge_websocket_messages,
)

__all__ = [
    "TARGET_SAMPLE_RATE",
    "LiveMode",
    "NormalizedEvent",
    "OpenAIRealtimeClient",
    "RealtimeClientConfig",
    "RealtimeClientError",
    "RealtimeEventType",
    "RealtimeSettings",
    "TranscriptAssembler",
    "base64_pcm16",
    "bridge_websocket_messages",
    "chunk_pcm16",
    "float_samples_to_pcm16",
    "normalize_audio",
    "parse_realtime_event",
    "realtime_settings_from_legacy",
]
