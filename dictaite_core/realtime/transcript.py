"""Segment-based source transcript assembly."""

from __future__ import annotations

from collections import OrderedDict
from dataclasses import dataclass

from .events import NormalizedEvent, RealtimeEventType


@dataclass(slots=True)
class _Segment:
    text: str = ""
    final: bool = False


class TranscriptAssembler:
    """Assemble transcript deltas and completions by item id."""

    def __init__(self) -> None:
        self._segments: OrderedDict[str, _Segment] = OrderedDict()
        self._anonymous: list[str] = []

    def apply(self, event: NormalizedEvent) -> str:
        if event.type == RealtimeEventType.SOURCE_DELTA:
            self.add_delta(event.text, event.item_id)
        elif event.type == RealtimeEventType.SOURCE_COMPLETED:
            self.complete(event.text, event.item_id)
        return self.text

    def add_delta(self, text: str, item_id: str | None) -> None:
        if not text:
            return
        if not item_id:
            self._anonymous.append(text)
            return
        segment = self._segments.setdefault(item_id, _Segment())
        if segment.final:
            return
        segment.text += text

    def complete(self, text: str, item_id: str | None) -> None:
        if not text:
            return
        if not item_id:
            if text not in self._anonymous:
                self._anonymous.append(text)
            return
        segment = self._segments.setdefault(item_id, _Segment())
        segment.text = text
        segment.final = True

    @property
    def text(self) -> str:
        parts = [segment.text.strip() for segment in self._segments.values() if segment.text.strip()]
        parts.extend(part.strip() for part in self._anonymous if part.strip())
        return " ".join(parts)
