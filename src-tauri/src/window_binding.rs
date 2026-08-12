use crate::{
    model::{ClientSize, WindowTarget},
    player::StopToken,
    window_target::{self, WindowHandle, WindowSnapshot},
};
use serde::Serialize;
use std::{
    sync::{mpsc, Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use tauri::{
    webview::PageLoadEvent, AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State,
    WebviewWindow, WebviewWindowBuilder,
};

const BINDING_WINDOW_LABEL: &str = "window-binding";
const HIGHLIGHT_WINDOW_LABEL: &str = "window-highlight";
const BINDING_EVENT: &str = "remember://window-binding";
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const UI_TRANSITION_TIMEOUT: Duration = Duration::from_secs(2);

pub type SharedWindowBinding = Arc<WindowBindingCoordinator>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowBindingTargetView {
    pub executable_path: String,
    pub window_class: String,
    pub title: String,
    pub client_size: ClientSize,
    pub dpi: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowBindingCandidateView {
    pub candidate_id: u32,
    pub executable_path: String,
    pub window_class: String,
    pub title: String,
    pub client_size: ClientSize,
    pub dpi: u32,
    pub process_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowBindingRequest {
    pub request_id: u64,
    pub target: WindowBindingTargetView,
    pub candidates: Vec<WindowBindingCandidateView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManualBindingResult {
    pub handle: WindowHandle,
    pub pause_ms: u64,
}

#[derive(Default)]
pub struct WindowBindingCoordinator {
    state: Mutex<CoordinatorState>,
    changed: Condvar,
}

#[derive(Default)]
struct CoordinatorState {
    next_request_id: u64,
    pending: Option<PendingBinding>,
}

struct PendingBinding {
    request: WindowBindingRequest,
    handles: Vec<WindowHandle>,
    resolution: Option<Result<WindowHandle, String>>,
}

impl WindowBindingCoordinator {
    fn begin(
        &self,
        target: &WindowTarget,
        candidates: &[WindowSnapshot],
    ) -> Result<WindowBindingRequest, String> {
        if candidates.is_empty() {
            return Err("没有可供手动选择的窗口。".to_string());
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| "window binding state lock poisoned".to_string())?;
        if state.pending.is_some() {
            return Err("another window binding request is already active".to_string());
        }
        state.next_request_id = state.next_request_id.wrapping_add(1).max(1);
        let request_id = state.next_request_id;
        let request = WindowBindingRequest {
            request_id,
            target: target_view(target),
            candidates: candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| candidate_view(index, candidate))
                .collect::<Result<_, _>>()?,
        };
        state.pending = Some(PendingBinding {
            request: request.clone(),
            handles: candidates
                .iter()
                .map(|candidate| candidate.handle)
                .collect(),
            resolution: None,
        });
        Ok(request)
    }

    fn pending_request(&self) -> Result<Option<WindowBindingRequest>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "window binding state lock poisoned".to_string())?;
        Ok(state
            .pending
            .as_ref()
            .map(|pending| pending.request.clone()))
    }

    fn candidate_handle(&self, request_id: u64, candidate_id: u32) -> Result<WindowHandle, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "window binding state lock poisoned".to_string())?;
        let pending = matching_pending(&state, request_id)?;
        pending
            .handles
            .get(candidate_id as usize)
            .copied()
            .ok_or_else(|| "window binding candidate is unavailable".to_string())
    }

    fn ensure_request_active(&self, request_id: u64) -> Result<(), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "window binding state lock poisoned".to_string())?;
        matching_pending(&state, request_id).map(|_| ())
    }

    fn resolve(&self, request_id: u64, candidate_id: u32) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "window binding state lock poisoned".to_string())?;
        let pending = matching_pending_mut(&mut state, request_id)?;
        let handle = pending
            .handles
            .get(candidate_id as usize)
            .copied()
            .ok_or_else(|| "window binding candidate is unavailable".to_string())?;
        pending.resolution = Some(Ok(handle));
        self.changed.notify_all();
        Ok(())
    }

    fn cancel(&self, request_id: u64) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "window binding state lock poisoned".to_string())?;
        let pending = matching_pending_mut(&mut state, request_id)?;
        pending.resolution = Some(Err("playback stopped".to_string()));
        self.changed.notify_all();
        Ok(())
    }

    fn fail(&self, request_id: u64, error: String) {
        if let Ok(mut state) = self.state.lock() {
            if let Ok(pending) = matching_pending_mut(&mut state, request_id) {
                pending.resolution = Some(Err(error));
                self.changed.notify_all();
            }
        }
    }

    fn wait(&self, request_id: u64, stop_token: &StopToken) -> Result<WindowHandle, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "window binding state lock poisoned".to_string())?;
        loop {
            if stop_token.is_stopped() {
                clear_matching_pending(&mut state, request_id);
                return Err("playback stopped".to_string());
            }

            let resolution = matching_pending_mut(&mut state, request_id)?
                .resolution
                .take();
            if let Some(resolution) = resolution {
                clear_matching_pending(&mut state, request_id);
                return resolution;
            }

            let (next_state, _) = self
                .changed
                .wait_timeout(state, STOP_POLL_INTERVAL)
                .map_err(|_| "window binding state lock poisoned".to_string())?;
            state = next_state;
        }
    }

    fn reset(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(pending) = state.pending.as_mut() {
                pending.resolution = Some(Err("playback stopped".to_string()));
            }
            self.changed.notify_all();
        }
    }
}

fn matching_pending(state: &CoordinatorState, request_id: u64) -> Result<&PendingBinding, String> {
    state
        .pending
        .as_ref()
        .filter(|pending| pending.request.request_id == request_id)
        .ok_or_else(|| "window binding request is no longer active".to_string())
}

fn matching_pending_mut(
    state: &mut CoordinatorState,
    request_id: u64,
) -> Result<&mut PendingBinding, String> {
    state
        .pending
        .as_mut()
        .filter(|pending| pending.request.request_id == request_id)
        .ok_or_else(|| "window binding request is no longer active".to_string())
}

fn clear_matching_pending(state: &mut CoordinatorState, request_id: u64) {
    if state
        .pending
        .as_ref()
        .is_some_and(|pending| pending.request.request_id == request_id)
    {
        state.pending = None;
    }
}

fn target_view(target: &WindowTarget) -> WindowBindingTargetView {
    WindowBindingTargetView {
        executable_path: target.executable_path.clone(),
        window_class: target.window_class.clone(),
        title: target.title.clone(),
        client_size: target.client_size,
        dpi: target.dpi,
    }
}

fn candidate_view(
    index: usize,
    candidate: &WindowSnapshot,
) -> Result<WindowBindingCandidateView, String> {
    Ok(WindowBindingCandidateView {
        candidate_id: index
            .try_into()
            .map_err(|_| "too many window binding candidates".to_string())?,
        executable_path: candidate.executable_path.clone(),
        window_class: candidate.window_class.clone(),
        title: candidate.title.clone(),
        client_size: candidate.client_size,
        dpi: candidate.dpi,
        process_id: candidate.process_id,
    })
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let coordinator = app
        .try_state::<SharedWindowBinding>()
        .ok_or_else(|| "window binding state is unavailable".to_string())?;
    coordinator.reset();
    hide_windows(app)
}

pub fn request_manual_binding(
    app: &AppHandle,
    target: &WindowTarget,
    candidates: &[WindowSnapshot],
    stop_token: &StopToken,
) -> Result<ManualBindingResult, String> {
    let coordinator = app
        .try_state::<SharedWindowBinding>()
        .ok_or_else(|| "window binding state is unavailable".to_string())?;
    let request = coordinator.begin(target, candidates)?;
    let request_id = request.request_id;
    let coordinator_for_main = Arc::clone(&coordinator);
    let app_for_main = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if let Err(error) = show_binding_window(&app_for_main, &request) {
            coordinator_for_main.fail(request_id, error);
        }
    }) {
        coordinator.fail(request_id, error.to_string());
    }

    let started = Instant::now();
    let resolution = coordinator.wait(request_id, stop_token);
    hide_windows_before_resume(app)?;
    let handle = resolution?;
    Ok(ManualBindingResult {
        handle,
        pause_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
    })
}

#[tauri::command]
pub fn get_pending_window_binding(
    state: State<'_, SharedWindowBinding>,
) -> Result<Option<WindowBindingRequest>, String> {
    state.pending_request()
}

#[tauri::command]
pub fn select_window_binding_candidate(
    state: State<'_, SharedWindowBinding>,
    request_id: u64,
    candidate_id: u32,
) -> Result<(), String> {
    state.resolve(request_id, candidate_id)
}

#[tauri::command]
pub fn cancel_window_binding(
    state: State<'_, SharedWindowBinding>,
    request_id: u64,
) -> Result<(), String> {
    state.cancel(request_id)
}

#[tauri::command]
pub fn highlight_window_binding_candidate(
    app: AppHandle,
    state: State<'_, SharedWindowBinding>,
    request_id: u64,
    candidate_id: Option<u32>,
) -> Result<(), String> {
    let handle = match candidate_id {
        Some(candidate_id) => Some(state.candidate_handle(request_id, candidate_id)?),
        None => {
            state.ensure_request_active(request_id)?;
            None
        }
    };
    let coordinator = Arc::clone(state.inner());
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let active_handle = match candidate_id {
            Some(candidate_id) => coordinator.candidate_handle(request_id, candidate_id).ok(),
            None if coordinator.ensure_request_active(request_id).is_ok() => None,
            None => return,
        };
        if active_handle != handle {
            return;
        }
        let result = match active_handle {
            Some(handle) => show_highlight_window(&app_for_main, handle),
            None => hide_highlight_window(&app_for_main),
        };
        if let Err(error) = result {
            let _ = hide_highlight_window(&app_for_main);
            eprintln!("Remember window candidate highlight failed: {error}");
        }
    })
    .map_err(|error| error.to_string())
}

fn show_binding_window(app: &AppHandle, request: &WindowBindingRequest) -> Result<(), String> {
    let window = ensure_window(app, BINDING_WINDOW_LABEL)?;
    window
        .emit(BINDING_EVENT, request)
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

fn show_highlight_window(app: &AppHandle, handle: WindowHandle) -> Result<(), String> {
    let snapshot = window_target::snapshot_window(handle).map_err(|error| error.to_string())?;
    let window = ensure_window(app, HIGHLIGHT_WINDOW_LABEL)?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(
            snapshot.client_origin.x,
            snapshot.client_origin.y,
        ))
        .map_err(|error| error.to_string())?;
    window
        .set_size(PhysicalSize::new(
            snapshot.client_size.width,
            snapshot.client_size.height,
        ))
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())
}

fn ensure_window(app: &AppHandle, label: &str) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(label) {
        return Ok(window);
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == label)
        .cloned()
        .ok_or_else(|| format!("{label} window configuration is unavailable"))?;
    WebviewWindowBuilder::from_config(app, &config)
        .map_err(|error| error.to_string())?
        .on_page_load(|window, payload| {
            if payload.event() != PageLoadEvent::Finished || window.label() != BINDING_WINDOW_LABEL
            {
                return;
            }
            let Some(state) = window.app_handle().try_state::<SharedWindowBinding>() else {
                return;
            };
            if let Ok(Some(request)) = state.pending_request() {
                let _ = window.emit(BINDING_EVENT, request);
            }
        })
        .build()
        .map_err(|error| error.to_string())
}

fn hide_windows_before_resume(app: &AppHandle) -> Result<(), String> {
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let _ = result_tx.send(hide_windows(&app_for_main));
    })
    .map_err(|error| error.to_string())?;

    let deadline = Instant::now() + UI_TRANSITION_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("手动窗口选择界面未能及时关闭，已停止回放。".to_string());
        }
        match result_rx.recv_timeout(remaining.min(STOP_POLL_INTERVAL)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("手动窗口选择界面关闭任务意外终止。".to_string());
            }
        }
    }
}

fn hide_windows(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(BINDING_WINDOW_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }
    hide_highlight_window(app)
}

fn hide_highlight_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(HIGHLIGHT_WINDOW_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{TargetWindowAvailability, TargetWindowId};

    fn target() -> WindowTarget {
        WindowTarget {
            id: TargetWindowId(1),
            executable_path: r"C:\Apps\Editor.exe".to_string(),
            window_class: "EditorWindow".to_string(),
            title: "notes.txt".to_string(),
            client_size: ClientSize {
                width: 800,
                height: 600,
            },
            dpi: 96,
            availability: TargetWindowAvailability::Deferred,
        }
    }

    fn candidate(raw_handle: usize, title: &str) -> WindowSnapshot {
        WindowSnapshot {
            handle: WindowHandle::from_raw(raw_handle),
            executable_path: r"C:\Apps\Editor.exe".to_string(),
            window_class: "EditorWindow".to_string(),
            title: title.to_string(),
            client_origin: window_target::ScreenPoint { x: 10, y: 20 },
            client_size: ClientSize {
                width: 800,
                height: 600,
            },
            dpi: 96,
            process_id: 42,
            visible: true,
            minimized: false,
        }
    }

    #[test]
    fn exposes_candidates_without_exposing_raw_window_handles() {
        let coordinator = WindowBindingCoordinator::default();
        let request = coordinator
            .begin(
                &target(),
                &[candidate(0x1234, "first"), candidate(0x5678, "second")],
            )
            .unwrap();

        assert_eq!(request.request_id, 1);
        assert_eq!(request.candidates[0].candidate_id, 0);
        assert_eq!(request.candidates[1].candidate_id, 1);
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("4660"));
        assert!(!json.contains("22136"));
    }

    #[test]
    fn ignores_stale_request_ids_and_resolves_the_selected_candidate() {
        let coordinator = WindowBindingCoordinator::default();
        let request = coordinator
            .begin(
                &target(),
                &[candidate(0x1234, "first"), candidate(0x5678, "second")],
            )
            .unwrap();

        assert!(coordinator
            .ensure_request_active(request.request_id)
            .is_ok());
        assert!(coordinator
            .ensure_request_active(request.request_id + 1)
            .is_err());
        assert!(coordinator.resolve(request.request_id + 1, 0).is_err());
        coordinator.resolve(request.request_id, 1).unwrap();
        assert_eq!(
            coordinator
                .wait(request.request_id, &StopToken::default())
                .unwrap(),
            WindowHandle::from_raw(0x5678)
        );
        assert_eq!(coordinator.pending_request().unwrap(), None);
    }

    #[test]
    fn cancellation_uses_the_normal_playback_stopped_result() {
        let coordinator = WindowBindingCoordinator::default();
        let request = coordinator
            .begin(&target(), &[candidate(0x1234, "first")])
            .unwrap();

        coordinator.cancel(request.request_id).unwrap();
        assert_eq!(
            coordinator
                .wait(request.request_id, &StopToken::default())
                .unwrap_err(),
            "playback stopped"
        );
    }

    #[test]
    fn stop_token_interrupts_a_pending_choice() {
        let coordinator = WindowBindingCoordinator::default();
        let request = coordinator
            .begin(&target(), &[candidate(0x1234, "first")])
            .unwrap();
        let stop_token = StopToken::default();
        stop_token.request_stop();

        assert_eq!(
            coordinator
                .wait(request.request_id, &stop_token)
                .unwrap_err(),
            "playback stopped"
        );
        assert_eq!(coordinator.pending_request().unwrap(), None);
    }
}
