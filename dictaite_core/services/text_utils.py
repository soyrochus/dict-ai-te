"""Utilities shared by transcription and translation services."""

from __future__ import annotations

import re

_PARA_SPLIT = re.compile(r"\n\s*\n")
_SPACE_COLLAPSE = re.compile(r"\s+")


def format_structured_text(text: str) -> str:
    """Normalise whitespace and paragraph spacing for readability."""

    stripped = text.strip()
    if not stripped:
        return ""
    paragraphs: list[str] = []
    for block in _PARA_SPLIT.split(stripped):
        block = block.strip()
        if not block:
            continue
        lines = [line.strip() for line in block.splitlines() if line.strip()]
        normalized = " ".join(_SPACE_COLLAPSE.sub(" ", line) for line in lines)
        if normalized:
            paragraphs.append(normalized)
    return "\n\n".join(paragraphs)


__all__ = ["format_structured_text"]
