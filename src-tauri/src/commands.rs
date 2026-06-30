use crate::{
    app_state::{AppController, UiState},
    input::SystemInputExecutor,
    player::play_actions,
    storage,
};
use chrono::Utc;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, State};

pub type SharedApp = Arc<Mutex<AppController>>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn emit_state(app: &AppHandle, state: UiState) -> Result<(), String> {
    app.emit("remember://state", state)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_state(state: State<'_, SharedApp>) -> Result<UiState, String> {
    let controller = state
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    Ok(controller.ui_state())
}

#[tauri::command]
pub fn start_recording(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    let started_at_ms = now_ms();
    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.start_recording(
            format!("recording-{started_at_ms}"),
            started_at_ms,
            Utc::now().to_rfc3339(),
        )?;
        controller.ui_state()
    };
    emit_state(&app, ui_state.clone())?;
    Ok(ui_state)
}

#[tauri::command]
pub fn stop_recording(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.stop_recording(now_ms())?;
        controller.ui_state()
    };
    emit_state(&app, ui_state.clone())?;
    Ok(ui_state)
}

#[tauri::command]
pub fn open_recording(
    app: AppHandle,
    state: State<'_, SharedApp>,
    path: PathBuf,
) -> Result<UiState, String> {
    let recording = storage::load_recording(&path).map_err(|error| error.to_string())?;
    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.set_recording(recording)?;
        controller.ui_state()
    };
    emit_state(&app, ui_state.clone())?;
    Ok(ui_state)
}

#[tauri::command]
pub fn save_current_recording(state: State<'_, SharedApp>, path: PathBuf) -> Result<(), String> {
    let recording = {
        let controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.saveable_recording()?
    };
    storage::save_recording(&path, &recording).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn start_playback(
    app: AppHandle,
    state: State<'_, SharedApp>,
    loop_count: u32,
    speed_multiplier: f64,
) -> Result<UiState, String> {
    let (actions, stop_token, ui_state) = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        let actions = controller.start_playback(loop_count, speed_multiplier)?;
        let stop_token = controller.stop_token();
        (actions, stop_token, controller.ui_state())
    };
    emit_state(&app, ui_state.clone())?;

    let app_for_thread = app.clone();
    let state_for_thread = state.inner().clone();
    thread::spawn(move || {
        let executor = SystemInputExecutor;
        let result = play_actions(&actions, &executor, &stop_token);
        let next_state = {
            match state_for_thread.lock() {
                Ok(mut controller) => {
                    let message = match result {
                        Ok(()) => "Playback finished".to_string(),
                        Err(error) => error,
                    };
                    controller.mark_idle(message);
                    Some(controller.ui_state())
                }
                Err(_) => None,
            }
        };
        if let Some(next_state) = next_state {
            let _ = emit_state(&app_for_thread, next_state);
        }
    });

    Ok(ui_state)
}

#[tauri::command]
pub fn stop_playback(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.stop_playback();
        controller.ui_state()
    };
    emit_state(&app, ui_state.clone())?;
    Ok(ui_state)
}
