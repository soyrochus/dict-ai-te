"""Shared helpers for acquiring API clients."""

from __future__ import annotations

import logging
import os
from functools import lru_cache

from dotenv import load_dotenv
from openai import OpenAI

LOGGER = logging.getLogger(__name__)


@lru_cache(maxsize=1)
def get_openai_client() -> OpenAI:
    """Return a cached OpenAI client configured via environment variables."""

    load_dotenv()
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        raise RuntimeError("OPENAI_API_KEY is not configured")
    LOGGER.debug("Initialising OpenAI client")
    return OpenAI(api_key=api_key)


__all__ = ["get_openai_client"]
