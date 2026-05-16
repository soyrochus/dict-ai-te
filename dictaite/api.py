"""Compatibility API wrappers that delegate to :mod:`dictaite_core`."""

from __future__ import annotations

from typing import BinaryIO

from dictaite_core.services import synthesize_speech
from dictaite_core.services._client import get_openai_client  # re-exported for backwards compatibility


def transcribe_file(file: BinaryIO, language: str | None = None) -> str:
    raise RuntimeError("Batch transcription was removed; use realtime transcription instead")


def translate_text(text: str, _src_name: str, tgt_name: str) -> str:
    raise RuntimeError("Batch translation was removed; use realtime translation instead")


def generate_preview(text: str, voice: str | None = None) -> bytes:
    return synthesize_speech(text, voice)


__all__ = ["get_openai_client", "transcribe_file", "translate_text", "generate_preview"]
