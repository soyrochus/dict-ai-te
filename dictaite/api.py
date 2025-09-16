# Part of dictaite: Recording, transcribing, and translating voice notes | Copyright (c) 2025 | License: MIT
"""OpenAI API client and convenience functions for transcription and translation."""

import os
from typing import BinaryIO

from dotenv import load_dotenv
from openai import OpenAI

# Default model and prompt settings
WHISPER_MODEL = "gpt-4o-transcribe"
WHISPER_PROMPT = (
    "Transcribe the audio and return well-structured paragraphs. "
    "Use blank lines to separate paragraphs and fix simple punctuation errors. "
    "Ensure proper spacing, punctuation, and clear paragraph breaks."
)
CHAT_MODEL = "gpt-4o-mini-2025-08-07"
CHAT_TEMPERATURE = 0.2
CHAT_PROMPT_TEMPLATE = (
    "Translate the following text from {src_name} to {tgt_name}. "
    "Format the translation into clear paragraphs separated by blank lines. "
    "Ensure proper spacing, punctuation, and paragraph structure. "
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
    """Transcribe audio file using the OpenAI Whisper API."""
    client = get_openai_client()
    if not client:
        raise RuntimeError("OpenAI API key not configured")

    kwargs: dict[str, object] = {
        "model": WHISPER_MODEL,
        "file": file,
        "prompt": WHISPER_PROMPT,
    }
    if language and language != "default":
        kwargs["language"] = language

    response = client.audio.transcriptions.create(**kwargs)
    return response.text


def translate_text(text: str, src_name: str, tgt_name: str) -> str:
    """Translate text from src_name to tgt_name using the OpenAI Chat API."""
    client = get_openai_client()
    if not client:
        raise RuntimeError("OpenAI API key not configured")

    prompt = CHAT_PROMPT_TEMPLATE.format(src_name=src_name, tgt_name=tgt_name, text=text)
    comp = client.chat.completions.create(
        model=CHAT_MODEL,
        messages=[{"role": "user", "content": prompt}],
        temperature=CHAT_TEMPERATURE,
    )
    content = comp.choices[0].message.content
    return content.strip() if content else ""