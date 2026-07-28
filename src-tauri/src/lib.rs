pub mod activity_indicator;
pub mod advanced_settings;
pub mod app_state;
pub mod clock;
pub mod commands;
pub mod hotkeys;
pub mod input;
pub mod model;
pub mod player;
pub mod privileges;
pub mod recorder;
mod settings_bundle;
mod single_instance;
pub mod storage;
pub mod tray;

use app_state::AppController;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    let single_instance = match single_instance::acquire() {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(error) => panic!("Remember single-instance setup failed: {error}"),
    };
    let shared: commands::SharedApp = Arc::new(Mutex::new(AppController::new()));
    let advanced_settings: advanced_settings::SharedAdvancedSettings =
        Arc::new(Mutex::new(advanced_settings::AdvancedSettings::default()));
    let capture_shared = shared.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(shared)
        .manage(advanced_settings)
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::start_recording,
            commands::stop_recording,
            commands::list_recordings,
            commands::delete_recording,
            commands::rename_recording,
            commands::open_recording,
            commands::save_current_recording,
            commands::get_hotkeys,
            commands::get_advanced_settings,
            commands::get_settings_bundle,
            commands::set_settings_bundle,
            commands::show_advanced_settings,
            commands::get_privilege_state,
            commands::restart_as_administrator,
            commands::start_playback,
            commands::set_playback_settings,
            commands::stop_playback,
        ])
        .on_window_event(|window, event| {
            if hide_instead_of_close(window.label()) {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        eprintln!("Remember could not hide window {}: {error}", window.label());
                    }
                }
            }
        })
        .setup(move |app| {
            single_instance.listen_for_activation(app.handle().clone())?;
            if let Err(error) = commands::restore_administrator_restart_recovery(app.handle()) {
                eprintln!("Remember could not restore an administrator-restart recovery: {error}");
                if let Some(state) = app.try_state::<commands::SharedApp>() {
                    if let Ok(mut controller) = state.lock() {
                        controller.set_error(format!(
                            "Administrator-restart recovery was preserved but could not be restored: {error}"
                        ));
                    }
                }
            }
            tray::setup(app.handle()).map_err(std::io::Error::other)?;
            let loaded_settings =
                settings_bundle::load(app.handle()).map_err(std::io::Error::other)?;
            advanced_settings::replace(app.handle(), loaded_settings.advanced)
                .map_err(std::io::Error::other)?;
            activity_indicator::setup(app.handle()).map_err(std::io::Error::other)?;
            let hotkey_config = loaded_settings.hotkeys;
            hotkeys::apply_to_controller(app.handle(), &hotkey_config)
                .map_err(std::io::Error::other)?;
            hotkeys::register(app.handle(), &hotkey_config, true).map_err(std::io::Error::other)?;
            #[cfg(target_os = "windows")]
            let main_window_hwnd = app
                .get_webview_window("main")
                .and_then(|window| window.hwnd().ok())
                .map(|hwnd| hwnd.0 as usize);
            #[cfg(target_os = "windows")]
            let advanced_settings_hwnd = app
                .get_webview_window("advanced-settings")
                .and_then(|window| window.hwnd().ok())
                .map(|hwnd| hwnd.0 as usize);
            #[cfg(not(target_os = "windows"))]
            let main_window_hwnd = None;
            #[cfg(not(target_os = "windows"))]
            let advanced_settings_hwnd = None;

            let capture_runtime = input::start_capture(
                capture_shared.clone(),
                app.handle().clone(),
                [main_window_hwnd, advanced_settings_hwnd],
            )
            .map_err(std::io::Error::other)?;
            if !app.manage(Mutex::new(capture_runtime)) {
                return Err(std::io::Error::other("input capture runtime already managed").into());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}

fn hide_instead_of_close(label: &str) -> bool {
    matches!(label, "main" | "advanced-settings")
}

#[cfg(test)]
mod tests {
    use super::hide_instead_of_close;

    #[test]
    fn interactive_windows_hide_instead_of_being_destroyed() {
        assert!(hide_instead_of_close("main"));
        assert!(hide_instead_of_close("advanced-settings"));
        assert!(!hide_instead_of_close("activity-indicator"));
    }
}
