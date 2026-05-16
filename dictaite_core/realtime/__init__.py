"""Shared realtime transcription and translation helpers."""

from .audio import (
    TARGET_SAMPLE_RATE,
    base64_pcm16,
    chunk_pcm16,
    float_samples_to_pcm16,
    normalize_audio,
)
from .events import NormalizedEvent, RealtimeEventType, parse_realtime_event
from .transcript import TranscriptAssembler
from .transport import (
    LiveMode,
    OpenAIRealtimeClient,
    RealtimeClientConfig,
    RealtimeClientError,
)

__all__ = [
    "TARGET_SAMPLE_RATE",
    "LiveMode",
    "NormalizedEvent",
    "OpenAIRealtimeClient",
    "RealtimeClientConfig",
    "RealtimeClientError",
    "RealtimeEventType",
    "TranscriptAssembler",
    "base64_pcm16",
    "chunk_pcm16",
    "float_samples_to_pcm16",
    "normalize_audio",
    "parse_realtime_event",
]
