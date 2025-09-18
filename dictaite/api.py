"""Compatibility API wrappers that delegate to :mod:`dictaite_core`."""

from __future__ import annotations

from typing import BinaryIO

from dictaite_core.services import synthesize_speech, transcribe, translate
from dictaite_core.services._client import get_openai_client  # re-exported for backwards compatibility


def transcribe_file(file: BinaryIO, language: str | None = None) -> str:
    data = file.read()
    lang = None if not language or language == "default" else language
    return transcribe(data, "audio/wav", lang)


def translate_text(text: str, _src_name: str, tgt_name: str) -> str:
    return translate(text, tgt_name)


def generate_preview(text: str, voice: str | None = None) -> bytes:
    return synthesize_speech(text, voice)


__all__ = ["get_openai_client", "transcribe_file", "translate_text", "generate_preview"]
