"""Settings migration helpers for realtime mode."""

from __future__ import annotations

from typing import Any, Mapping

from dictaite_core.config import Settings

from .models import RealtimeSettings


def realtime_settings_from_legacy(value: Settings | Mapping[str, Any]) -> RealtimeSettings:
    """Map existing settings fields to live-first settings."""

    if isinstance(value, Settings):
        translate_by_default = value.translate_by_default
        target_language = value.default_target_language
        source_language = value.default_language
    else:
        translate_by_default = bool(value.get("live_translation_enabled", value.get("translate_by_default", False)))
        target_language = _optional_str(value.get("target_language", value.get("default_target_language", "en")))
        source_language = _optional_str(value.get("source_language", value.get("default_language")))

    return RealtimeSettings(
        live_translation_enabled=translate_by_default,
        target_language=target_language,
        source_language=source_language,
    )


def _optional_str(value: Any) -> str | None:
    if value in (None, "", "default"):
        return None
    return str(value)
