pub mod app_state;
pub mod clock;
pub mod commands;
pub mod hotkeys;
pub mod input;
pub mod model;
pub mod player;
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
    let capture_shared = shared.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(shared)
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
            commands::start_playback,
            commands::set_playback_settings,
            commands::stop_playback,
        ])
        .setup(move |app| {
            tray::setup(app.handle()).map_err(std::io::Error::other)?;
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
            #[cfg(not(target_os = "windows"))]
            let main_window_hwnd = None;

            let capture_runtime = input::start_capture(
                capture_shared.clone(),
                app.handle().clone(),
                main_window_hwnd,
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
