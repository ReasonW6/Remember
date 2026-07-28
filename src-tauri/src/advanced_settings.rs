use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager};

const SETTINGS_FILE: &str = "settings.json";

pub type SharedAdvancedSettings = Arc<Mutex<AdvancedSettings>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdvancedSettings {
    pub feedback_volume_percent: u8,
    pub feedback_muted: bool,
    pub show_activity_indicator: bool,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            feedback_volume_percent: 50,
            feedback_muted: false,
            show_activity_indicator: true,
        }
    }
}

pub fn load(app: &AppHandle) -> Result<AdvancedSettings, String> {
    let path = config_path(app)?;
    if !path.exists() {
        return Ok(AdvancedSettings::default());
    }

    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(settings_from_json_or_default(&raw))
}

pub fn save(app: &AppHandle, settings: AdvancedSettings) -> Result<AdvancedSettings, String> {
    let settings = normalize(settings)?;
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(&settings).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())?;
    Ok(settings)
}

pub fn current(app: &AppHandle) -> Result<AdvancedSettings, String> {
    let state = app
        .try_state::<SharedAdvancedSettings>()
        .ok_or_else(|| "advanced settings state is unavailable".to_string())?;
    let settings = state
        .lock()
        .map_err(|_| "advanced settings lock poisoned".to_string())?;
    Ok(*settings)
}

pub fn replace(app: &AppHandle, settings: AdvancedSettings) -> Result<(), String> {
    let state = app
        .try_state::<SharedAdvancedSettings>()
        .ok_or_else(|| "advanced settings state is unavailable".to_string())?;
    let mut current = state
        .lock()
        .map_err(|_| "advanced settings lock poisoned".to_string())?;
    *current = settings;
    Ok(())
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join(SETTINGS_FILE))
        .map_err(|error| error.to_string())
}

pub(crate) fn normalize(settings: AdvancedSettings) -> Result<AdvancedSettings, String> {
    if settings.feedback_volume_percent > 100 {
        return Err("feedback volume must be between 0 and 100".to_string());
    }
    Ok(settings)
}

fn settings_from_json_or_default(raw: &str) -> AdvancedSettings {
    match serde_json::from_str::<AdvancedSettings>(raw)
        .and_then(|settings| normalize(settings).map_err(serde::de::Error::custom))
    {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("Remember advanced settings ignored: {error}");
            AdvancedSettings::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_the_current_sound_level_and_show_the_indicator() {
        assert_eq!(
            AdvancedSettings::default(),
            AdvancedSettings {
                feedback_volume_percent: 50,
                feedback_muted: false,
                show_activity_indicator: true,
            }
        );
    }

    #[test]
    fn older_partial_settings_receive_defaults_for_new_fields() {
        let settings = settings_from_json_or_default(r#"{"feedback_volume_percent":25}"#);

        assert_eq!(
            settings,
            AdvancedSettings {
                feedback_volume_percent: 25,
                feedback_muted: false,
                show_activity_indicator: true,
            }
        );
    }

    #[test]
    fn invalid_persisted_volume_falls_back_to_defaults() {
        let settings = settings_from_json_or_default(r#"{"feedback_volume_percent":101}"#);

        assert_eq!(settings, AdvancedSettings::default());
    }
}
