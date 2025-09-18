"""Shared UI constants for all front-ends."""

from __future__ import annotations

from typing import Sequence

LANGUAGES: Sequence[dict[str, str]] = [
    {"code": "default", "name": "Default (Auto-detect)"},
    {"code": "en", "name": "English"},
    {"code": "zh", "name": "中文 (Chinese, Mandarin)"},
    {"code": "es", "name": "Español (Spanish)"},
    {"code": "de", "name": "Deutsch (German)"},
    {"code": "fr", "name": "Français (French)"},
    {"code": "ja", "name": "日本語 (Japanese)"},
    {"code": "pt", "name": "Português (Portuguese)"},
    {"code": "ru", "name": "Русский (Russian)"},
    {"code": "ar", "name": "العربية (Arabic)"},
    {"code": "it", "name": "Italiano (Italian)"},
    {"code": "ko", "name": "한국어 (Korean)"},
    {"code": "hi", "name": "हिन्दी (Hindi)"},
    {"code": "nl", "name": "Nederlands (Dutch)"},
    {"code": "tr", "name": "Türkçe (Turkish)"},
    {"code": "pl", "name": "Polski (Polish)"},
    {"code": "id", "name": "Bahasa Indonesia (Indonesian)"},
    {"code": "th", "name": "ภาษาไทย (Thai)"},
    {"code": "sv", "name": "Svenska (Swedish)"},
    {"code": "he", "name": "עברית (Hebrew)"},
    {"code": "cs", "name": "Čeština (Czech)"},
]

LANGUAGE_NAME = {item["code"]: item["name"] for item in LANGUAGES}

FEMALE_VOICES = [
    ("nova", "Nova"),
    ("alloy", "Alloy"),
    ("verse", "Verse"),
    ("sol", "Sol"),
]

MALE_VOICES = [
    ("onyx", "Onyx"),
    ("sage", "Sage"),
    ("echo", "Echo"),
    ("ember", "Ember"),
]

VOICE_SAMPLE_TEXT = "This is a short sample to preview the selected voice."

__all__ = [
    "LANGUAGES",
    "LANGUAGE_NAME",
    "FEMALE_VOICES",
    "MALE_VOICES",
    "VOICE_SAMPLE_TEXT",
]
