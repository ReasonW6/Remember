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

use app_state::AppController;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

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
            if window.label() == "advanced-settings"
                && matches!(event, tauri::WindowEvent::Destroyed)
            {
                if let Some(handles) = window
                    .app_handle()
                    .try_state::<Arc<input::OwnWindowHandles>>()
                {
                    handles.clear_advanced_settings();
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match close_behavior(window.label()) {
                    CloseBehavior::ExitApplication => {
                        api.prevent_close();
                        let app = window.app_handle();
                        match commands::prepare_for_exit(app) {
                            Ok(()) => app.exit(0),
                            Err(error) => commands::report_exit_failure(app, error),
                        }
                    }
                    CloseBehavior::HideWindow => {
                        api.prevent_close();
                        if let Err(error) = window.hide() {
                            eprintln!("Remember could not hide window {}: {error}", window.label());
                        }
                    }
                    CloseBehavior::Default => {}
                }
            }
        })
        .setup(move |app| {
            single_instance.listen_for_activation(app.handle().clone())?;
            let loaded_settings =
                settings_bundle::load(app.handle()).map_err(std::io::Error::other)?;
            advanced_settings::replace(app.handle(), loaded_settings.advanced)
                .map_err(std::io::Error::other)?;
            activity_indicator::setup(app.handle()).map_err(std::io::Error::other)?;
            let hotkey_config = loaded_settings.hotkeys;
            hotkeys::apply_to_controller(app.handle(), &hotkey_config)
                .map_err(std::io::Error::other)?;
            hotkeys::register(app.handle(), &hotkey_config, true).map_err(std::io::Error::other)?;
            let own_windows = Arc::new(input::OwnWindowHandles::default());
            #[cfg(target_os = "windows")]
            let main_window_hwnd = app
                .get_webview_window("main")
                .and_then(|window| window.hwnd().ok())
                .map(|hwnd| hwnd.0 as usize);
            #[cfg(not(target_os = "windows"))]
            let main_window_hwnd = None;
            if let Some(hwnd) = main_window_hwnd {
                own_windows.set_main(hwnd);
            }
            if !app.manage(own_windows.clone()) {
                return Err(std::io::Error::other("own-window handles already managed").into());
            }

            let capture_runtime =
                input::start_capture(capture_shared.clone(), app.handle().clone(), own_windows)
                    .map_err(std::io::Error::other)?;
            if !app.manage(Mutex::new(capture_runtime)) {
                return Err(std::io::Error::other("input capture runtime already managed").into());
            }
            commands::restore_administrator_restart_recovery_in_background(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CloseBehavior {
    ExitApplication,
    HideWindow,
    Default,
}

fn close_behavior(label: &str) -> CloseBehavior {
    match label {
        "main" => CloseBehavior::ExitApplication,
        "advanced-settings" => CloseBehavior::HideWindow,
        _ => CloseBehavior::Default,
    }
}

pub(crate) fn show_main_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{close_behavior, CloseBehavior};

    #[test]
    fn main_window_exits_while_child_window_only_hides() {
        assert_eq!(close_behavior("main"), CloseBehavior::ExitApplication);
        assert_eq!(
            close_behavior("advanced-settings"),
            CloseBehavior::HideWindow
        );
        assert_eq!(close_behavior("activity-indicator"), CloseBehavior::Default);
    }
}
