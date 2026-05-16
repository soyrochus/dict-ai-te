"""Realtime model types."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum


class LiveMode(StrEnum):
    TRANSCRIBE = "transcribe"
    TRANSLATE = "translate"


@dataclass(frozen=True, slots=True)
class RealtimeSettings:
    """Settings used by live-first realtime sessions."""

    live_translation_enabled: bool = False
    target_language: str | None = "en"
    source_language: str | None = None

    @property
    def mode(self) -> LiveMode:
        return LiveMode.TRANSLATE if self.live_translation_enabled else LiveMode.TRANSCRIBE
