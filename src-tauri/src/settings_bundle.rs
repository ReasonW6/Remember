use crate::{
    advanced_settings::{self, AdvancedSettings},
    hotkeys::{self, HotkeyConfig},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
};
use tauri::{AppHandle, Manager};

const SETTINGS_BUNDLE_FILE: &str = "preferences.json";
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsBundle {
    pub advanced: AdvancedSettings,
    pub hotkeys: HotkeyConfig,
}

pub fn load(app: &AppHandle) -> Result<SettingsBundle, String> {
    let path = config_path(app)?;
    if !path.exists() {
        let bundle = normalize(SettingsBundle {
            advanced: advanced_settings::load(app)?,
            hotkeys: hotkeys::load_config(app)?,
        })?;
        return save(app, bundle);
    }

    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let bundle = serde_json::from_str::<SettingsBundle>(&raw).map_err(|error| error.to_string())?;
    normalize(bundle)
}

pub fn save(app: &AppHandle, bundle: SettingsBundle) -> Result<SettingsBundle, String> {
    let bundle = normalize(bundle)?;
    let path = config_path(app)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "cannot determine settings directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;

    let json = serde_json::to_vec_pretty(&bundle).map_err(|error| error.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(SETTINGS_BUNDLE_FILE);
    let (temp_path, mut temp_file) = loop {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(".{file_name}.{}.{}.tmp", process::id(), sequence));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => break (temp_path, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    };

    let write_result = (|| -> Result<(), String> {
        temp_file
            .write_all(&json)
            .map_err(|error| error.to_string())?;
        temp_file.flush().map_err(|error| error.to_string())?;
        temp_file.sync_all().map_err(|error| error.to_string())?;
        drop(temp_file);
        crate::storage::atomic_replace(&temp_path, &path).map_err(|error| error.to_string())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result?;
    Ok(bundle)
}

pub fn normalize(bundle: SettingsBundle) -> Result<SettingsBundle, String> {
    Ok(SettingsBundle {
        advanced: advanced_settings::normalize(bundle.advanced)?,
        hotkeys: hotkeys::normalize_config(&bundle.hotkeys)?,
    })
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join(SETTINGS_BUNDLE_FILE))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_both_halves_before_a_bundle_can_be_saved() {
        let invalid_volume = normalize(SettingsBundle {
            advanced: AdvancedSettings {
                feedback_volume_percent: 101,
                ..AdvancedSettings::default()
            },
            hotkeys: HotkeyConfig::default(),
        });
        assert!(invalid_volume.is_err());

        let invalid_hotkeys = normalize(SettingsBundle {
            advanced: AdvancedSettings::default(),
            hotkeys: HotkeyConfig {
                record: "F8".to_string(),
                playback: "F8".to_string(),
                stop: "F8".to_string(),
            },
        });
        assert!(invalid_hotkeys.is_err());
    }

    #[test]
    fn normalizes_a_complete_bundle() {
        let bundle = normalize(SettingsBundle {
            advanced: AdvancedSettings::default(),
            hotkeys: HotkeyConfig {
                record: "ctrl+shift+r".to_string(),
                playback: "F12".to_string(),
                stop: "ctrl+shift+r".to_string(),
            },
        })
        .expect("normalize bundle");

        assert_eq!(bundle.hotkeys.record, "Ctrl+Shift+R");
        assert_eq!(bundle.hotkeys.stop, "Ctrl+Shift+R");
    }
}
