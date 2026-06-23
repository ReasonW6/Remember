pub mod hotkeys;
mod keyboard;
mod mouse;
pub mod playback_input;
pub mod player;
pub mod recorder;
pub mod storage;
pub mod windows;

use hotkeys::EmergencyHotkeyOutcome;
use player::{PlaybackControlPayload, PlaybackOptions, PlaybackStartPayload, PlayerState};
use recorder::{RecorderState, RecordingStartPayload, RecordingStopPayload, ScreenRect};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use storage::{Flow, FlowSummary, SavedFlow};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Position, State, WebviewWindow};

#[derive(Serialize)]
struct AppStatusPayload {
    status: &'static str,
    label: &'static str,
}

#[tauri::command]
fn show_workbench(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("workbench")
        .ok_or_else(|| "workbench window was not created".to_string())?;

    window.show().map_err(|error| error.to_string())?;
    if let Some(control) = app.get_webview_window("control") {
        place_workbench_pair(&app, &window, &control)?;
    }
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

fn place_workbench_pair(
    app: &AppHandle,
    workbench: &WebviewWindow,
    control: &WebviewWindow,
) -> Result<(), String> {
    let monitor = workbench
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(app.primary_monitor().map_err(|error| error.to_string())?)
        .ok_or_else(|| "primary monitor was not found".to_string())?;

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let margin = 48;
    let gap = 32;
    let workbench_width = 1220;
    let control_width = 520;

    let workbench_x = monitor_position.x + margin;
    let workbench_y = monitor_position.y + margin;
    let right_limit = monitor_position.x + monitor_size.width as i32 - control_width - margin;
    let preferred_control_x = workbench_x + workbench_width + gap;
    let control_x = preferred_control_x
        .min(right_limit)
        .max(monitor_position.x + margin);
    let control_y = workbench_y + 92;

    workbench
        .set_position(Position::Physical(PhysicalPosition::new(
            workbench_x,
            workbench_y,
        )))
        .map_err(|error| error.to_string())?;
    control
        .set_position(Position::Physical(PhysicalPosition::new(
            control_x, control_y,
        )))
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
fn focus_control(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("control")
        .ok_or_else(|| "control window was not created".to_string())?;

    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

fn app_data_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_initial_flow(app: AppHandle) -> Result<SavedFlow, String> {
    let root = app_data_root(&app)?;
    storage::ensure_default_flow_in_dir(&root).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_flows(app: AppHandle) -> Result<Vec<FlowSummary>, String> {
    let root = app_data_root(&app)?;
    storage::list_flow_summaries_in_dir(&root).map_err(|error| error.to_string())
}

#[tauri::command]
fn load_flow(app: AppHandle, file_name: String) -> Result<SavedFlow, String> {
    let root = app_data_root(&app)?;
    storage::load_flow_file(&root, &file_name).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_flow(app: AppHandle, flow: Flow) -> Result<SavedFlow, String> {
    let root = app_data_root(&app)?;
    storage::save_flow_to_dir(&root, &flow).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_flow_as(app: AppHandle, flow: Flow, display_name: String) -> Result<SavedFlow, String> {
    let root = app_data_root(&app)?;
    storage::save_flow_as_to_dir(&root, &flow, &display_name).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_flow(app: AppHandle, display_name: String) -> Result<SavedFlow, String> {
    let root = app_data_root(&app)?;
    storage::create_flow_in_dir(&root, &display_name).map_err(|error| error.to_string())
}

#[tauri::command]
fn start_recording(
    app: AppHandle,
    recorder: State<'_, Mutex<RecorderState>>,
) -> Result<RecordingStartPayload, String> {
    let mut recorder = recorder
        .lock()
        .map_err(|_| "recorder state is unavailable".to_string())?;
    let target_window = windows::active_window_target();
    let payload = recorder
        .start_with_target_window(target_window)
        .map_err(|error| error.to_string())?;
    if let Err(error) = recorder
        .enable_mouse_capture()
        .and_then(|_| recorder.enable_keyboard_capture())
    {
        let _ = recorder.stop();
        return Err(error.to_string());
    }
    app.emit("recording-started", &payload)
        .map_err(|error| error.to_string())?;
    Ok(payload)
}

#[tauri::command]
fn stop_recording(
    app: AppHandle,
    recorder: State<'_, Mutex<RecorderState>>,
) -> Result<RecordingStopPayload, String> {
    let mut recorder = recorder
        .lock()
        .map_err(|_| "recorder state is unavailable".to_string())?;
    let excluded_regions = app_window_regions(&app);
    let payload = recorder
        .stop_excluding_regions(&excluded_regions)
        .map_err(|error| error.to_string())?;
    app.emit("recording-stopped", &payload)
        .map_err(|error| error.to_string())?;
    Ok(payload)
}

#[tauri::command]
fn start_playback(
    app: AppHandle,
    player: State<'_, Mutex<PlayerState>>,
    flow: Flow,
    speed_multiplier: f64,
    loop_count: u32,
    infinite_loop_confirmed: bool,
) -> Result<PlaybackStartPayload, String> {
    let mut player = player
        .lock()
        .map_err(|_| "player state is unavailable".to_string())?;
    let finished_app = app.clone();
    let payload = player
        .start(
            flow,
            PlaybackOptions {
                speed_multiplier,
                loop_count,
                infinite_loop_confirmed,
            },
            move |finished| {
                let _ = finished_app.emit("playback-finished", &finished);
            },
        )
        .map_err(|error| error.to_string())?;
    app.emit("playback-started", &payload)
        .map_err(|error| error.to_string())?;
    Ok(payload)
}

#[tauri::command]
fn stop_playback(
    app: AppHandle,
    player: State<'_, Mutex<PlayerState>>,
) -> Result<PlaybackControlPayload, String> {
    let mut player = player
        .lock()
        .map_err(|_| "player state is unavailable".to_string())?;
    let payload = player.stop().map_err(|error| error.to_string())?;
    app.emit("playback-stopped", &payload)
        .map_err(|error| error.to_string())?;
    Ok(payload)
}

#[tauri::command]
fn emergency_stop_playback(
    app: AppHandle,
    player: State<'_, Mutex<PlayerState>>,
) -> Result<PlaybackControlPayload, String> {
    let mut player = player
        .lock()
        .map_err(|_| "player state is unavailable".to_string())?;
    let payload = player.emergency_stop().map_err(|error| error.to_string())?;
    app.emit("playback-stopped", &payload)
        .map_err(|error| error.to_string())?;
    Ok(payload)
}

fn app_window_regions(app: &AppHandle) -> Vec<ScreenRect> {
    ["control", "workbench"]
        .iter()
        .filter_map(|label| app.get_webview_window(label))
        .filter(|window| window.is_visible().unwrap_or(false))
        .filter_map(|window| {
            let position = window.outer_position().ok()?;
            let size = window.outer_size().ok()?;
            Some(ScreenRect {
                left: position.x,
                top: position.y,
                right: position.x + size.width as i32,
                bottom: position.y + size.height as i32,
            })
        })
        .collect()
}

#[tauri::command]
fn status_ready() -> AppStatusPayload {
    AppStatusPayload {
        status: "ready",
        label: "就绪",
    }
}

#[tauri::command]
fn status_recording() -> AppStatusPayload {
    AppStatusPayload {
        status: "recording",
        label: "录制中",
    }
}

#[tauri::command]
fn status_playing() -> AppStatusPayload {
    AppStatusPayload {
        status: "playing",
        label: "回放中",
    }
}

#[tauri::command]
fn status_stopped() -> AppStatusPayload {
    AppStatusPayload {
        status: "stopped",
        label: "已停止",
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(RecorderState::default()))
        .manage(Mutex::new(PlayerState::default()))
        .setup(|app| {
            let app_handle = app.handle().clone();
            match hotkeys::start_emergency_hotkey(move || {
                let player = app_handle.state::<Mutex<PlayerState>>();
                let Ok(mut player) = player.lock() else {
                    return;
                };
                if let EmergencyHotkeyOutcome::Stopped(payload) =
                    hotkeys::trigger_emergency_stop(&mut player)
                {
                    let _ = app_handle.emit("playback-stopped", &payload);
                }
            }) {
                Ok(guard) => {
                    app.manage(Mutex::new(guard));
                }
                Err(error) => eprintln!("global emergency hotkey unavailable: {error}"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_workbench,
            focus_control,
            get_initial_flow,
            list_flows,
            load_flow,
            save_flow,
            save_flow_as,
            create_flow,
            start_recording,
            stop_recording,
            start_playback,
            stop_playback,
            emergency_stop_playback,
            status_ready,
            status_recording,
            status_playing,
            status_stopped
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Remember app");
}
