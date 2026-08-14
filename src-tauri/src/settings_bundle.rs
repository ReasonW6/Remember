use crate::{
    advanced_settings::{self, AdvancedSettings},
    hotkeys::{self, HotkeyConfig},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};
use tauri::{AppHandle, Manager};

const SETTINGS_BUNDLE_FILE: &str = "preferences.json";
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainWindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MainWindowPreferences {
    pub compact: bool,
    pub position: Option<MainWindowPosition>,
}

impl Default for MainWindowPreferences {
    fn default() -> Self {
        Self {
            compact: true,
            position: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsBundle {
    pub advanced: AdvancedSettings,
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub main_window: MainWindowPreferences,
}

pub fn load(app: &AppHandle) -> Result<SettingsBundle, String> {
    let path = config_path(app)?;
    if !path.exists() {
        let bundle = normalize(SettingsBundle {
            advanced: advanced_settings::load(app)?,
            hotkeys: hotkeys::load_config(app)?,
            main_window: MainWindowPreferences::default(),
        })?;
        return save(app, bundle);
    }

    match read_existing_bundle(&path) {
        Ok(bundle) => Ok(bundle),
        Err(error) => {
            let backup = quarantine_invalid_bundle(&path);
            match &backup {
                Ok(backup) => eprintln!(
                    "Remember ignored invalid preferences at {} ({error}); preserved them at {}.",
                    path.display(),
                    backup.display()
                ),
                Err(backup_error) => eprintln!(
                    "Remember ignored invalid preferences at {} ({error}); the invalid file could not be quarantined: {backup_error}.",
                    path.display()
                ),
            }

            let fallback = normalize(SettingsBundle {
                advanced: AdvancedSettings::default(),
                hotkeys: HotkeyConfig::default(),
                main_window: MainWindowPreferences::default(),
            })?;
            if backup.is_ok() {
                if let Err(save_error) = save(app, fallback.clone()) {
                    eprintln!(
                        "Remember could not persist recovered default preferences: {save_error}."
                    );
                }
            }
            Ok(fallback)
        }
    }
}

fn read_existing_bundle(path: &Path) -> Result<SettingsBundle, String> {
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let bundle = serde_json::from_str::<SettingsBundle>(&raw).map_err(|error| error.to_string())?;
    normalize(bundle)
}

fn quarantine_invalid_bundle(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "cannot determine settings directory".to_string())?;
    for suffix in 0_u64.. {
        let file_name = if suffix == 0 {
            "preferences.invalid.json".to_string()
        } else {
            format!("preferences.invalid.{suffix}.json")
        };
        let backup = parent.join(file_name);
        if backup.exists() {
            continue;
        }
        fs::rename(path, &backup).map_err(|error| error.to_string())?;
        return Ok(backup);
    }
    unreachable!("the quarantine suffix space cannot be exhausted")
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
        main_window: bundle.main_window,
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "remember-settings-{name}-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ))
    }

    #[test]
    fn validates_both_halves_before_a_bundle_can_be_saved() {
        let invalid_volume = normalize(SettingsBundle {
            advanced: AdvancedSettings {
                feedback_volume_percent: 101,
                ..AdvancedSettings::default()
            },
            hotkeys: HotkeyConfig::default(),
            main_window: MainWindowPreferences::default(),
        });
        assert!(invalid_volume.is_err());

        let invalid_hotkeys = normalize(SettingsBundle {
            advanced: AdvancedSettings::default(),
            hotkeys: HotkeyConfig {
                record: "F8".to_string(),
                playback: "F8".to_string(),
                stop: "F8".to_string(),
            },
            main_window: MainWindowPreferences::default(),
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
            main_window: MainWindowPreferences {
                compact: false,
                position: Some(MainWindowPosition { x: -1200, y: 80 }),
            },
        })
        .expect("normalize bundle");

        assert_eq!(bundle.hotkeys.record, "Ctrl+Shift+R");
        assert_eq!(bundle.hotkeys.stop, "Ctrl+Shift+R");
        assert_eq!(
            bundle.main_window,
            MainWindowPreferences {
                compact: false,
                position: Some(MainWindowPosition { x: -1200, y: 80 }),
            }
        );
    }

    #[test]
    fn older_bundles_default_to_a_compact_window_without_a_saved_position() {
        let bundle: SettingsBundle = serde_json::from_str(
            r#"{
                "advanced": {
                    "feedback_volume_percent": 50,
                    "feedback_muted": false,
                    "show_activity_indicator": true,
                    "window_relative_recording_enabled": false
                },
                "hotkeys": {"record":"F8","playback":"F12","stop":"F8"}
            }"#,
        )
        .expect("legacy settings bundle");

        assert_eq!(bundle.main_window, MainWindowPreferences::default());
    }

    #[test]
    fn invalid_preferences_are_quarantined_for_startup_recovery() {
        let directory = test_directory("quarantine");
        fs::create_dir_all(&directory).expect("create test directory");
        let path = directory.join(SETTINGS_BUNDLE_FILE);
        fs::write(&path, "{bad").expect("write invalid preferences");

        assert!(read_existing_bundle(&path).is_err());
        let backup = quarantine_invalid_bundle(&path).expect("quarantine invalid preferences");

        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(backup).expect("read quarantined preferences"),
            "{bad"
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
