"""Configuration helpers shared by all user interfaces."""

from __future__ import annotations

from dataclasses import asdict, dataclass
from pathlib import Path
import json
import logging
import os
from typing import Any, Mapping

try:  # Python 3.11+
    import tomllib  # type: ignore[attr-defined]
except ModuleNotFoundError:  # pragma: no cover - Python <3.11 fallback
    tomllib = None  # type: ignore[assignment]

LOGGER = logging.getLogger(__name__)

CONFIG_DIR = Path(os.environ.get("DICTAITE_HOME", Path.home() / ".dictaite"))
SETTINGS_PATH = CONFIG_DIR / "settings.json"
LEGACY_CONFIG_DIR = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "dict-ai-te"
LEGACY_CONFIG_PATH = LEGACY_CONFIG_DIR / "dict-ai-te_config.toml"


@dataclass(slots=True)
class Settings:
    """Runtime configuration shared by GTK and web UIs."""

    default_language: str | None = None
    translate_by_default: bool = False
    default_target_language: str | None = "en"
    female_voice: str = "Nova"
    male_voice: str = "Onyx"

    @classmethod
    def from_mapping(cls, payload: Mapping[str, Any]) -> "Settings":
        """Create :class:`Settings` from any mapping."""
        return cls(
            default_language=_coerce_optional_str(payload.get("default_language")),
            translate_by_default=bool(payload.get("translate_by_default", payload.get("translation_enabled", False))),
            default_target_language=_coerce_optional_str(
                payload.get("default_target_language", payload.get("target_language"))
            ),
            female_voice=str(payload.get("female_voice", payload.get("femaleVoice", "Nova"))) or "Nova",
            male_voice=str(payload.get("male_voice", payload.get("maleVoice", "Onyx"))) or "Onyx",
        )

    def to_mapping(self) -> dict[str, Any]:
        """Serialize to a mapping suitable for JSON/TOML dumps."""
        return asdict(self)


def load_settings(path: Path | None = None) -> Settings:
    """Load settings from disk, migrating legacy formats when needed."""

    settings_path = path or SETTINGS_PATH
    try:
        raw = settings_path.read_text(encoding="utf-8")
    except FileNotFoundError:
        LOGGER.debug("No settings.json found at %s; checking legacy config", settings_path)
        legacy = _load_legacy_settings()
        if legacy:
            LOGGER.info("Migrated legacy configuration from %s", LEGACY_CONFIG_PATH)
            save_settings(legacy, settings_path)
            return legacy
        return Settings()
    except OSError as exc:  # pragma: no cover - filesystem failure
        LOGGER.warning("Failed reading settings at %s: %s", settings_path, exc)
        return Settings()

    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        LOGGER.warning("Invalid JSON in %s: %s", settings_path, exc)
        return Settings()

    return Settings.from_mapping(payload)


def save_settings(settings: Settings, path: Path | None = None) -> None:
    """Persist settings to disk in JSON format."""

    settings_path = path or SETTINGS_PATH
    settings_path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(settings.to_mapping(), indent=2, sort_keys=True)
    settings_path.write_text(payload, encoding="utf-8")
    LOGGER.debug("Saved settings to %s", settings_path)


def _load_legacy_settings() -> Settings | None:
    if not LEGACY_CONFIG_PATH.exists():
        return None
    if tomllib is None:  # pragma: no cover - Python <3.11 fallback
        LOGGER.warning("tomllib not available; cannot parse legacy TOML config")
        return None
    try:
        raw = LEGACY_CONFIG_PATH.read_text(encoding="utf-8")
        payload = tomllib.loads(raw)
    except (OSError, tomllib.TOMLDecodeError) as exc:  # type: ignore[attr-defined]
        LOGGER.warning("Failed parsing legacy settings: %s", exc)
        return None
    settings = Settings.from_mapping(payload)
    return settings


def _coerce_optional_str(value: Any) -> str | None:
    if value in (None, "", "default"):
        return None
    return str(value)


__all__ = [
    "Settings",
    "load_settings",
    "save_settings",
    "CONFIG_DIR",
    "SETTINGS_PATH",
]
