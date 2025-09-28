use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const SETTINGS_FILENAME: &str = "settings.json";
const LEGACY_FILENAME: &str = "dict-ai-te_config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(deserialize_with = "deserialize_optional_lang")]
    pub default_language: Option<String>,
    pub translate_by_default: bool,
    #[serde(deserialize_with = "deserialize_optional_lang")]
    pub default_target_language: Option<String>,
    pub female_voice: String,
    pub male_voice: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_language: None,
            translate_by_default: false,
            default_target_language: Some("en".to_string()),
            female_voice: "nova".to_string(),
            male_voice: "onyx".to_string(),
        }
    }
}

fn deserialize_optional_lang<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
            None
        } else {
            Some(trimmed.to_string())
        }
    }))
}

pub fn load_settings() -> Settings {
    load_settings_from_path(None).unwrap_or_default()
}

pub fn load_settings_from_path(custom_path: Option<&Path>) -> Result<Settings> {
    let settings_path = custom_path
        .map(Path::to_owned)
        .unwrap_or_else(default_settings_path);

    match fs::read_to_string(&settings_path) {
        Ok(raw) => {
            let parsed: Settings = serde_json::from_str(&raw)
                .with_context(|| format!("Invalid JSON in {}", settings_path.display()))?;
            Ok(fill_defaults(parsed))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            if let Some(legacy) = load_legacy_settings()? {
                save_settings_to_path(&legacy, &settings_path)?;
                Ok(legacy)
            } else {
                Ok(Settings::default())
            }
        }
        Err(err) => Err(err).with_context(|| format!("Failed reading {}", settings_path.display())),
    }
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let path = default_settings_path();
    save_settings_to_path(settings, &path)
}

pub fn save_settings_to_path(settings: &Settings, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed creating {}", parent.display()))?;
    }
    let payload =
        serde_json::to_string_pretty(settings).context("Failed serializing settings to JSON")?;
    fs::write(path, payload).with_context(|| format!("Failed writing {}", path.display()))
}

pub fn default_settings_path() -> PathBuf {
    config_dir().join(SETTINGS_FILENAME)
}

pub fn config_dir() -> PathBuf {
    if let Ok(custom) = env::var("DICTAITE_HOME") {
        let path = PathBuf::from(custom);
        if path.is_absolute() {
            return path;
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".dictaite")
}

fn load_legacy_settings() -> Result<Option<Settings>> {
    let legacy_path = legacy_settings_path();
    if !legacy_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&legacy_path)
        .with_context(|| format!("Failed reading {}", legacy_path.display()))?;
    let table: toml::Value = toml::from_str(&raw)
        .with_context(|| format!("Invalid TOML in {}", legacy_path.display()))?;
    let mut settings = Settings::default();
    if let Some(value) = table.get("default_language").and_then(|v| v.as_str()) {
        settings.default_language = normalize_optional(value);
    }
    if let Some(value) = table.get("translate_by_default").and_then(|v| v.as_bool()) {
        settings.translate_by_default = value;
    } else if let Some(value) = table.get("translation_enabled").and_then(|v| v.as_bool()) {
        settings.translate_by_default = value;
    }
    if let Some(value) = table
        .get("default_target_language")
        .and_then(|v| v.as_str())
    {
        settings.default_target_language = normalize_optional(value);
    } else if let Some(value) = table.get("target_language").and_then(|v| v.as_str()) {
        settings.default_target_language = normalize_optional(value);
    }
    if let Some(value) = table.get("female_voice").and_then(|v| v.as_str()) {
        settings.female_voice = value.to_string();
    } else if let Some(value) = table.get("femaleVoice").and_then(|v| v.as_str()) {
        settings.female_voice = value.to_string();
    }
    if let Some(value) = table.get("male_voice").and_then(|v| v.as_str()) {
        settings.male_voice = value.to_string();
    } else if let Some(value) = table.get("maleVoice").and_then(|v| v.as_str()) {
        settings.male_voice = value.to_string();
    }
    Ok(Some(fill_defaults(settings)))
}

fn legacy_settings_path() -> PathBuf {
    let dir = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
    dir.join("dict-ai-te").join(LEGACY_FILENAME)
}

fn normalize_optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn fill_defaults(mut settings: Settings) -> Settings {
    if settings.female_voice.trim().is_empty() {
        settings.female_voice = "nova".to_string();
    } else {
        settings.female_voice = settings.female_voice.trim().to_ascii_lowercase();
    }
    if settings.male_voice.trim().is_empty() {
        settings.male_voice = "onyx".to_string();
    } else {
        settings.male_voice = settings.male_voice.trim().to_ascii_lowercase();
    }
    if let Some(ref mut lang) = settings.default_language {
        if lang.trim().is_empty() {
            settings.default_language = None;
        }
    }
    if let Some(ref mut lang) = settings.default_target_language {
        if lang.trim().is_empty() {
            settings.default_target_language = Some("en".to_string());
        }
    }
    settings
}
