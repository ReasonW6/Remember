use crate::{
    app_state::AppMode,
    commands::{self, SharedApp},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn register(app: &AppHandle) -> Result<(), String> {
    let record_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyR);
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
                    let _ = commands::start_recording(app.clone(), state);
                }
            }
        })
        .map_err(|error| error.to_string())?;

    let playback_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyP);
    app.global_shortcut()
        .on_shortcut(playback_shortcut, |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }

            if let Some(state) = app.try_state::<SharedApp>() {
                let _ = commands::start_playback(app.clone(), state, 1, 1.0);
            }
        })
        .map_err(|error| error.to_string())?;

    let stop_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Escape);
    app.global_shortcut()
        .on_shortcut(stop_shortcut, |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }

            if let Some(state) = app.try_state::<SharedApp>() {
                let _ = commands::stop_playback(app.clone(), state);
            }
        })
        .map_err(|error| error.to_string())?;

    Ok(())
}
