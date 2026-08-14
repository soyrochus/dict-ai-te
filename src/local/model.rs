use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use poll_promise::Promise;
use sha2::{Digest, Sha256};

use crate::settings::{config_dir, LocalSettings};

#[derive(Debug, Clone, Copy)]
pub struct ModelManifestEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub const MODEL_MANIFEST: &[ModelManifestEntry] = &[
    ModelManifestEntry {
        id: "base.en",
        display_name: "Base English (148 MB)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        size: 147_964_211,
        sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
    },
    ModelManifestEntry {
        id: "small",
        display_name: "Small Multilingual (488 MB)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        size: 487_601_967,
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAvailability {
    Ready(PathBuf),
    #[allow(dead_code)]
    Downloading {
        bytes: u64,
        total: u64,
    },
    NotDownloaded(PathBuf),
    MissingConfiguredPath(PathBuf),
}

pub fn manifest_entry(id: &str) -> Option<&'static ModelManifestEntry> {
    MODEL_MANIFEST.iter().find(|entry| entry.id == id)
}

pub fn model_path(engine: &str, model_id: &str) -> PathBuf {
    config_dir()
        .join("models")
        .join(engine)
        .join(model_id)
        .join(format!("ggml-{model_id}.bin"))
}

pub fn resolve_model_path(settings: &LocalSettings) -> PathBuf {
    settings
        .model_path
        .clone()
        .unwrap_or_else(|| model_path(&settings.engine, &settings.model))
}

pub fn availability(settings: &LocalSettings) -> ModelAvailability {
    let path = resolve_model_path(settings);
    if path.is_file() {
        ModelAvailability::Ready(path)
    } else if settings.model_path.is_some() {
        ModelAvailability::MissingConfiguredPath(path)
    } else {
        ModelAvailability::NotDownloaded(path)
    }
}

pub struct ModelDownload {
    progress_bytes: Arc<AtomicU64>,
    cancelled: Arc<AtomicBool>,
    promise: Promise<Result<PathBuf, String>>,
}

impl ModelDownload {
    pub fn start(entry: ModelManifestEntry, destination: PathBuf) -> Self {
        let progress_bytes = Arc::new(AtomicU64::new(0));
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_progress = progress_bytes.clone();
        let task_cancelled = cancelled.clone();
        let promise = Promise::spawn_thread("dictaite-model-download", move || {
            download_model(&entry, &destination, &task_progress, &task_cancelled)
        });
        Self {
            progress_bytes,
            cancelled,
            promise,
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn progress(&self, total: u64) -> (u64, f32) {
        let bytes = self.progress_bytes.load(Ordering::Acquire);
        let fraction = if total == 0 {
            0.0
        } else {
            (bytes as f32 / total as f32).clamp(0.0, 1.0)
        };
        (bytes, fraction)
    }

    pub fn ready(&self) -> Option<&Result<PathBuf, String>> {
        self.promise.ready()
    }
}

fn download_model(
    entry: &ModelManifestEntry,
    destination: &Path,
    progress: &AtomicU64,
    cancelled: &AtomicBool,
) -> Result<PathBuf, String> {
    let mut response = reqwest::blocking::get(entry.url)
        .map_err(|err| format!("Model download failed: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Model download failed: {err}"))?;
    install_stream(entry, destination, progress, cancelled, &mut response)
}

fn install_stream(
    entry: &ModelManifestEntry,
    destination: &Path,
    progress: &AtomicU64,
    cancelled: &AtomicBool,
    mut input: impl Read,
) -> Result<PathBuf, String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "Model destination has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    let part_path = destination.with_extension("bin.part");
    let result = (|| {
        let mut output = File::create(&part_path).map_err(|err| err.to_string())?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("Model download cancelled".to_string());
            }
            let read = input.read(&mut buffer).map_err(|err| err.to_string())?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .map_err(|err| err.to_string())?;
            hasher.update(&buffer[..read]);
            progress.fetch_add(read as u64, Ordering::Release);
        }
        output.sync_all().map_err(|err| err.to_string())?;
        let actual = format!("{:x}", hasher.finalize());
        if actual != entry.sha256 {
            return Err(format!(
                "Model verification failed: expected {}, got {actual}",
                entry.sha256
            ));
        }
        fs::rename(&part_path, destination).map_err(|err| err.to_string())?;
        Ok(destination.to_path_buf())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_model_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("dictaite-model-{unique}"))
            .join(name)
    }

    #[test]
    fn override_path_takes_precedence() {
        let settings = LocalSettings {
            model: "small".into(),
            model_path: Some(PathBuf::from("/custom/model.bin")),
            ..LocalSettings::default()
        };
        assert_eq!(
            resolve_model_path(&settings),
            PathBuf::from("/custom/model.bin")
        );
    }

    #[test]
    fn model_path_uses_application_home_layout() {
        let path = model_path("whisper", "base.en");
        assert!(path.ends_with("models/whisper/base.en/ggml-base.en.bin"));
    }

    #[test]
    fn model_path_honours_dictaite_home() {
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();
        let root = temp_model_path("home");
        let previous = std::env::var_os("DICTAITE_HOME");
        std::env::set_var("DICTAITE_HOME", &root);
        let path = model_path("whisper", "base.en");
        match previous {
            Some(value) => std::env::set_var("DICTAITE_HOME", value),
            None => std::env::remove_var("DICTAITE_HOME"),
        }
        assert!(path.starts_with(root));
    }

    #[test]
    fn unavailable_override_is_reported_distinctly() {
        let settings = LocalSettings {
            model_path: Some(PathBuf::from("/path/that/does/not/exist.bin")),
            ..LocalSettings::default()
        };
        assert!(matches!(
            availability(&settings),
            ModelAvailability::MissingConfiguredPath(_)
        ));
    }

    #[test]
    fn checksum_mismatch_discards_partial_file() {
        let destination = temp_model_path("model.bin");
        let entry = ModelManifestEntry {
            id: "test",
            display_name: "Test",
            url: "unused",
            size: 3,
            sha256: "incorrect",
        };
        let result = install_stream(
            &entry,
            &destination,
            &AtomicU64::new(0),
            &AtomicBool::new(false),
            Cursor::new(b"abc"),
        );
        assert!(result.unwrap_err().contains("verification failed"));
        assert!(!destination.with_extension("bin.part").exists());
        let _ = fs::remove_dir_all(destination.parent().unwrap());
    }

    #[test]
    fn cancellation_discards_partial_file() {
        let destination = temp_model_path("model.bin");
        let entry = ModelManifestEntry {
            id: "test",
            display_name: "Test",
            url: "unused",
            size: 3,
            sha256: "unused",
        };
        let result = install_stream(
            &entry,
            &destination,
            &AtomicU64::new(0),
            &AtomicBool::new(true),
            Cursor::new(b"abc"),
        );
        assert_eq!(result.unwrap_err(), "Model download cancelled");
        assert!(!destination.with_extension("bin.part").exists());
        let _ = fs::remove_dir_all(destination.parent().unwrap());
    }
}
