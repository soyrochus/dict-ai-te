use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SETTINGS_FILENAME: &str = "settings.json";
const LEGACY_FILENAME: &str = "dict-ai-te_config.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Openai,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalSettings {
    pub engine: String,
    pub model: String,
    pub model_path: Option<PathBuf>,
    pub device: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OpenAiSettings {
    pub api_key: Option<String>,
}

impl std::fmt::Debug for OpenAiSettings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenAiSettings")
            .field(
                "api_key",
                &resolve_api_key_from(None, self.api_key.as_deref()).masked(),
            )
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    Environment,
    Settings,
    Absent,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedApiKey {
    value: Option<String>,
    source: ApiKeySource,
}

impl ResolvedApiKey {
    pub fn as_str(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn source(&self) -> ApiKeySource {
        self.source
    }

    pub fn masked(&self) -> String {
        let Some(value) = self.value.as_deref() else {
            return "not configured".to_string();
        };
        let chars = value.chars().collect::<Vec<_>>();
        if chars.len() <= 4 {
            return "•".repeat(chars.len());
        }
        format!(
            "{}{}",
            "•".repeat(chars.len() - 4),
            chars[chars.len() - 4..].iter().collect::<String>()
        )
    }
}

impl std::fmt::Debug for ResolvedApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedApiKey")
            .field("source", &self.source)
            .field("value", &self.masked())
            .finish()
    }
}

impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            engine: "whisper".to_string(),
            model: "base.en".to_string(),
            model_path: None,
            device: "auto".to_string(),
        }
    }
}

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
    pub backend: Backend,
    pub openai: OpenAiSettings,
    pub local: LocalSettings,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_language: None,
            translate_by_default: false,
            default_target_language: Some("en".to_string()),
            female_voice: "nova".to_string(),
            male_voice: "onyx".to_string(),
            backend: Backend::Openai,
            openai: OpenAiSettings::default(),
            local: LocalSettings::default(),
            unknown: BTreeMap::new(),
        }
    }
}

pub fn effective_translate(settings: &Settings, requested: bool) -> bool {
    settings.backend != Backend::Local && requested
}

pub fn resolve_api_key(settings: &Settings) -> ResolvedApiKey {
    resolve_api_key_from(
        env::var("OPENAI_API_KEY").ok().as_deref(),
        settings.openai.api_key.as_deref(),
    )
}

pub fn resolve_api_key_from(environment: Option<&str>, stored: Option<&str>) -> ResolvedApiKey {
    if let Some(value) = normalize_secret(environment) {
        return ResolvedApiKey {
            value: Some(value),
            source: ApiKeySource::Environment,
        };
    }
    if let Some(value) = normalize_secret(stored) {
        return ResolvedApiKey {
            value: Some(value),
            source: ApiKeySource::Settings,
        };
    }
    ResolvedApiKey {
        value: None,
        source: ApiKeySource::Absent,
    }
}

fn normalize_secret(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
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
    if normalize_secret(settings.openai.api_key.as_deref()).is_some() {
        secure_write(path, payload.as_bytes())
    } else {
        fs::write(path, payload).with_context(|| format!("Failed writing {}", path.display()))
    }
}

#[cfg(unix)]
fn secure_write(path: &Path, payload: &[u8]) -> Result<()> {
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    let temp = parent.join(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .with_context(|| {
                format!(
                    "Failed creating secure settings file in {}",
                    parent.display()
                )
            })?;
        file.write_all(payload)
            .context("Failed writing secure settings")?;
        file.sync_all().context("Failed syncing secure settings")?;
        drop(file);
        fs::rename(&temp, path).with_context(|| format!("Failed installing {}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(not(unix))]
fn secure_write(path: &Path, payload: &[u8]) -> Result<()> {
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
    settings.openai.api_key = normalize_secret(settings.openai.api_key.as_deref());
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn legacy_json_defaults_to_openai_and_local_defaults() {
        let settings: Settings = serde_json::from_value(json!({
            "default_language": "es",
            "translate_by_default": true,
            "female_voice": "nova",
            "male_voice": "onyx"
        }))
        .unwrap();
        assert_eq!(settings.backend, Backend::Openai);
        assert_eq!(settings.local, LocalSettings::default());
        assert_eq!(settings.openai, OpenAiSettings::default());
        assert!(settings.translate_by_default);
    }

    #[test]
    fn backend_local_and_unknown_keys_round_trip() {
        let settings: Settings = serde_json::from_value(json!({
            "backend": "local",
            "local": {
                "engine": "whisper",
                "model": "small",
                "model_path": "/tmp/custom.bin",
                "device": "cpu"
            },
            "future_option": {"enabled": true}
        }))
        .unwrap();
        assert_eq!(settings.backend, Backend::Local);
        assert_eq!(settings.local.model, "small");
        let encoded = serde_json::to_value(&settings).unwrap();
        assert_eq!(encoded["future_option"], json!({"enabled": true}));
    }

    #[test]
    fn effective_translate_forces_local_without_mutating_preference() {
        let mut settings = Settings {
            translate_by_default: true,
            backend: Backend::Local,
            ..Settings::default()
        };
        assert!(!effective_translate(
            &settings,
            settings.translate_by_default
        ));
        assert!(settings.translate_by_default);
        settings.backend = Backend::Openai;
        assert!(effective_translate(
            &settings,
            settings.translate_by_default
        ));
    }

    #[test]
    fn backend_selection_survives_save_and_reload() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("dictaite-settings-{unique}.json"));
        let settings = Settings {
            backend: Backend::Local,
            ..Settings::default()
        };
        save_settings_to_path(&settings, &path).unwrap();
        let loaded = load_settings_from_path(Some(&path)).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(loaded.backend, Backend::Local);
    }

    #[test]
    fn api_key_resolution_prefers_environment_and_trims_values() {
        let resolved = resolve_api_key_from(Some("  env-secret  "), Some("stored-secret"));
        assert_eq!(resolved.source(), ApiKeySource::Environment);
        assert_eq!(resolved.as_str(), Some("env-secret"));

        let stored = resolve_api_key_from(Some("  "), Some("  stored-secret  "));
        assert_eq!(stored.source(), ApiKeySource::Settings);
        assert_eq!(stored.as_str(), Some("stored-secret"));

        let absent = resolve_api_key_from(None, Some("\t"));
        assert_eq!(absent.source(), ApiKeySource::Absent);
        assert_eq!(absent.as_str(), None);
    }

    #[test]
    fn resolved_key_debug_and_mask_never_expose_the_secret() {
        let secret = "sk-sentinel-123456";
        let resolved = resolve_api_key_from(None, Some(secret));
        assert!(!resolved.masked().contains(secret));
        assert!(!format!("{resolved:?}").contains(secret));
        let mut settings = Settings::default();
        settings.openai.api_key = Some(secret.to_string());
        assert!(!format!("{settings:?}").contains(secret));
        assert!(resolved.masked().ends_with("3456"));
    }

    #[test]
    fn stored_key_round_trips_and_clear_serializes_as_null() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("dictaite-key-settings-{unique}.json"));
        let mut settings = Settings::default();
        settings.openai.api_key = Some("  stored-secret  ".to_string());
        save_settings_to_path(&settings, &path).unwrap();
        let loaded = load_settings_from_path(Some(&path)).unwrap();
        assert_eq!(loaded.openai.api_key.as_deref(), Some("stored-secret"));

        settings.openai.api_key = None;
        save_settings_to_path(&settings, &path).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&raw).unwrap()["openai"]["api_key"],
            Value::Null
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn environment_key_is_never_serialized() {
        let settings = Settings::default();
        let resolved = resolve_api_key_from(Some("environment-sentinel"), None);
        assert_eq!(resolved.source(), ApiKeySource::Environment);
        let encoded = serde_json::to_string(&settings).unwrap();
        assert!(!encoded.contains("environment-sentinel"));
        assert_eq!(
            serde_json::from_str::<Value>(&encoded).unwrap()["openai"]["api_key"],
            Value::Null
        );
    }

    #[cfg(unix)]
    #[test]
    fn settings_with_a_key_are_atomically_installed_with_mode_0600() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("dictaite-secure-settings-{unique}"));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let mut settings = Settings::default();
        settings.openai.api_key = Some("stored-secret".to_string());
        save_settings_to_path(&settings, &path).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(fs::read_dir(&dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn failed_secure_install_cleans_temp_and_preserves_destination() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("dictaite-failed-settings-{unique}"));
        fs::create_dir(&dir).unwrap();
        let destination = dir.join("settings.json");
        fs::create_dir(&destination).unwrap();
        let mut settings = Settings::default();
        settings.openai.api_key = Some("stored-secret".to_string());
        assert!(save_settings_to_path(&settings, &destination).is_err());
        assert!(destination.is_dir());
        assert!(fs::read_dir(&dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        fs::remove_dir_all(dir).unwrap();
    }
}
