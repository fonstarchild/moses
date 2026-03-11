/// Persistent settings — saved to ~/.moses/settings.json
/// Stores: last model, last workspace path.
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub model: Option<String>,
    pub workspace: Option<String>,
}

fn settings_path() -> PathBuf {
    moses_data_dir().join("settings.json")
}

pub fn moses_data_dir() -> PathBuf {
    // Windows: %APPDATA%\Moses
    // macOS/Linux: ~/.moses
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Public"))
            .join("Moses")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
            .join(".moses")
    }
}

pub fn load() -> Settings {
    let path = settings_path();
    let Ok(data) = std::fs::read_to_string(&path) else {
        return Settings::default();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save(settings: &Settings) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(&path, json);
    }
}
