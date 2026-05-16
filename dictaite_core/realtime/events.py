"""OpenAI realtime event normalization."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from typing import Any, Mapping


class RealtimeEventType(StrEnum):
    SOURCE_DELTA = "source_delta"
    SOURCE_COMPLETED = "source_completed"
    TRANSLATION_DELTA = "translation_delta"
    TRANSLATED_AUDIO_DELTA = "translated_audio_delta"
    SESSION_STATE = "session_state"
    ERROR = "error"
    UNKNOWN = "unknown"


@dataclass(frozen=True, slots=True)
class NormalizedEvent:
    type: RealtimeEventType
    text: str = ""
    item_id: str | None = None
    state: str | None = None
    error: str | None = None


def parse_realtime_event(payload: Mapping[str, Any]) -> NormalizedEvent:
    """Normalize known realtime transcription and translation events."""

    event_type = str(payload.get("type") or "")
    if event_type == "conversation.item.input_audio_transcription.delta":
        return NormalizedEvent(
            RealtimeEventType.SOURCE_DELTA,
            text=_text(payload, "delta"),
            item_id=_optional_str(payload.get("item_id")),
        )
    if event_type == "conversation.item.input_audio_transcription.completed":
        return NormalizedEvent(
            RealtimeEventType.SOURCE_COMPLETED,
            text=_text(payload, "transcript", "text"),
            item_id=_optional_str(payload.get("item_id")),
        )
    if event_type == "session.input_transcript.delta":
        return NormalizedEvent(RealtimeEventType.SOURCE_DELTA, text=_text(payload, "delta"))
    if event_type in {
        "session.output_transcript.delta",
        "response.output_text.delta",
        "response.output_audio_transcript.delta",
    }:
        return NormalizedEvent(RealtimeEventType.TRANSLATION_DELTA, text=_text(payload, "delta"))
    if event_type in {"session.output_audio.delta", "response.audio.delta", "response.output_audio.delta"}:
        return NormalizedEvent(RealtimeEventType.TRANSLATED_AUDIO_DELTA)
    if event_type.startswith("session.") or event_type.startswith("response."):
        return NormalizedEvent(RealtimeEventType.SESSION_STATE, state=event_type)
    if event_type == "error":
        error = payload.get("error")
        if isinstance(error, Mapping):
            message = str(error.get("message") or error.get("code") or "Realtime error")
        else:
            message = str(error or "Realtime error")
        return NormalizedEvent(RealtimeEventType.ERROR, error=message)
    return NormalizedEvent(RealtimeEventType.UNKNOWN, state=event_type or None)


def _text(payload: Mapping[str, Any], *keys: str) -> str:
    for key in keys:
        value = payload.get(key)
        if value is not None:
            return str(value)
    return ""


def _optional_str(value: Any) -> str | None:
    if value in (None, ""):
        return None
    return str(value)
