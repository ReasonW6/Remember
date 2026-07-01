use crate::{
    app_state::AppMode,
    commands::{self, SharedApp},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const HOTKEY_ALREADY_REGISTERED: &str = "HotKey already registered";

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

pub fn register(app: &AppHandle) -> Result<(), String> {
    let record_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyR);
    allow_registered_hotkey_conflict(
        app.global_shortcut()
            .on_shortcut(record_shortcut, |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }

                if let Some(state) = app.try_state::<SharedApp>() {
                    let is_recording = match state.lock() {
                        Ok(controller) => controller.mode() == AppMode::Recording,
                        Err(_) => return,
                    };

                    if is_recording {
                        let _ = commands::stop_recording(app.clone(), state);
                    } else {
                        let _ = commands::start_recording_from_hotkey(app.clone(), state);
                    }
                }
            })
            .map_err(|error| error.to_string()),
    )?;

    let playback_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyP);
    allow_registered_hotkey_conflict(
        app.global_shortcut()
            .on_shortcut(playback_shortcut, |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }

                if let Some(state) = app.try_state::<SharedApp>() {
                    let _ = commands::start_playback(app.clone(), state, 1, 1.0);
                }
            })
            .map_err(|error| error.to_string()),
    )?;

    let stop_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Escape);
    allow_registered_hotkey_conflict(
        app.global_shortcut()
            .on_shortcut(stop_shortcut, |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }

                if let Some(state) = app.try_state::<SharedApp>() {
                    let _ = commands::stop_active(app.clone(), state);
                }
            })
            .map_err(|error| error.to_string()),
    )?;

    Ok(())
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
}
