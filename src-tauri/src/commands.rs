use crate::{
    app_state::{AppController, AppMode, ControlHotkeyAction, PlaybackRun, UiState},
    clock::now_ms,
    hotkeys::{self, HotkeyConfig},
    input::SystemInputExecutor,
    player::play_recording,
    storage::{self, RecordingFile},
};
use chrono::Utc;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};

pub type SharedApp = Arc<Mutex<AppController>>;
const RECORDINGS_CHANGED_EVENT: &str = "remember://recordings-changed";

fn emit_state(app: &AppHandle, state: UiState) -> Result<(), String> {
    app.emit("remember://state", state)
        .map_err(|error| error.to_string())
}

fn emit_recordings_changed(app: &AppHandle) -> Result<(), String> {
    app.emit(RECORDINGS_CHANGED_EVENT, ())
        .map_err(|error| error.to_string())
}

fn recording_library_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = recording_library_dir_for_executable(&executable)?;
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory)
}

fn recording_library_dir_for_executable(executable: &Path) -> Result<PathBuf, String> {
    executable
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.join("recordings"))
        .ok_or_else(|| "cannot determine executable directory".to_string())
}

fn mark_recording_saved(
    state: &SharedApp,
    recording: &crate::model::Recording,
) -> Result<(), String> {
    let mut controller = state
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    controller.mark_recording_saved(recording);
    Ok(())
}

fn save_recording_to_library_shared(
    app: &AppHandle,
    state: &SharedApp,
    recording: &crate::model::Recording,
) -> Result<(), String> {
    let library_dir = recording_library_dir(app)?;
    storage::save_recording_to_library(&library_dir, recording)
        .map_err(|error| error.to_string())?;
    mark_recording_saved(state, recording)?;
    emit_recordings_changed(app)
}

fn save_pending_recording_shared(app: &AppHandle, state: &SharedApp) -> Result<(), String> {
    let recording = {
        let controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.recording_pending_save().cloned()
    };
    match recording {
        Some(recording) => save_recording_to_library_shared(app, state, &recording),
        None => Ok(()),
    }
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
    start_recording_impl(app, state.inner().clone(), false)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_recording_from_hotkey(
    app: AppHandle,
    state: State<'_, SharedApp>,
) -> Result<UiState, String> {
    start_recording_from_hotkey_shared(app, state.inner().clone())
}

pub(crate) fn start_recording_from_hotkey_shared(
    app: AppHandle,
    state: SharedApp,
) -> Result<UiState, String> {
    start_recording_impl(app, state, true)
}

fn start_recording_impl(
    app: AppHandle,
    state: SharedApp,
    from_hotkey: bool,
) -> Result<UiState, String> {
    let started_at_ms = now_ms();
    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        let name = format!("recording-{started_at_ms}");
        let created_at = Utc::now().to_rfc3339();
        if from_hotkey {
            controller.start_recording_from_hotkey(name, started_at_ms, created_at)?;
        } else {
            controller.start_recording(name, started_at_ms, created_at)?;
        }
        controller.ui_state()
    };
    emit_state(&app, ui_state.clone())?;
    Ok(ui_state)
}

#[tauri::command]
pub fn stop_recording(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    stop_recording_shared(app, state.inner().clone())
}

pub(crate) fn stop_recording_shared(app: AppHandle, state: SharedApp) -> Result<UiState, String> {
    let (recording, ui_state) = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        let recording = controller.stop_recording(now_ms())?;
        (recording, controller.ui_state())
    };
    emit_state(&app, ui_state.clone())?;
    save_recording_to_library_shared(&app, &state, &recording)?;
    Ok(ui_state)
}

#[tauri::command]
pub fn list_recordings(app: AppHandle) -> Result<Vec<RecordingFile>, String> {
    let library_dir = recording_library_dir(&app)?;
    storage::list_recordings(&library_dir).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_recording(app: AppHandle, path: PathBuf) -> Result<(), String> {
    let library_dir = recording_library_dir(&app)?;
    storage::delete_recording_from_library(&library_dir, &path)
        .map_err(|error| error.to_string())?;
    emit_recordings_changed(&app)
}

#[tauri::command]
pub fn rename_recording(app: AppHandle, path: PathBuf, new_name: String) -> Result<String, String> {
    let library_dir = recording_library_dir(&app)?;
    let renamed_path = storage::rename_recording_in_library(&library_dir, &path, &new_name)
        .map_err(|error| error.to_string())?;
    emit_recordings_changed(&app)?;
    Ok(renamed_path.to_string_lossy().to_string())
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
    storage::save_recording(&path, &recording).map_err(|error| error.to_string())?;
    mark_recording_saved(state.inner(), &recording)
}

#[tauri::command]
pub fn get_hotkeys(app: AppHandle) -> Result<HotkeyConfig, String> {
    hotkeys::load_config(&app)
}

#[tauri::command]
pub fn set_hotkeys(
    app: AppHandle,
    state: State<'_, SharedApp>,
    config: HotkeyConfig,
) -> Result<HotkeyConfig, String> {
    let previous = hotkeys::load_config(&app)?;
    let normalized = hotkeys::normalize_config(&config)?;

    let _ = hotkeys::unregister_all(&app);
    if let Err(error) = hotkeys::register(&app, &normalized, false) {
        let _ = hotkeys::unregister_all(&app);
        let _ = hotkeys::register(&app, &previous, true);
        return Err(error);
    }

    if let Err(error) = hotkeys::save_config(&app, &normalized) {
        let _ = hotkeys::unregister_all(&app);
        let _ = hotkeys::register(&app, &previous, true);
        return Err(error);
    }
    {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        let hotkeys = normalized
            .control_hotkeys()
            .map_err(|error| error.to_string())?;
        let record_hotkey = normalized
            .record_hotkey()
            .map_err(|error| error.to_string())?;
        let playback_hotkey = normalized
            .playback_hotkey()
            .map_err(|error| error.to_string())?;
        let stop_hotkey = normalized
            .stop_hotkey()
            .map_err(|error| error.to_string())?;
        controller.set_control_hotkeys(hotkeys, record_hotkey, playback_hotkey, stop_hotkey);
    }
    Ok(normalized)
}

#[tauri::command]
pub fn start_playback(
    app: AppHandle,
    state: State<'_, SharedApp>,
    loop_count: Option<u32>,
    speed_multiplier: f64,
) -> Result<UiState, String> {
    start_playback_shared(app, state.inner().clone(), loop_count, speed_multiplier)
}

#[tauri::command]
pub fn set_playback_settings(
    state: State<'_, SharedApp>,
    loop_count: Option<u32>,
    speed_multiplier: f64,
) -> Result<(), String> {
    let mut controller = state
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    controller.set_playback_settings(loop_count, speed_multiplier)
}

pub(crate) fn start_playback_shared(
    app: AppHandle,
    state: SharedApp,
    loop_count: Option<u32>,
    speed_multiplier: f64,
) -> Result<UiState, String> {
    start_playback_impl(app, state, |controller| {
        controller.start_playback(loop_count, speed_multiplier)
    })
}

pub(crate) fn start_playback_current_shared(
    app: AppHandle,
    state: SharedApp,
) -> Result<UiState, String> {
    start_playback_impl(app, state, |controller| {
        controller.start_playback_with_current_settings()
    })
}

fn start_playback_impl<F>(app: AppHandle, state: SharedApp, start: F) -> Result<UiState, String>
where
    F: FnOnce(&mut AppController) -> Result<PlaybackRun, String>,
{
    let (run, stop_token, ui_state) = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        let run = start(&mut controller)?;
        let stop_token = controller.stop_token();
        (run, stop_token, controller.ui_state())
    };
    emit_state(&app, ui_state.clone())?;

    let app_for_thread = app.clone();
    let state_for_thread = state.clone();
    thread::spawn(move || {
        let executor = SystemInputExecutor;
        let result = play_recording(&run.recording, run.settings, &executor, &stop_token);
        let next_state = {
            match state_for_thread.lock() {
                Ok(mut controller) => {
                    let (message, message_is_error) = match result {
                        Ok(()) => ("Playback finished".to_string(), false),
                        Err(error) if error == "playback stopped" => {
                            ("Playback stopped".to_string(), false)
                        }
                        Err(error) => (error, true),
                    };
                    if controller.finish_playback_if_current(run.id, message, message_is_error) {
                        Some(controller.ui_state())
                    } else {
                        None
                    }
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

#[cfg(not(target_os = "windows"))]
pub(crate) fn stop_active(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    stop_active_shared(app, state.inner().clone())
}

pub(crate) fn stop_active_shared(app: AppHandle, state: SharedApp) -> Result<UiState, String> {
    let mode = {
        let controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.mode()
    };
    if mode == AppMode::Recording {
        return stop_recording_shared(app, state);
    }

    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.stop_active(now_ms())?;
        controller.ui_state()
    };
    emit_state(&app, ui_state.clone())?;
    Ok(ui_state)
}

pub(crate) fn prepare_for_exit(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<SharedApp>() else {
        return Ok(());
    };
    let state = state.inner().clone();
    let mode = state
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?
        .mode();

    match mode {
        AppMode::Recording => {
            stop_recording_shared(app.clone(), state.clone())?;
        }
        AppMode::Playing => {
            let ui_state = {
                let mut controller = state
                    .lock()
                    .map_err(|_| "state lock poisoned".to_string())?;
                controller.stop_playback();
                controller.ui_state()
            };
            emit_state(app, ui_state)?;

            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                let mode = state
                    .lock()
                    .map_err(|_| "state lock poisoned".to_string())?
                    .mode();
                if mode != AppMode::Playing {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("playback cleanup did not finish before exit".to_string());
                }
                thread::sleep(Duration::from_millis(5));
            }
        }
        AppMode::Idle => {}
    }

    save_pending_recording_shared(app, &state)
}

pub(crate) fn run_control_hotkey_action(
    app: AppHandle,
    state: SharedApp,
    action: ControlHotkeyAction,
) {
    let result = match action {
        ControlHotkeyAction::Record => start_recording_from_hotkey_shared(app, state),
        ControlHotkeyAction::Playback => start_playback_current_shared(app, state),
        ControlHotkeyAction::Stop => stop_active_shared(app, state),
    };
    if let Err(error) = result {
        eprintln!("Remember control hotkey failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::recording_library_dir_for_executable;
    use std::path::{Path, PathBuf};

    #[test]
    fn recording_library_is_next_to_the_executable() {
        let executable = Path::new("portable").join("remember.exe");

        let directory =
            recording_library_dir_for_executable(&executable).expect("recording directory");

        assert_eq!(directory, PathBuf::from("portable").join("recordings"));
    }
}
