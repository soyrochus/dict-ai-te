"""Compatibility shim exposing the new core settings helpers."""

from __future__ import annotations

from dataclasses import asdict

from dictaite_core.config import Settings, load_settings, save_settings


class AppConfig(Settings):
    """Backward compatible alias for :class:`dictaite_core.config.Settings`."""

    @classmethod
    def load(cls) -> "AppConfig":
        settings = load_settings()
        return cls(**asdict(settings))

    def save(self) -> None:
        save_settings(self)


__all__ = ["AppConfig", "Settings", "load_settings", "save_settings"]
