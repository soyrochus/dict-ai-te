from __future__ import annotations

"""Application configuration handling for dict-ai-te."""

from dataclasses import dataclass
from pathlib import Path
import os
import tomllib


CONFIG_DIR = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "dict-ai-te"
CONFIG_PATH = CONFIG_DIR / "dict-ai-te_config.toml"


@dataclass
class AppConfig:
    """Persisted configuration for default selections and voices."""

    default_language: str = "default"
    default_target_language: str = "en"
    translation_enabled: bool = False
    female_voice: str = "nova"
    male_voice: str = "onyx"

    @classmethod
    def load(cls) -> "AppConfig":
        """Load configuration from disk if present, otherwise defaults."""
        try:
            raw = CONFIG_PATH.read_text(encoding="utf-8")
        except FileNotFoundError:
            return cls()
        except OSError:
            return cls()
        try:
            data = tomllib.loads(raw)
        except tomllib.TOMLDecodeError:
            return cls()
        return cls(
            default_language=str(data.get("default_language", "default")),
            default_target_language=str(data.get("default_target_language", "en")),
            translation_enabled=bool(data.get("translation_enabled", False)),
            female_voice=str(data.get("female_voice", "nova")),
            male_voice=str(data.get("male_voice", "onyx")),
        )

    def save(self) -> None:
        """Persist configuration to disk in TOML format."""
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        CONFIG_PATH.write_text(self.to_toml(), encoding="utf-8")

    def to_toml(self) -> str:
        """Serialize configuration fields into simple TOML."""
        return (
            f'default_language = "{_escape(self.default_language)}"\n'
            f'default_target_language = "{_escape(self.default_target_language)}"\n'
            f"translation_enabled = {_format_bool(self.translation_enabled)}\n"
            f'female_voice = "{_escape(self.female_voice)}"\n'
            f'male_voice = "{_escape(self.male_voice)}"\n'
        )


def _escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def _format_bool(value: bool) -> str:
    return "true" if value else "false"


__all__ = ["AppConfig", "CONFIG_PATH", "CONFIG_DIR"]
