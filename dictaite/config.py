# Part of dictaite: Recording, transcribing, and translating voice notes | Copyright (c) 2025 | License: MIT
"""Configuration management for dict-ai-te."""

import os
from pathlib import Path
from typing import Dict, Any

try:
    import tomllib
except ImportError:
    import tomli as tomllib

try:
    import tomli_w
except ImportError:
    # If tomli_w is not available, we'll use a simple TOML writer
    tomli_w = None


DEFAULT_CONFIG = {
    "voices": {
        "female": "nova",
        "male": "onyx"
    },
    "languages": {
        "default_source": "en",
        "default_target": "es"
    }
}

AVAILABLE_VOICES = {
    "female": ["nova", "alloy", "echo", "shimmer"],
    "male": ["onyx", "fable"]
}


def get_config_dir() -> Path:
    """Get the configuration directory, creating it if it doesn't exist."""
    config_dir = Path.home() / ".config" / "dict-ai-te"
    config_dir.mkdir(parents=True, exist_ok=True)
    return config_dir


def get_config_path() -> Path:
    """Get the full path to the configuration file."""
    return get_config_dir() / "dict-ai-te_config.toml"


def load_config() -> Dict[str, Any]:
    """Load configuration from TOML file, return defaults if file doesn't exist or on error."""
    config_path = get_config_path()
    
    if not config_path.exists():
        return DEFAULT_CONFIG.copy()
    
    try:
        with open(config_path, "rb") as f:
            config = tomllib.load(f)
        
        # Merge with defaults to ensure all required keys exist
        result = DEFAULT_CONFIG.copy()
        if "voices" in config:
            result["voices"].update(config["voices"])
        if "languages" in config:
            result["languages"].update(config["languages"])
        
        return result
    except Exception as e:
        print(f"Warning: Failed to load config from {config_path}: {e}")
        return DEFAULT_CONFIG.copy()


def save_config(config: Dict[str, Any]) -> bool:
    """Save configuration to TOML file."""
    config_path = get_config_path()
    
    try:
        if tomli_w:
            with open(config_path, "wb") as f:
                tomli_w.dump(config, f)
        else:
            # Simple TOML writer fallback
            with open(config_path, "w", encoding="utf-8") as f:
                _write_simple_toml(config, f)
        return True
    except Exception as e:
        print(f"Warning: Failed to save config to {config_path}: {e}")
        return False


def _write_simple_toml(data: Dict[str, Any], file) -> None:
    """Simple TOML writer for basic configuration."""
    for section_name, section_data in data.items():
        file.write(f"[{section_name}]\n")
        if isinstance(section_data, dict):
            for key, value in section_data.items():
                if isinstance(value, str):
                    file.write(f'{key} = "{value}"\n')
                else:
                    file.write(f"{key} = {value}\n")
        file.write("\n")