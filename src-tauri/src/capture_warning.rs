use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{
    webview::PageLoadEvent, AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow,
    WebviewWindowBuilder,
};

const WINDOW_LABEL: &str = "capture-warning";
const WARNING_EVENT: &str = "remember://capture-warning";
const UPDATE_INTERVAL: Duration = Duration::from_millis(20);
const CURSOR_OFFSET_X: i32 = 18;
const CURSOR_OFFSET_Y: i32 = 22;
const SCREEN_MARGIN: i32 = 8;

static STATE: Mutex<WarningState> = Mutex::new(WarningState::new());
static UPDATE_SENDER: Mutex<Option<SyncSender<()>>> = Mutex::new(None);
static CREATE_LOCK: Mutex<()> = Mutex::new(());
static MAIN_FLUSH_PENDING: AtomicBool = AtomicBool::new(false);
static PAGE_READY: AtomicBool = AtomicBool::new(false);
static CONFIGURED: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct DesiredWarning {
    visible: bool,
    message: String,
    cursor_x: i32,
    cursor_y: i32,
    revision: u64,
}

struct WarningState {
    desired: DesiredWarning,
    applied_revision: u64,
}

impl WarningState {
    const fn new() -> Self {
        Self {
            desired: DesiredWarning {
                visible: false,
                message: String::new(),
                cursor_x: 0,
                cursor_y: 0,
                revision: 0,
            },
            applied_revision: 0,
        }
    }
}

#[derive(Clone, Serialize)]
struct CaptureWarningPayload {
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesktopBounds {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    PAGE_READY.store(false, Ordering::Release);
    CONFIGURED.store(false, Ordering::Release);

    {
        let mut state = STATE
            .lock()
            .map_err(|_| "capture warning state lock poisoned during setup".to_string())?;
        let revision = state.desired.revision.wrapping_add(1);
        state.desired = DesiredWarning {
            visible: false,
            message: String::new(),
            cursor_x: 0,
            cursor_y: 0,
            revision,
        };
        state.applied_revision = revision.wrapping_sub(1);
    }

    let sender = start_update_worker(app.clone())?;
    {
        let mut current = UPDATE_SENDER
            .lock()
            .map_err(|_| "capture warning update sender lock poisoned during setup".to_string())?;
        *current = Some(sender);
    }
    notify_update_worker()
}

pub fn show(_app: &AppHandle, message: &str, cursor_x: i32, cursor_y: i32) -> Result<(), String> {
    {
        let mut state = STATE
            .lock()
            .map_err(|_| "capture warning state lock poisoned while showing".to_string())?;
        if state.desired.visible
            && state.desired.message == message
            && state.desired.cursor_x == cursor_x
            && state.desired.cursor_y == cursor_y
        {
            return Ok(());
        }
        state.desired.visible = true;
        state.desired.message.clear();
        state.desired.message.push_str(message);
        state.desired.cursor_x = cursor_x;
        state.desired.cursor_y = cursor_y;
        state.desired.revision = state.desired.revision.wrapping_add(1);
    }
    notify_update_worker()
}

pub fn hide(_app: &AppHandle) -> Result<(), String> {
    {
        let mut state = STATE
            .lock()
            .map_err(|_| "capture warning state lock poisoned while hiding".to_string())?;
        if !state.desired.visible {
            return Ok(());
        }
        state.desired.visible = false;
        state.desired.message.clear();
        state.desired.revision = state.desired.revision.wrapping_add(1);
    }
    notify_update_worker()
}

fn start_update_worker(app: AppHandle) -> Result<SyncSender<()>, String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("remember-capture-warning".to_string())
        .spawn(move || run_update_worker(app, receiver))
        .map_err(|error| format!("capture warning update worker could not start: {error}"))?;
    Ok(sender)
}

fn run_update_worker(app: AppHandle, receiver: Receiver<()>) {
    loop {
        if receiver.recv().is_err() {
            return;
        }

        let started = Instant::now();
        let mut disconnected = false;
        loop {
            let remaining = UPDATE_INTERVAL.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            match receiver.recv_timeout(remaining) {
                Ok(()) => {}
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        schedule_main_flush(&app);
        if disconnected {
            return;
        }
    }
}

fn schedule_main_flush(app: &AppHandle) {
    if MAIN_FLUSH_PENDING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    let app_for_main = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        let result = apply_latest_on_main(&app_for_main);
        MAIN_FLUSH_PENDING.store(false, Ordering::Release);
        if let Err(error) = result {
            eprintln!("Remember capture warning update failed: {error}");
        }
        if has_unapplied_state() {
            if let Err(error) = notify_update_worker() {
                eprintln!("Remember capture warning reschedule failed: {error}");
            }
        }
    }) {
        MAIN_FLUSH_PENDING.store(false, Ordering::Release);
        eprintln!("Remember capture warning main-thread scheduling failed: {error}");
    }
}

fn notify_update_worker() -> Result<(), String> {
    let sender = UPDATE_SENDER
        .lock()
        .map_err(|_| "capture warning update sender lock poisoned".to_string())?
        .clone()
        .ok_or_else(|| "capture warning controller has not been set up".to_string())?;
    match sender.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) => Ok(()),
        Err(TrySendError::Disconnected(())) => {
            Err("capture warning update worker has stopped".to_string())
        }
    }
}

fn has_unapplied_state() -> bool {
    STATE
        .lock()
        .map(|state| state.desired.revision != state.applied_revision)
        .unwrap_or(false)
}

fn desired_snapshot() -> Result<DesiredWarning, String> {
    STATE
        .lock()
        .map(|state| state.desired.clone())
        .map_err(|_| "capture warning state lock poisoned while applying".to_string())
}

fn mark_applied(revision: u64) {
    if let Ok(mut state) = STATE.lock() {
        state.applied_revision = revision;
    }
}

fn apply_latest_on_main(app: &AppHandle) -> Result<(), String> {
    let desired = desired_snapshot()?;
    let result = apply_desired_on_main(app, &desired);
    mark_applied(desired.revision);
    result
}

fn apply_desired_on_main(app: &AppHandle, desired: &DesiredWarning) -> Result<(), String> {
    if !desired.visible {
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            window
                .hide()
                .map_err(|error| format!("capture warning could not hide: {error}"))?;
        }
        return Ok(());
    }

    let window = ensure_window(app)?;
    if !PAGE_READY.load(Ordering::Acquire) {
        return Ok(());
    }
    apply_visible_warning(&window, desired)
}

fn ensure_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        if !CONFIGURED.load(Ordering::Acquire) {
            configure_window(&window)?;
        }
        return Ok(window);
    }

    let _create_guard = CREATE_LOCK
        .lock()
        .map_err(|_| "capture warning creation lock poisoned".to_string())?;
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        if !CONFIGURED.load(Ordering::Acquire) {
            configure_window(&window)?;
        }
        return Ok(window);
    }

    PAGE_READY.store(false, Ordering::Release);
    CONFIGURED.store(false, Ordering::Release);
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == WINDOW_LABEL)
        .cloned()
        .ok_or_else(|| "capture warning window configuration is unavailable".to_string())?;
    let window = WebviewWindowBuilder::from_config(app, &config)
        .map_err(|error| format!("capture warning window builder failed: {error}"))?
        .on_page_load(|window, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            PAGE_READY.store(true, Ordering::Release);
            let window_for_main = window.clone();
            if let Err(error) = window.run_on_main_thread(move || {
                if let Err(error) = apply_latest_to_window(&window_for_main) {
                    eprintln!("Remember capture warning page-ready update failed: {error}");
                }
            }) {
                eprintln!("Remember capture warning page-ready scheduling failed: {error}");
            }
        })
        .build()
        .map_err(|error| format!("capture warning window creation failed: {error}"))?;
    configure_window(&window)?;
    Ok(window)
}

fn configure_window(window: &WebviewWindow) -> Result<(), String> {
    window
        .set_focusable(false)
        .map_err(|error| format!("capture warning could not disable focus: {error}"))?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| format!("capture warning could not ignore cursor events: {error}"))?;
    CONFIGURED.store(true, Ordering::Release);
    Ok(())
}

fn apply_latest_to_window(window: &WebviewWindow) -> Result<(), String> {
    let desired = desired_snapshot()?;
    let result = if desired.visible {
        apply_visible_warning(window, &desired)
    } else {
        window
            .hide()
            .map_err(|error| format!("capture warning could not hide after page load: {error}"))
    };
    mark_applied(desired.revision);
    result
}

fn apply_visible_warning(window: &WebviewWindow, desired: &DesiredWarning) -> Result<(), String> {
    let window_size = window
        .outer_size()
        .map_err(|error| format!("capture warning size could not be read: {error}"))?;
    let bounds = virtual_desktop_bounds()?;
    let (x, y) = warning_position(
        desired.cursor_x,
        desired.cursor_y,
        window_size.width,
        window_size.height,
        bounds,
    );
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| format!("capture warning position could not update: {error}"))?;
    window
        .emit(
            WARNING_EVENT,
            CaptureWarningPayload {
                message: desired.message.clone(),
            },
        )
        .map_err(|error| format!("capture warning message could not emit: {error}"))?;
    window
        .show()
        .map_err(|error| format!("capture warning could not show: {error}"))
}

#[cfg(target_os = "windows")]
fn virtual_desktop_bounds() -> Result<DesktopBounds, String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };

    let bounds = DesktopBounds {
        left: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
        top: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        width: unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
        height: unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
    };
    if bounds.width <= 0 || bounds.height <= 0 {
        Err("capture warning could not read virtual desktop bounds".to_string())
    } else {
        Ok(bounds)
    }
}

#[cfg(not(target_os = "windows"))]
fn virtual_desktop_bounds() -> Result<DesktopBounds, String> {
    Err("capture warning virtual desktop bounds are Windows-only".to_string())
}

fn warning_position(
    cursor_x: i32,
    cursor_y: i32,
    window_width: u32,
    window_height: u32,
    bounds: DesktopBounds,
) -> (i32, i32) {
    let left = i64::from(bounds.left);
    let top = i64::from(bounds.top);
    let right = left + i64::from(bounds.width.max(0));
    let bottom = top + i64::from(bounds.height.max(0));
    let width = i64::from(window_width);
    let height = i64::from(window_height);
    let margin = i64::from(SCREEN_MARGIN);
    let cursor_x = i64::from(cursor_x);
    let cursor_y = i64::from(cursor_y);

    let preferred_x = cursor_x + i64::from(CURSOR_OFFSET_X);
    let preferred_y = cursor_y + i64::from(CURSOR_OFFSET_Y);
    let flipped_x = cursor_x - i64::from(CURSOR_OFFSET_X) - width;
    let flipped_y = cursor_y - i64::from(CURSOR_OFFSET_Y) - height;
    let x = if preferred_x + width + margin <= right {
        preferred_x
    } else {
        flipped_x
    };
    let y = if preferred_y + height + margin <= bottom {
        preferred_y
    } else {
        flipped_y
    };

    let min_x = left + margin;
    let min_y = top + margin;
    let max_x = (right - margin - width).max(min_x);
    let max_y = (bottom - margin - height).max(min_y);
    (
        clamp_i64_to_i32(x.clamp(min_x, max_x)),
        clamp_i64_to_i32(y.clamp(min_y, max_y)),
    )
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::{warning_position, DesktopBounds};

    const BOUNDS: DesktopBounds = DesktopBounds {
        left: 0,
        top: 0,
        width: 1920,
        height: 1080,
    };

    #[test]
    fn places_warning_below_and_to_the_right_when_space_is_available() {
        assert_eq!(warning_position(100, 200, 360, 72, BOUNDS), (118, 222));
    }

    #[test]
    fn flips_warning_away_from_the_bottom_right_edges() {
        assert_eq!(warning_position(1900, 1060, 360, 72, BOUNDS), (1522, 966));
    }

    #[test]
    fn clamps_warning_inside_negative_virtual_desktop_bounds() {
        let bounds = DesktopBounds {
            left: -1920,
            top: -200,
            width: 3840,
            height: 1280,
        };

        assert_eq!(
            warning_position(-2500, -500, 360, 72, bounds),
            (-1912, -192)
        );
    }

    #[test]
    fn keeps_an_oversized_warning_anchored_to_the_desktop_margin() {
        let bounds = DesktopBounds {
            left: 10,
            top: 20,
            width: 100,
            height: 50,
        };

        assert_eq!(warning_position(50, 30, 360, 72, bounds), (18, 28));
    }
}
