# Part of dictaite: Recording, transcribing, and translating voice notes | Copyright (c) 2025 | License: MIT
"""OpenAI API client and convenience functions for transcription and translation."""

import os
import re
from typing import BinaryIO

from dotenv import load_dotenv
from openai import OpenAI

# Default model and prompt settings
TRANSCRIBE_MODEL = "gpt-4o-transcribe"
TRANSCRIBE_PROMPT = (
    "Transcribe the audio and return well-structured paragraphs. "
    "Use blank lines to separate paragraphs and fix simple punctuation errors."
)
TRANSLATE_MODEL = "gpt-5-mini-2025-08-07"
CHAT_TEMPERATURE = 0.2
CHAT_PROMPT_TEMPLATE = (
    "Translate the following text from {src_name} to {tgt_name}. "
    "Format the translation into clear paragraphs separated by blank lines. "
    "Return only the translated text.\n\n{text}"
)


def get_openai_client() -> OpenAI | None:
    """Load OpenAI API key from environment (or .env) and return an OpenAI client."""
    load_dotenv()
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        return None
    return OpenAI(api_key=api_key)


def transcribe_file(file: BinaryIO, language: str | None = None) -> str:
    """Transcribe audio file using the OpenAI transcription API."""
    client = get_openai_client()
    if not client:
        raise RuntimeError("OpenAI API key not configured")

    kwargs: dict[str, object] = {
        "model": TRANSCRIBE_MODEL,
        "file": file,
        "prompt": TRANSCRIBE_PROMPT,
    }
    if language and language != "default":
        kwargs["language"] = language

    response = client.audio.transcriptions.create(**kwargs)
    return format_structured_text(response.text)


def translate_text(text: str, src_name: str, tgt_name: str) -> str:
    """Translate text from src_name to tgt_name using the OpenAI Chat API."""
    client = get_openai_client()
    if not client:
        raise RuntimeError("OpenAI API key not configured")

    prompt = CHAT_PROMPT_TEMPLATE.format(src_name=src_name, tgt_name=tgt_name, text=text)
    comp = client.chat.completions.create(
        model=TRANSLATE_MODEL,
        messages=[{"role": "user", "content": prompt}],
        temperature=CHAT_TEMPERATURE,
    )
    content = comp.choices[0].message.content
    return format_structured_text(content or "")


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


def synthesize_speech_wav(text: str, voice: str) -> bytes:
    """Generate speech from text using OpenAI TTS and return WAV bytes."""
    client = get_openai_client()
    if not client:
        raise RuntimeError("OpenAI API key not configured")

    response = client.audio.speech.create(
        input=text,
        model="tts-1",
        voice=voice,
        response_format="wav"
    )
    
    # Handle different response types
    if isinstance(response, (bytes, bytearray)):
        return bytes(response)
    elif hasattr(response, "read"):
        return response.read()
    elif hasattr(response, "content"):
        return response.content
    elif hasattr(response, "iter_bytes"):
        # For httpx streaming responses
        return b"".join(response.iter_bytes())
    else:
        raise TypeError(f"Unknown response type: {type(response)}")
