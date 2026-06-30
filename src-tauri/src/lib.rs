pub mod app_state;
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
            commands::open_recording,
            commands::save_current_recording,
            commands::start_playback,
            commands::stop_playback,
        ])
        .setup(move |app| {
            tray::setup(app.handle())
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            hotkeys::register(app.handle())
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            let capture_runtime = input::start_capture(capture_shared.clone())
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            if !app.manage(Mutex::new(capture_runtime)) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "input capture runtime already managed",
                )
                .into());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
