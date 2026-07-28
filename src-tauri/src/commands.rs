use crate::{
    advanced_settings::{self, AdvancedSettings},
    app_state::{AppController, AppMode, ControlHotkeyAction, PlaybackRun, UiState},
    clock::now_ms,
    hotkeys::{self, HotkeyConfig},
    input::SystemInputExecutor,
    player::play_recording,
    privileges::{self, PrivilegeState},
    settings_bundle::{self, SettingsBundle},
    storage::{self, RecordingFile},
};
use chrono::{DateTime, Local};
use std::{
    fs,
    path::{Path, PathBuf},
    process,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};

pub type SharedApp = Arc<Mutex<AppController>>;
const RECORDINGS_CHANGED_EVENT: &str = "remember://recordings-changed";
const HOTKEYS_CHANGED_EVENT: &str = "remember://hotkeys-changed";
const ADVANCED_SETTINGS_CHANGED_EVENT: &str = "remember://advanced-settings-changed";
const ADMIN_RESTART_RECOVERY_DIR: &str = "administrator-restart-recovery";
static RECORDING_SAVE_LOCK: Mutex<()> = Mutex::new(());
static SETTINGS_TRANSACTION_LOCK: Mutex<()> = Mutex::new(());
static ADMIN_RESTART_RECOVERY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn emit_state(app: &AppHandle, state: UiState) -> Result<(), String> {
    let indicator_enabled = match advanced_settings::current(app) {
        Ok(settings) => settings.show_activity_indicator,
        Err(error) => {
            eprintln!("Remember advanced settings could not be read: {error}");
            true
        }
    };
    if state.mode == AppMode::Idle {
        let emit_result = app
            .emit("remember://state", state.clone())
            .map_err(|error| error.to_string());
        if let Err(error) = crate::activity_indicator::sync(app, state.mode, indicator_enabled) {
            eprintln!("Remember activity indicator could not update: {error}");
        }
        return emit_result;
    }

    if let Err(error) = crate::activity_indicator::sync(app, state.mode, indicator_enabled) {
        eprintln!("Remember activity indicator could not update: {error}");
    }
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
    recording: &Arc<crate::model::Recording>,
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
    recording: &Arc<crate::model::Recording>,
) -> Result<(), String> {
    let library_dir = recording_library_dir(app)?;
    storage::save_recording_to_library(&library_dir, recording)
        .map_err(|error| error.to_string())?;
    mark_recording_saved(state, recording)?;
    if let Err(error) = emit_recordings_changed(app) {
        eprintln!("Remember recordings-changed event failed after saving: {error}");
    }
    Ok(())
}

fn save_pending_recording_shared(app: &AppHandle, state: &SharedApp) -> Result<(), String> {
    let _save_guard = RECORDING_SAVE_LOCK
        .lock()
        .map_err(|_| "recording save lock poisoned".to_string())?;
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
    start_recording_impl(app, state.inner().clone(), false, None, false)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_recording_from_hotkey(
    app: AppHandle,
    state: State<'_, SharedApp>,
) -> Result<UiState, String> {
    start_recording_from_hotkey_shared(app, state.inner().clone())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_recording_from_hotkey_shared(
    app: AppHandle,
    state: SharedApp,
) -> Result<UiState, String> {
    start_recording_impl(app, state, true, None, false)
}

fn start_recording_impl(
    app: AppHandle,
    state: SharedApp,
    from_hotkey: bool,
    started_at_ms: Option<u64>,
    capture_boundary_is_ordered: bool,
) -> Result<UiState, String> {
    let capture_pause = if capture_boundary_is_ordered {
        None
    } else {
        Some(crate::input::pause_capture_events()?)
    };
    let started_at_ms = started_at_ms.unwrap_or_else(now_ms);
    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        let name = format!("recording-{started_at_ms}");
        let created_at = Local::now().to_rfc3339();
        if from_hotkey {
            controller.start_recording_from_hotkey(name, started_at_ms, created_at)?;
        } else {
            controller.start_recording(name, started_at_ms, created_at)?;
        }
        controller.ui_state()
    };
    drop(capture_pause);
    emit_state(&app, ui_state.clone())?;
    Ok(ui_state)
}

#[tauri::command]
pub fn stop_recording(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    stop_recording_shared(app, state.inner().clone())
}

pub(crate) fn stop_recording_shared(app: AppHandle, state: SharedApp) -> Result<UiState, String> {
    stop_recording_impl(app, state, now_ms(), false, true)
}

fn stop_recording_impl(
    app: AppHandle,
    state: SharedApp,
    stopped_at_ms: u64,
    capture_boundary_is_ordered: bool,
    save_to_library: bool,
) -> Result<UiState, String> {
    let capture_pause = if capture_boundary_is_ordered {
        None
    } else {
        Some(crate::input::pause_capture_events()?)
    };
    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        let recording = controller.stop_recording(stopped_at_ms)?;
        let name = default_recording_name(&recording);
        drop(recording);
        controller.set_stopped_recording_name(name)?;
        controller.ui_state()
    };
    drop(capture_pause);
    if !save_to_library {
        emit_state(&app, ui_state.clone())?;
        return Ok(ui_state);
    }
    match save_pending_recording_shared(&app, &state) {
        Ok(()) => {
            emit_state(&app, ui_state.clone())?;
            Ok(ui_state)
        }
        Err(error) => {
            let error = format!("Recording stopped but could not be saved: {error}");
            let error_state = {
                let mut controller = state
                    .lock()
                    .map_err(|_| "state lock poisoned".to_string())?;
                controller.set_error(error.clone());
                controller.ui_state()
            };
            if let Err(emit_error) = emit_state(&app, error_state) {
                return Err(format!("{error}; state update failed: {emit_error}"));
            }
            Err(error)
        }
    }
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
    settings_bundle::load(&app).map(|bundle| bundle.hotkeys)
}

#[tauri::command]
pub fn get_advanced_settings(app: AppHandle) -> Result<AdvancedSettings, String> {
    advanced_settings::current(&app)
}

#[tauri::command]
pub fn get_settings_bundle(app: AppHandle) -> Result<SettingsBundle, String> {
    settings_bundle::load(&app)
}

#[tauri::command]
pub fn set_settings_bundle(
    app: AppHandle,
    state: State<'_, SharedApp>,
    bundle: SettingsBundle,
) -> Result<SettingsBundle, String> {
    let bundle = settings_bundle::normalize(bundle)?;
    let _transaction_guard = SETTINGS_TRANSACTION_LOCK
        .lock()
        .map_err(|_| "settings transaction lock poisoned".to_string())?;
    let previous = settings_bundle::load(&app)?;
    let control_hotkeys = bundle.hotkeys.control_hotkeys()?;
    let record_hotkey = bundle.hotkeys.record_hotkey()?;
    let playback_hotkey = bundle.hotkeys.playback_hotkey()?;
    let stop_hotkey = bundle.hotkeys.stop_hotkey()?;
    let advanced_state = app
        .try_state::<advanced_settings::SharedAdvancedSettings>()
        .ok_or_else(|| "advanced settings state is unavailable".to_string())?;
    let mut current_advanced = advanced_state
        .lock()
        .map_err(|_| "advanced settings lock poisoned".to_string())?;
    let mut controller = state
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;

    if let Err(error) = hotkeys::unregister_all(&app) {
        let restore_error = restore_hotkey_registration(&app, &previous.hotkeys).err();
        return Err(combine_transaction_error(error, restore_error));
    }
    if let Err(error) = hotkeys::register(&app, &bundle.hotkeys, false) {
        let restore_error = restore_hotkey_registration(&app, &previous.hotkeys).err();
        return Err(combine_transaction_error(error, restore_error));
    }
    if let Err(error) = settings_bundle::save(&app, bundle.clone()) {
        let restore_error = restore_hotkey_registration(&app, &previous.hotkeys).err();
        return Err(combine_transaction_error(error, restore_error));
    }

    *current_advanced = bundle.advanced;
    controller.set_control_hotkeys(control_hotkeys, record_hotkey, playback_hotkey, stop_hotkey);
    let mode = controller.mode();
    drop(controller);
    drop(current_advanced);

    if let Err(error) =
        crate::activity_indicator::sync(&app, mode, bundle.advanced.show_activity_indicator)
    {
        eprintln!("Remember activity indicator could not update after settings save: {error}");
    }
    if let Err(error) = app.emit(HOTKEYS_CHANGED_EVENT, bundle.hotkeys.clone()) {
        eprintln!("Remember hotkey change event failed: {error}");
    }
    if let Err(error) = app.emit(ADVANCED_SETTINGS_CHANGED_EVENT, bundle.advanced) {
        eprintln!("Remember advanced settings change event failed: {error}");
    }
    Ok(bundle)
}

fn restore_hotkey_registration(app: &AppHandle, config: &HotkeyConfig) -> Result<(), String> {
    let unregister_error = hotkeys::unregister_all(app).err();
    let register_error = hotkeys::register(app, config, true).err();
    match (unregister_error, register_error) {
        (None, None) => Ok(()),
        (Some(unregister_error), None) => Err(format!(
            "could not clear partially registered hotkeys: {unregister_error}"
        )),
        (None, Some(register_error)) => Err(register_error),
        (Some(unregister_error), Some(register_error)) => Err(format!(
            "could not clear partially registered hotkeys: {unregister_error}; previous hotkeys could not be registered: {register_error}"
        )),
    }
}

fn combine_transaction_error(error: String, rollback_error: Option<String>) -> String {
    match rollback_error {
        Some(rollback_error) => {
            format!("{error}; previous hotkeys could not be restored: {rollback_error}")
        }
        None => error,
    }
}

#[tauri::command]
pub fn show_advanced_settings(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("advanced-settings")
        .ok_or_else(|| "advanced settings window is unavailable".to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_privilege_state() -> Result<PrivilegeState, String> {
    privileges::state()
}

#[tauri::command]
pub fn restart_as_administrator(app: AppHandle) -> Result<(), String> {
    if privileges::state()?.is_elevated {
        return Err("already running as administrator".to_string());
    }
    let recovery_path = prepare_for_administrator_restart(&app)?;
    if let Err(error) = privileges::restart_as_administrator(&app) {
        if let Some(path) = recovery_path {
            if let Err(cleanup_error) = fs::remove_file(&path) {
                if cleanup_error.kind() != std::io::ErrorKind::NotFound {
                    return Err(format!(
                        "{error}; administrator-restart recovery cleanup failed: {cleanup_error}"
                    ));
                }
            }
        }
        return Err(error);
    }
    app.exit(0);
    Ok(())
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

#[cfg(not(target_os = "windows"))]
pub(crate) fn stop_active_shared(app: AppHandle, state: SharedApp) -> Result<UiState, String> {
    stop_active_impl(app, state, now_ms(), false)
}

fn stop_active_impl(
    app: AppHandle,
    state: SharedApp,
    stopped_at_ms: u64,
    capture_boundary_is_ordered: bool,
) -> Result<UiState, String> {
    let mode = {
        let controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.mode()
    };
    if mode == AppMode::Recording {
        return stop_recording_impl(app, state, stopped_at_ms, capture_boundary_is_ordered, true);
    }

    let ui_state = {
        let mut controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.stop_active(stopped_at_ms)?;
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

fn prepare_for_administrator_restart(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    let Some(state) = app.try_state::<SharedApp>() else {
        return Ok(None);
    };
    let state = state.inner().clone();
    let mode = state
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?
        .mode();

    match mode {
        AppMode::Recording => {
            stop_recording_impl(app.clone(), state.clone(), now_ms(), false, false)?;
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
                    return Err(
                        "playback cleanup did not finish before administrator restart".to_string(),
                    );
                }
                thread::sleep(Duration::from_millis(5));
            }
        }
        AppMode::Idle => {}
    }

    persist_administrator_restart_recovery(app, &state)
}

fn persist_administrator_restart_recovery(
    app: &AppHandle,
    state: &SharedApp,
) -> Result<Option<PathBuf>, String> {
    let _save_guard = RECORDING_SAVE_LOCK
        .lock()
        .map_err(|_| "recording save lock poisoned".to_string())?;
    let recording = {
        let controller = state
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        controller.recording_pending_save().cloned()
    };
    let Some(recording) = recording else {
        return Ok(None);
    };

    let recovery_dir = administrator_restart_recovery_dir(app)?;
    fs::create_dir_all(&recovery_dir).map_err(|error| error.to_string())?;
    let path = loop {
        let sequence = ADMIN_RESTART_RECOVERY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = recovery_dir.join(format!(
            "administrator-restart-{}-{}-{sequence}.remember.json",
            process::id(),
            now_ms()
        ));
        if !candidate.exists() {
            break candidate;
        }
    };
    storage::save_recording(&path, &recording).map_err(|error| error.to_string())?;
    Ok(Some(path))
}

pub(crate) fn restore_administrator_restart_recovery(app: &AppHandle) -> Result<(), String> {
    let recovery_dir = administrator_restart_recovery_dir(app)?;
    if storage::list_recordings(&recovery_dir)
        .map_err(|error| error.to_string())?
        .is_empty()
    {
        return Ok(());
    }
    let library_dir = recording_library_dir(app)?;
    restore_administrator_restart_recovery_dirs(&recovery_dir, &library_dir)
}

fn restore_administrator_restart_recovery_dirs(
    recovery_dir: &Path,
    library_dir: &Path,
) -> Result<(), String> {
    let recoveries = storage::list_recordings(recovery_dir).map_err(|error| error.to_string())?;
    let mut errors = Vec::new();
    for recovery in recoveries {
        if let Some(error) = recovery.load_error {
            errors.push(format!(
                "administrator-restart recovery {} is invalid: {error}",
                recovery.path
            ));
            continue;
        }
        let path = PathBuf::from(recovery.path);
        let recording = match storage::load_recording(&path) {
            Ok(recording) => recording,
            Err(error) => {
                errors.push(format!(
                    "administrator-restart recovery {} could not be read: {error}",
                    path.display()
                ));
                continue;
            }
        };
        let Some(file_name) = path.file_name() else {
            errors.push(format!(
                "administrator-restart recovery path is invalid: {}",
                path.display()
            ));
            continue;
        };
        let destination = library_dir.join(file_name);
        if destination.exists() {
            match storage::load_recording(&destination) {
                Ok(installed) if installed == recording => {
                    if let Err(error) = fs::remove_file(&path) {
                        errors.push(format!(
                            "administrator-restart recovery {} was already restored but could not be cleaned up: {error}",
                            path.display()
                        ));
                    }
                }
                Ok(_) => errors.push(format!(
                    "administrator-restart recovery {} conflicts with an existing library file",
                    path.display()
                )),
                Err(error) => errors.push(format!(
                    "administrator-restart recovery {} conflicts with an unreadable library file: {error}",
                    path.display()
                )),
            }
            continue;
        }

        if let Err(error) = storage::save_recording_without_overwrite(&destination, &recording) {
            errors.push(format!(
                "administrator-restart recovery {} could not be restored: {error}",
                path.display()
            ));
            continue;
        }
        if let Err(error) = fs::remove_file(&path) {
            errors.push(format!(
                "administrator-restart recovery {} was restored but could not be cleaned up: {error}",
                path.display()
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn administrator_restart_recovery_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join(ADMIN_RESTART_RECOVERY_DIR))
        .map_err(|error| error.to_string())
}

pub(crate) fn run_control_hotkey_action(
    app: AppHandle,
    state: SharedApp,
    action: ControlHotkeyAction,
    action_at_ms: u64,
) {
    let result = match action {
        ControlHotkeyAction::Record => {
            start_recording_impl(app.clone(), state.clone(), true, Some(action_at_ms), true)
        }
        ControlHotkeyAction::Playback => start_playback_current_shared(app.clone(), state.clone()),
        ControlHotkeyAction::Stop => {
            stop_active_impl(app.clone(), state.clone(), action_at_ms, true)
        }
    };
    if let Err(error) = result {
        eprintln!("Remember control hotkey failed: {error}");
        let error_state = match state.lock() {
            Ok(mut controller) => controller.set_error(error).then(|| controller.ui_state()),
            Err(_) => None,
        };
        if let Some(error_state) = error_state {
            if let Err(emit_error) = emit_state(&app, error_state) {
                eprintln!("Remember control hotkey error state failed: {emit_error}");
            }
        }
    }
}

fn default_recording_name(recording: &crate::model::Recording) -> String {
    let date = DateTime::parse_from_rfc3339(&recording.created_at)
        .map(|created_at| created_at.format("%Y%m%d").to_string())
        .unwrap_or_else(|_| Local::now().format("%Y%m%d").to_string());
    format!("{date}-{}ms", recording.duration_ms)
}

#[cfg(test)]
mod tests {
    use super::{
        default_recording_name, recording_library_dir_for_executable,
        restore_administrator_restart_recovery_dirs,
    };
    use crate::model::Recording;
    use crate::storage::{list_recordings, save_recording, save_recording_to_library};
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn recording_library_is_next_to_the_executable() {
        let executable = Path::new("portable").join("remember.exe");

        let directory =
            recording_library_dir_for_executable(&executable).expect("recording directory");

        assert_eq!(directory, PathBuf::from("portable").join("recordings"));
    }

    #[test]
    fn default_recording_name_uses_start_date_and_duration() {
        let recording = Recording {
            version: 1,
            name: "temporary".to_string(),
            created_at: "2026-07-26T23:59:00+08:00".to_string(),
            duration_ms: 11_824,
            steps: Vec::new(),
        };

        assert_eq!(default_recording_name(&recording), "20260726-11824ms");
    }

    #[test]
    fn administrator_restart_restores_every_unique_recovery() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "remember-admin-recovery-{}-{unique}",
            process::id()
        ));
        let recovery_dir = root.join("recovery");
        let library_dir = root.join("library");
        let recording = Recording {
            version: 1,
            name: "handoff".to_string(),
            created_at: "2026-07-26T23:59:00+08:00".to_string(),
            duration_ms: 10,
            steps: Vec::new(),
        };
        let first_recovery =
            save_recording_to_library(&recovery_dir, &recording).expect("first recovery");
        save_recording_to_library(&recovery_dir, &recording).expect("second recovery");

        restore_administrator_restart_recovery_dirs(&recovery_dir, &library_dir)
            .expect("restore recoveries");
        save_recording(&first_recovery, &recording).expect("recreate recovery after crash");
        restore_administrator_restart_recovery_dirs(&recovery_dir, &library_dir)
            .expect("idempotent restore");

        assert!(list_recordings(&recovery_dir)
            .expect("list recoveries")
            .is_empty());
        let restored = list_recordings(&library_dir).expect("list restored recordings");
        assert_eq!(restored.len(), 2);

        for recording in restored {
            fs::remove_file(recording.path).expect("clean up restored recording");
        }
        fs::remove_dir(&library_dir).expect("clean up library");
        fs::remove_dir(&recovery_dir).expect("clean up recovery");
        fs::remove_dir(root).expect("clean up root");
    }

    #[test]
    fn invalid_administrator_recovery_does_not_block_valid_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "remember-admin-recovery-invalid-{}-{unique}",
            process::id()
        ));
        let recovery_dir = root.join("recovery");
        let library_dir = root.join("library");
        let recording = Recording {
            version: 1,
            name: "valid-handoff".to_string(),
            created_at: "2026-07-26T23:59:00+08:00".to_string(),
            duration_ms: 10,
            steps: Vec::new(),
        };
        let valid_path =
            save_recording_to_library(&recovery_dir, &recording).expect("valid recovery");
        let invalid_path = recovery_dir.join("invalid.remember.json");
        fs::write(&invalid_path, "{not valid json").expect("invalid recovery");

        let error = restore_administrator_restart_recovery_dirs(&recovery_dir, &library_dir)
            .expect_err("invalid recovery warning");

        assert!(error.contains("invalid"));
        assert!(!valid_path.exists());
        assert!(invalid_path.exists());
        let restored = list_recordings(&library_dir).expect("list restored recordings");
        assert_eq!(restored.len(), 1);

        fs::remove_file(&invalid_path).expect("clean up invalid recovery");
        fs::remove_file(&restored[0].path).expect("clean up restored recording");
        fs::remove_dir(&library_dir).expect("clean up library");
        fs::remove_dir(&recovery_dir).expect("clean up recovery");
        fs::remove_dir(root).expect("clean up root");
    }
}
