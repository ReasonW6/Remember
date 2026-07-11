#[cfg(not(target_os = "windows"))]
use crate::{app_state::AppMode, commands};
use crate::{
    app_state::{ControlHotkey, ControlHotkeyModifiers},
    commands::SharedApp,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, str::FromStr};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
#[cfg(not(target_os = "windows"))]
use tauri_plugin_global_shortcut::{ShortcutEvent, ShortcutState};

#[cfg(any(test, not(target_os = "windows")))]
const HOTKEY_ALREADY_REGISTERED: &str = "HotKey already registered";
const HOTKEY_CONFIG_FILE: &str = "hotkeys.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub record: String,
    pub playback: String,
    pub stop: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            record: "F8".to_string(),
            playback: "F12".to_string(),
            stop: "F8".to_string(),
        }
    }
}

impl HotkeyConfig {
    pub fn control_hotkeys(&self) -> Result<Vec<ControlHotkey>, String> {
        let config = normalize_config(self)?;
        let mut hotkeys = vec![
            control_hotkey_from_shortcut(&config.record)?,
            control_hotkey_from_shortcut(&config.playback)?,
        ];
        let stop_hotkey = control_hotkey_from_shortcut(&config.stop)?;
        if !hotkeys.contains(&stop_hotkey) {
            hotkeys.push(stop_hotkey);
        }
        Ok(hotkeys)
    }

    pub fn record_hotkey(&self) -> Result<ControlHotkey, String> {
        let config = normalize_config(self)?;
        control_hotkey_from_shortcut(&config.record)
    }

    pub fn playback_hotkey(&self) -> Result<ControlHotkey, String> {
        let config = normalize_config(self)?;
        control_hotkey_from_shortcut(&config.playback)
    }

    pub fn stop_hotkey(&self) -> Result<ControlHotkey, String> {
        let config = normalize_config(self)?;
        control_hotkey_from_shortcut(&config.stop)
    }
}

#[cfg(any(test, not(target_os = "windows")))]
fn allow_registered_hotkey_conflict(result: Result<(), String>) -> Result<(), String> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.contains(HOTKEY_ALREADY_REGISTERED) => {
            eprintln!("Remember hotkey unavailable: {error}");
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub fn load_config(app: &AppHandle) -> Result<HotkeyConfig, String> {
    let path = config_path(app)?;
    if !path.exists() {
        return Ok(HotkeyConfig::default());
    }

    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    Ok(config_from_json_or_default(&raw))
}

fn config_from_json_or_default(raw: &str) -> HotkeyConfig {
    let loaded = serde_json::from_str::<HotkeyConfig>(raw)
        .map_err(|error| error.to_string())
        .and_then(|config| normalize_config(&config));
    match loaded {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Remember hotkey config ignored: {error}");
            HotkeyConfig::default()
        }
    }
}

pub fn save_config(app: &AppHandle, config: &HotkeyConfig) -> Result<(), String> {
    let normalized = normalize_config(config)?;
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(&normalized).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())
}

pub fn normalize_config(config: &HotkeyConfig) -> Result<HotkeyConfig, String> {
    let normalized = HotkeyConfig {
        record: canonical_shortcut(&config.record)?,
        playback: canonical_shortcut(&config.playback)?,
        stop: canonical_shortcut(&config.stop)?,
    };
    if normalized.playback == normalized.record || normalized.playback == normalized.stop {
        return Err("playback hotkey must be different from record and stop hotkeys".to_string());
    }
    Ok(normalized)
}

pub fn register(
    app: &AppHandle,
    config: &HotkeyConfig,
    allow_conflicts: bool,
) -> Result<(), String> {
    let config = normalize_config(config)?;
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        let _ = allow_conflicts;
        let _ = config;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let record_shortcut = config.record.clone();
        let playback_shortcut = config.playback.clone();
        let stop_shortcut = config.stop.clone();
        let record_is_stop = record_shortcut == stop_shortcut;

        register_one(
            app,
            &record_shortcut,
            allow_conflicts,
            move |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed || main_window_focused(app) {
                    return;
                }

                if let Some(state) = app.try_state::<SharedApp>() {
                    let mode = match state.lock() {
                        Ok(controller) => controller.mode(),
                        Err(_) => return,
                    };

                    match mode {
                        AppMode::Idle => {
                            let _ = commands::start_recording_from_hotkey(app.clone(), state);
                        }
                        AppMode::Recording if record_is_stop => {
                            let _ = commands::stop_recording(app.clone(), state);
                        }
                        AppMode::Playing if record_is_stop => {
                            let _ = commands::stop_active(app.clone(), state);
                        }
                        AppMode::Recording | AppMode::Playing => {}
                    }
                }
            },
        )?;

        register_one(
            app,
            &playback_shortcut,
            allow_conflicts,
            |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed || main_window_focused(app) {
                    return;
                }

                if let Some(state) = app.try_state::<SharedApp>() {
                    let _ =
                        commands::start_playback_current_shared(app.clone(), state.inner().clone());
                }
            },
        )?;

        if !record_is_stop {
            register_one(
                app,
                &stop_shortcut,
                allow_conflicts,
                |app, _shortcut, event| {
                    if event.state != ShortcutState::Pressed || main_window_focused(app) {
                        return;
                    }

                    if let Some(state) = app.try_state::<SharedApp>() {
                        let _ = commands::stop_active(app.clone(), state);
                    }
                },
            )?;
        }

        Ok(())
    }
}

pub fn unregister_all(app: &AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|error| error.to_string())
}

pub fn apply_to_controller(app: &AppHandle, config: &HotkeyConfig) -> Result<(), String> {
    let config = normalize_config(config)?;
    let hotkeys = config.control_hotkeys()?;
    let record_hotkey = config.record_hotkey()?;
    let playback_hotkey = config.playback_hotkey()?;
    let stop_hotkey = config.stop_hotkey()?;

    if let Some(state) = app.try_state::<SharedApp>() {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.set_control_hotkeys(hotkeys, record_hotkey, playback_hotkey, stop_hotkey);
    }

    Ok(())
}

// The window-level keydown listener in the frontend already handles hotkeys
// while the main window is focused, so the global shortcut defers to it to
// avoid double-triggering (mirrors the Windows hook's foreground check).
#[cfg(not(target_os = "windows"))]
fn main_window_focused(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn register_one<F>(
    app: &AppHandle,
    shortcut: &str,
    allow_conflicts: bool,
    handler: F,
) -> Result<(), String>
where
    F: Fn(&AppHandle, &Shortcut, ShortcutEvent) + Send + Sync + 'static,
{
    let result = app
        .global_shortcut()
        .on_shortcut(shortcut, handler)
        .map_err(|error| error.to_string());
    if allow_conflicts {
        allow_registered_hotkey_conflict(result)
    } else {
        result
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(HOTKEY_CONFIG_FILE))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
fn vk_code_for_key(key: &str) -> Result<u16, String> {
    Ok(control_hotkey_from_shortcut(&canonical_shortcut(key)?)?.vk_code)
}

fn canonical_shortcut(shortcut: &str) -> Result<String, String> {
    let trimmed = shortcut.trim();
    if trimmed.is_empty() {
        return Err("hotkey key cannot be empty".to_string());
    }
    let parsed = Shortcut::from_str(trimmed).map_err(|error| error.to_string())?;
    let vk_code = vk_code_for_code(parsed.key)
        .ok_or_else(|| format!("unsupported hotkey key {}", display_code(parsed.key)))?;
    if is_modifier_vk(vk_code) && parsed.mods.is_empty() {
        return Err("hotkey key cannot be only a modifier".to_string());
    }
    if parsed.mods.is_empty() && !is_function_key(parsed.key) {
        return Err("unmodified hotkey must be F1-F24".to_string());
    }
    Ok(display_shortcut(parsed))
}

fn is_function_key(code: Code) -> bool {
    matches!(
        code,
        Code::F1
            | Code::F2
            | Code::F3
            | Code::F4
            | Code::F5
            | Code::F6
            | Code::F7
            | Code::F8
            | Code::F9
            | Code::F10
            | Code::F11
            | Code::F12
            | Code::F13
            | Code::F14
            | Code::F15
            | Code::F16
            | Code::F17
            | Code::F18
            | Code::F19
            | Code::F20
            | Code::F21
            | Code::F22
            | Code::F23
            | Code::F24
    )
}

fn control_hotkey_from_shortcut(shortcut: &str) -> Result<ControlHotkey, String> {
    let parsed = Shortcut::from_str(shortcut).map_err(|error| error.to_string())?;
    let vk_code = vk_code_for_code(parsed.key)
        .ok_or_else(|| format!("unsupported hotkey key {}", display_code(parsed.key)))?;
    Ok(ControlHotkey {
        vk_code,
        modifiers: ControlHotkeyModifiers {
            ctrl: parsed.mods.contains(Modifiers::CONTROL),
            alt: parsed.mods.contains(Modifiers::ALT),
            shift: parsed.mods.contains(Modifiers::SHIFT),
            meta: parsed.mods.contains(Modifiers::SUPER),
        },
    })
}

fn display_shortcut(shortcut: Shortcut) -> String {
    let mut parts = Vec::new();
    if shortcut.mods.contains(Modifiers::CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if shortcut.mods.contains(Modifiers::ALT) {
        parts.push("Alt".to_string());
    }
    if shortcut.mods.contains(Modifiers::SHIFT) {
        parts.push("Shift".to_string());
    }
    if shortcut.mods.contains(Modifiers::SUPER) {
        parts.push("Win".to_string());
    }
    parts.push(display_code(shortcut.key));
    parts.join("+")
}

fn display_code(code: Code) -> String {
    match code {
        Code::KeyA => "A".to_string(),
        Code::KeyB => "B".to_string(),
        Code::KeyC => "C".to_string(),
        Code::KeyD => "D".to_string(),
        Code::KeyE => "E".to_string(),
        Code::KeyF => "F".to_string(),
        Code::KeyG => "G".to_string(),
        Code::KeyH => "H".to_string(),
        Code::KeyI => "I".to_string(),
        Code::KeyJ => "J".to_string(),
        Code::KeyK => "K".to_string(),
        Code::KeyL => "L".to_string(),
        Code::KeyM => "M".to_string(),
        Code::KeyN => "N".to_string(),
        Code::KeyO => "O".to_string(),
        Code::KeyP => "P".to_string(),
        Code::KeyQ => "Q".to_string(),
        Code::KeyR => "R".to_string(),
        Code::KeyS => "S".to_string(),
        Code::KeyT => "T".to_string(),
        Code::KeyU => "U".to_string(),
        Code::KeyV => "V".to_string(),
        Code::KeyW => "W".to_string(),
        Code::KeyX => "X".to_string(),
        Code::KeyY => "Y".to_string(),
        Code::KeyZ => "Z".to_string(),
        Code::Digit0 => "0".to_string(),
        Code::Digit1 => "1".to_string(),
        Code::Digit2 => "2".to_string(),
        Code::Digit3 => "3".to_string(),
        Code::Digit4 => "4".to_string(),
        Code::Digit5 => "5".to_string(),
        Code::Digit6 => "6".to_string(),
        Code::Digit7 => "7".to_string(),
        Code::Digit8 => "8".to_string(),
        Code::Digit9 => "9".to_string(),
        Code::Escape => "Esc".to_string(),
        Code::Space => "Space".to_string(),
        Code::Tab => "Tab".to_string(),
        Code::Enter => "Enter".to_string(),
        Code::Backspace => "Backspace".to_string(),
        Code::Delete => "Delete".to_string(),
        Code::Insert => "Insert".to_string(),
        Code::Home => "Home".to_string(),
        Code::End => "End".to_string(),
        Code::PageUp => "PageUp".to_string(),
        Code::PageDown => "PageDown".to_string(),
        Code::ArrowUp => "ArrowUp".to_string(),
        Code::ArrowDown => "ArrowDown".to_string(),
        Code::ArrowLeft => "ArrowLeft".to_string(),
        Code::ArrowRight => "ArrowRight".to_string(),
        Code::F1 => "F1".to_string(),
        Code::F2 => "F2".to_string(),
        Code::F3 => "F3".to_string(),
        Code::F4 => "F4".to_string(),
        Code::F5 => "F5".to_string(),
        Code::F6 => "F6".to_string(),
        Code::F7 => "F7".to_string(),
        Code::F8 => "F8".to_string(),
        Code::F9 => "F9".to_string(),
        Code::F10 => "F10".to_string(),
        Code::F11 => "F11".to_string(),
        Code::F12 => "F12".to_string(),
        Code::F13 => "F13".to_string(),
        Code::F14 => "F14".to_string(),
        Code::F15 => "F15".to_string(),
        Code::F16 => "F16".to_string(),
        Code::F17 => "F17".to_string(),
        Code::F18 => "F18".to_string(),
        Code::F19 => "F19".to_string(),
        Code::F20 => "F20".to_string(),
        Code::F21 => "F21".to_string(),
        Code::F22 => "F22".to_string(),
        Code::F23 => "F23".to_string(),
        Code::F24 => "F24".to_string(),
        _ => code.to_string(),
    }
}

fn vk_code_for_code(code: Code) -> Option<u16> {
    Some(match code {
        Code::KeyA => 0x41,
        Code::KeyB => 0x42,
        Code::KeyC => 0x43,
        Code::KeyD => 0x44,
        Code::KeyE => 0x45,
        Code::KeyF => 0x46,
        Code::KeyG => 0x47,
        Code::KeyH => 0x48,
        Code::KeyI => 0x49,
        Code::KeyJ => 0x4A,
        Code::KeyK => 0x4B,
        Code::KeyL => 0x4C,
        Code::KeyM => 0x4D,
        Code::KeyN => 0x4E,
        Code::KeyO => 0x4F,
        Code::KeyP => 0x50,
        Code::KeyQ => 0x51,
        Code::KeyR => 0x52,
        Code::KeyS => 0x53,
        Code::KeyT => 0x54,
        Code::KeyU => 0x55,
        Code::KeyV => 0x56,
        Code::KeyW => 0x57,
        Code::KeyX => 0x58,
        Code::KeyY => 0x59,
        Code::KeyZ => 0x5A,
        Code::Digit0 => 0x30,
        Code::Digit1 => 0x31,
        Code::Digit2 => 0x32,
        Code::Digit3 => 0x33,
        Code::Digit4 => 0x34,
        Code::Digit5 => 0x35,
        Code::Digit6 => 0x36,
        Code::Digit7 => 0x37,
        Code::Digit8 => 0x38,
        Code::Digit9 => 0x39,
        Code::Escape => 0x1B,
        Code::Space => 0x20,
        Code::Tab => 0x09,
        Code::Enter => 0x0D,
        Code::Backspace => 0x08,
        Code::Delete => 0x2E,
        Code::Insert => 0x2D,
        Code::Home => 0x24,
        Code::End => 0x23,
        Code::PageUp => 0x21,
        Code::PageDown => 0x22,
        Code::ArrowUp => 0x26,
        Code::ArrowDown => 0x28,
        Code::ArrowLeft => 0x25,
        Code::ArrowRight => 0x27,
        Code::F1 => 0x70,
        Code::F2 => 0x71,
        Code::F3 => 0x72,
        Code::F4 => 0x73,
        Code::F5 => 0x74,
        Code::F6 => 0x75,
        Code::F7 => 0x76,
        Code::F8 => 0x77,
        Code::F9 => 0x78,
        Code::F10 => 0x79,
        Code::F11 => 0x7A,
        Code::F12 => 0x7B,
        Code::F13 => 0x7C,
        Code::F14 => 0x7D,
        Code::F15 => 0x7E,
        Code::F16 => 0x7F,
        Code::F17 => 0x80,
        Code::F18 => 0x81,
        Code::F19 => 0x82,
        Code::F20 => 0x83,
        Code::F21 => 0x84,
        Code::F22 => 0x85,
        Code::F23 => 0x86,
        Code::F24 => 0x87,
        _ => return None,
    })
}

fn is_modifier_vk(vk_code: u16) -> bool {
    matches!(
        vk_code,
        0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_already_registered_hotkey_conflict() {
        let result = allow_registered_hotkey_conflict(Err(
            "HotKey already registered: HotKey { mods: Modifiers(ALT | CONTROL), key: KeyP }"
                .to_string(),
        ));

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn preserves_unrelated_hotkey_registration_errors() {
        let result =
            allow_registered_hotkey_conflict(Err("global shortcut backend failed".to_string()));

        assert_eq!(result, Err("global shortcut backend failed".to_string()));
    }

    #[test]
    fn normalizes_hotkey_config() {
        let config = normalize_config(&HotkeyConfig {
            record: "ctrl+shift+r".to_string(),
            playback: "F12".to_string(),
            stop: "ctrl+shift+r".to_string(),
        })
        .expect("normalize");

        assert_eq!(
            config,
            HotkeyConfig {
                record: "Ctrl+Shift+R".to_string(),
                playback: "F12".to_string(),
                stop: "Ctrl+Shift+R".to_string(),
            }
        );
    }

    #[test]
    fn defaults_to_single_key_hotkeys() {
        assert_eq!(
            HotkeyConfig::default(),
            HotkeyConfig {
                record: "F8".to_string(),
                playback: "F12".to_string(),
                stop: "F8".to_string(),
            }
        );
    }

    #[test]
    fn rejects_playback_duplicate_hotkey() {
        let error = normalize_config(&HotkeyConfig {
            record: "F6".to_string(),
            playback: "F6".to_string(),
            stop: "F8".to_string(),
        })
        .expect_err("duplicate keys");

        assert!(error.contains("playback hotkey must be different"));
    }

    #[test]
    fn rejects_unsafe_unmodified_hotkeys() {
        let error = normalize_config(&HotkeyConfig {
            record: "A".to_string(),
            playback: "F12".to_string(),
            stop: "F8".to_string(),
        })
        .expect_err("unmodified letter hotkey");

        assert!(error.contains("unmodified hotkey must be F1-F24"));
    }

    #[test]
    fn allows_unmodified_function_key_hotkeys() {
        assert!(normalize_config(&HotkeyConfig::default()).is_ok());
    }

    #[test]
    fn unsafe_persisted_hotkeys_fall_back_to_defaults() {
        let config = config_from_json_or_default(r#"{"record":"A","playback":"F12","stop":"F8"}"#);

        assert_eq!(config, HotkeyConfig::default());
    }

    #[test]
    fn maps_supported_hotkeys_to_windows_vk_codes() {
        assert_eq!(vk_code_for_key("Ctrl+R"), Ok(0x52));
        assert_eq!(vk_code_for_key("Ctrl+0"), Ok(0x30));
        assert_eq!(vk_code_for_key("F8"), Ok(0x77));
        assert_eq!(vk_code_for_key("Ctrl+Shift+R"), Ok(0x52));
    }
}
