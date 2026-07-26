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
pub mod storage;
pub mod tray;

use app_state::AppController;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
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
            commands::set_hotkeys,
            commands::get_advanced_settings,
            commands::set_advanced_settings,
            commands::show_advanced_settings,
            commands::get_privilege_state,
            commands::restart_as_administrator,
            commands::start_playback,
            commands::set_playback_settings,
            commands::stop_playback,
        ])
        .on_window_event(|window, event| {
            if window.label() == "advanced-settings" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            tray::setup(app.handle()).map_err(std::io::Error::other)?;
            let loaded_settings =
                advanced_settings::load(app.handle()).map_err(std::io::Error::other)?;
            advanced_settings::replace(app.handle(), loaded_settings)
                .map_err(std::io::Error::other)?;
            activity_indicator::setup(app.handle()).map_err(std::io::Error::other)?;
            let hotkey_config =
                hotkeys::load_config(app.handle()).map_err(std::io::Error::other)?;
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
