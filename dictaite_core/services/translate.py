"""Translation helper functions."""

from __future__ import annotations

import logging
from typing import Final

from ._client import get_openai_client
from .text_utils import format_structured_text

LOGGER = logging.getLogger(__name__)

TRANSLATE_MODEL: Final[str] = "gpt-5-mini-2025-08-07"
CHAT_TEMPERATURE: Final[float] = 0.2
CHAT_PROMPT_TEMPLATE: Final[str] = (
    "Translate the following text to {target}. "
    "Format the translation into clear paragraphs separated by blank lines. "
    "Return only the translated text.\n\n{text}"
)


def translate(text: str, target_lang: str) -> str:
    """Translate *text* into *target_lang* using the OpenAI chat endpoint."""

    LOGGER.info("Translating text to %s", target_lang)
    if not text.strip():
        return ""

    client = get_openai_client()
    prompt = CHAT_PROMPT_TEMPLATE.format(target=target_lang, text=text)
    response = client.chat.completions.create(
        model=TRANSLATE_MODEL,
        messages=[{"role": "user", "content": prompt}],
        temperature=CHAT_TEMPERATURE,
    )
    choice = response.choices[0]
    content = getattr(choice.message, "content", None) or ""
    LOGGER.debug("Received translation response (%d tokens)", getattr(choice, "index", -1))
    return format_structured_text(content)


__all__ = ["translate"]
