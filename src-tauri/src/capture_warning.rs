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
    AppHandle, Emitter, Listener, Manager, PhysicalPosition, WebviewWindow, WebviewWindowBuilder,
};

const WINDOW_LABEL: &str = "capture-warning";
const WARNING_EVENT: &str = "remember://capture-warning";
const WARNING_READY_EVENT: &str = "remember://capture-warning-ready";
const CURSOR_FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const CURSOR_OFFSET_X: i32 = 18;
const CURSOR_OFFSET_Y: i32 = 22;
const SCREEN_MARGIN: i32 = 8;
const PLAYBACK_ERROR_VISIBLE_FOR: Duration = Duration::from_secs(8);
const PLAYBACK_ERROR_MAX_CHARS: usize = 56;

static STATE: Mutex<WarningState> = Mutex::new(WarningState::new());
static APPLIED_WARNING: Mutex<AppliedWarning> = Mutex::new(AppliedWarning::hidden());
static UPDATE_SENDER: Mutex<Option<SyncSender<()>>> = Mutex::new(None);
static CREATE_LOCK: Mutex<()> = Mutex::new(());
static MAIN_FLUSH_PENDING: AtomicBool = AtomicBool::new(false);
static PAGE_READY: AtomicBool = AtomicBool::new(false);
static CONFIGURED: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct DesiredWarning {
    visible: bool,
    kind: WarningKind,
    title: String,
    message: String,
    detail: String,
    cursor_x: i32,
    cursor_y: i32,
    revision: u64,
}

struct WarningState {
    desired: DesiredWarning,
    applied_revision: u64,
    notice_revision: u64,
}

impl WarningState {
    const fn new() -> Self {
        Self {
            desired: DesiredWarning {
                visible: false,
                kind: WarningKind::CaptureUnavailable,
                title: String::new(),
                message: String::new(),
                detail: String::new(),
                cursor_x: 0,
                cursor_y: 0,
                revision: 0,
            },
            applied_revision: 0,
            notice_revision: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum WarningKind {
    CaptureUnavailable,
    PlaybackWaiting,
    PlaybackStopped,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CaptureWarningPayload {
    kind: WarningKind,
    title: String,
    message: String,
    detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedWarning {
    visible: bool,
    payload: Option<CaptureWarningPayload>,
    cursor: Option<(i32, i32)>,
}

impl AppliedWarning {
    const fn hidden() -> Self {
        Self {
            visible: false,
            payload: None,
            cursor: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibleUpdateDelta {
    move_window: bool,
    emit_payload: bool,
    show_window: bool,
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
            kind: WarningKind::CaptureUnavailable,
            title: String::new(),
            message: String::new(),
            detail: String::new(),
            cursor_x: 0,
            cursor_y: 0,
            revision,
        };
        state.applied_revision = revision.wrapping_sub(1);
        state.notice_revision = state.notice_revision.wrapping_add(1);
    }
    reset_applied_warning()?;

    let sender = start_update_worker(app.clone())?;
    {
        let mut current = UPDATE_SENDER
            .lock()
            .map_err(|_| "capture warning update sender lock poisoned during setup".to_string())?;
        *current = Some(sender);
    }
    app.listen(WARNING_READY_EVENT, |_| {
        PAGE_READY.store(true, Ordering::Release);
        if let Err(error) = reset_applied_warning() {
            eprintln!("Remember capture warning applied state could not reset: {error}");
        }
        if let Err(error) = notify_update_worker() {
            eprintln!("Remember capture warning ready update failed: {error}");
        }
    });
    notify_update_worker()
}

pub fn show(_app: &AppHandle, message: &str, cursor_x: i32, cursor_y: i32) -> Result<(), String> {
    show_notice(
        WarningKind::CaptureUnavailable,
        "此处无法录制",
        message,
        "把鼠标移回可读取窗口后会自动继续。",
        cursor_x,
        cursor_y,
    )?;
    Ok(())
}

pub fn show_playback_waiting_at_cursor(
    _app: &AppHandle,
    message: &str,
    seconds_remaining: u64,
) -> Result<(), String> {
    let (cursor_x, cursor_y) = current_cursor_position()?;
    show_notice(
        WarningKind::PlaybackWaiting,
        "回放正在等待",
        message,
        &format!("自动继续 · 剩余 {seconds_remaining} 秒 · 停止快捷键可取消"),
        cursor_x,
        cursor_y,
    )?;
    Ok(())
}

pub fn show_playback_stopped_at_cursor(_app: &AppHandle, message: &str) -> Result<(), String> {
    let (cursor_x, cursor_y) = current_cursor_position()?;
    let message = truncate_message(message, PLAYBACK_ERROR_MAX_CHARS);
    let revision = show_notice(
        WarningKind::PlaybackStopped,
        "回放已停止",
        &message,
        "准备好目标窗口或选项后，请重新开始回放。",
        cursor_x,
        cursor_y,
    )?;
    thread::spawn(move || {
        thread::sleep(PLAYBACK_ERROR_VISIBLE_FOR);
        hide_if_notice_revision(revision);
    });
    Ok(())
}

fn truncate_message(message: &str, max_chars: usize) -> String {
    if message.chars().count() <= max_chars {
        return message.to_string();
    }
    let mut shortened = message
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    shortened.push('…');
    shortened
}

fn show_notice(
    kind: WarningKind,
    title: &str,
    message: &str,
    detail: &str,
    cursor_x: i32,
    cursor_y: i32,
) -> Result<u64, String> {
    let notice_revision;
    {
        let mut state = STATE
            .lock()
            .map_err(|_| "capture warning state lock poisoned while showing".to_string())?;
        let content_changed = state.desired.kind != kind
            || state.desired.title != title
            || state.desired.message != message
            || state.desired.detail != detail;
        if state.desired.visible && !content_changed {
            return Ok(state.notice_revision);
        }
        state.desired.visible = true;
        state.desired.kind = kind;
        state.desired.title.clear();
        state.desired.title.push_str(title);
        state.desired.message.clear();
        state.desired.message.push_str(message);
        state.desired.detail.clear();
        state.desired.detail.push_str(detail);
        state.desired.cursor_x = cursor_x;
        state.desired.cursor_y = cursor_y;
        state.desired.revision = state.desired.revision.wrapping_add(1);
        state.notice_revision = state.notice_revision.wrapping_add(1);
        notice_revision = state.notice_revision;
    }
    notify_update_worker()?;
    Ok(notice_revision)
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
        state.desired.title.clear();
        state.desired.message.clear();
        state.desired.detail.clear();
        state.desired.revision = state.desired.revision.wrapping_add(1);
        state.notice_revision = state.notice_revision.wrapping_add(1);
    }
    notify_update_worker()
}

fn hide_if_notice_revision(revision: u64) {
    let changed = STATE.lock().map(|mut state| {
        if !state.desired.visible || state.notice_revision != revision {
            return false;
        }
        state.desired.visible = false;
        state.desired.title.clear();
        state.desired.message.clear();
        state.desired.detail.clear();
        state.desired.revision = state.desired.revision.wrapping_add(1);
        state.notice_revision = state.notice_revision.wrapping_add(1);
        true
    });
    if matches!(changed, Ok(true)) {
        let _ = notify_update_worker();
    }
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
    let mut cursor_error_reported = false;
    loop {
        if !warning_is_visible() {
            if receiver.recv().is_err() {
                return;
            }
            let _ = refresh_visible_cursor(&mut cursor_error_reported);
            schedule_main_flush(&app);
            continue;
        }

        let started = Instant::now();
        let mut notified = false;
        let mut disconnected = false;
        loop {
            let remaining = CURSOR_FRAME_INTERVAL.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            match receiver.recv_timeout(remaining) {
                Ok(()) => notified = true,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        if disconnected {
            return;
        }
        let cursor_changed = refresh_visible_cursor(&mut cursor_error_reported);
        if notified || cursor_changed || has_unapplied_state() {
            schedule_main_flush(&app);
        }
    }
}

fn warning_is_visible() -> bool {
    STATE
        .lock()
        .map(|state| state.desired.visible)
        .unwrap_or(false)
}

fn refresh_visible_cursor(error_reported: &mut bool) -> bool {
    let cursor = match current_cursor_position() {
        Ok(cursor) => {
            *error_reported = false;
            cursor
        }
        Err(error) => {
            if !*error_reported {
                eprintln!("Remember capture warning cursor tracking failed: {error}");
                *error_reported = true;
            }
            return false;
        }
    };
    STATE
        .lock()
        .map(|mut state| update_visible_cursor(&mut state, cursor))
        .unwrap_or(false)
}

fn update_visible_cursor(state: &mut WarningState, cursor: (i32, i32)) -> bool {
    if !state.desired.visible || (state.desired.cursor_x, state.desired.cursor_y) == cursor {
        return false;
    }
    state.desired.cursor_x = cursor.0;
    state.desired.cursor_y = cursor.1;
    state.desired.revision = state.desired.revision.wrapping_add(1);
    true
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
        if has_unapplied_state() {
            if let Err(error) = notify_update_worker() {
                eprintln!("Remember capture warning reschedule failed: {error}");
            }
        }
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

fn applied_warning_snapshot() -> Result<AppliedWarning, String> {
    APPLIED_WARNING
        .lock()
        .map(|applied| applied.clone())
        .map_err(|_| "capture warning applied state lock poisoned".to_string())
}

fn set_applied_warning(applied: AppliedWarning) -> Result<(), String> {
    *APPLIED_WARNING
        .lock()
        .map_err(|_| "capture warning applied state lock poisoned".to_string())? = applied;
    Ok(())
}

fn reset_applied_warning() -> Result<(), String> {
    set_applied_warning(AppliedWarning::hidden())
}

fn mark_applied(revision: u64) {
    if let Ok(mut state) = STATE.lock() {
        state.applied_revision = revision;
    }
}

fn apply_latest_on_main(app: &AppHandle) -> Result<(), String> {
    let desired = desired_snapshot()?;
    apply_desired_on_main(app, &desired)?;
    mark_applied(desired.revision);
    Ok(())
}

fn apply_desired_on_main(app: &AppHandle, desired: &DesiredWarning) -> Result<(), String> {
    if !desired.visible {
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            let visible = window.is_visible().map_err(|error| {
                format!("capture warning visibility could not be read: {error}")
            })?;
            if visible {
                window
                    .hide()
                    .map_err(|error| format!("capture warning could not hide: {error}"))?;
            }
        }
        reset_applied_warning()?;
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

fn apply_visible_warning(window: &WebviewWindow, desired: &DesiredWarning) -> Result<(), String> {
    let payload = CaptureWarningPayload {
        kind: desired.kind,
        title: desired.title.clone(),
        message: desired.message.clone(),
        detail: desired.detail.clone(),
    };
    let cursor = (desired.cursor_x, desired.cursor_y);
    let applied = applied_warning_snapshot()?;
    let delta = visible_update_delta(&applied, &payload, cursor);

    if delta.move_window {
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
    }
    if delta.emit_payload {
        window
            .emit(WARNING_EVENT, payload.clone())
            .map_err(|error| format!("capture warning message could not emit: {error}"))?;
    }
    if delta.show_window {
        window
            .show()
            .map_err(|error| format!("capture warning could not show: {error}"))?;
    }
    set_applied_warning(AppliedWarning {
        visible: true,
        payload: Some(payload),
        cursor: Some(cursor),
    })
}

fn visible_update_delta(
    applied: &AppliedWarning,
    payload: &CaptureWarningPayload,
    cursor: (i32, i32),
) -> VisibleUpdateDelta {
    VisibleUpdateDelta {
        move_window: !applied.visible || applied.cursor != Some(cursor),
        emit_payload: !applied.visible || applied.payload.as_ref() != Some(payload),
        show_window: !applied.visible,
    }
}

#[cfg(target_os = "windows")]
fn current_cursor_position() -> Result<(i32, i32), String> {
    use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};

    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }
        .map_err(|error| format!("capture warning could not read cursor position: {error}"))?;
    Ok((point.x, point.y))
}

#[cfg(not(target_os = "windows"))]
fn current_cursor_position() -> Result<(i32, i32), String> {
    Err("capture warning cursor tracking is Windows-only".to_string())
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
    use super::{
        truncate_message, update_visible_cursor, visible_update_delta, warning_position,
        AppliedWarning, CaptureWarningPayload, DesktopBounds, VisibleUpdateDelta, WarningKind,
        WarningState, CURSOR_FRAME_INTERVAL,
    };
    use std::time::Duration;

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

    #[test]
    fn stopped_notice_truncation_preserves_short_text_and_marks_long_text() {
        assert_eq!(truncate_message("Mihomo 不存在", 20), "Mihomo 不存在");
        assert_eq!(truncate_message("一二三四五六", 5), "一二三四…");
    }

    #[test]
    fn cursor_following_targets_about_sixty_frames_per_second() {
        assert_eq!(CURSOR_FRAME_INTERVAL, Duration::from_micros(16_667));
        let frames_per_second = 1.0 / CURSOR_FRAME_INTERVAL.as_secs_f64();
        assert!((59.9..=60.1).contains(&frames_per_second));
    }

    #[test]
    fn cursor_frames_do_not_invalidate_the_stopped_notice_timer() {
        let mut state = WarningState::new();
        state.desired.visible = true;
        state.notice_revision = 7;

        assert!(update_visible_cursor(&mut state, (200, 300)));
        assert_eq!(state.notice_revision, 7);
        assert_eq!(state.desired.revision, 1);
    }

    #[test]
    fn moving_a_visible_notice_does_not_emit_or_show_it_again() {
        let payload = CaptureWarningPayload {
            kind: WarningKind::PlaybackWaiting,
            title: "回放正在等待".to_string(),
            message: "请展开下拉框".to_string(),
            detail: "还剩 30 秒".to_string(),
        };
        let applied = AppliedWarning {
            visible: true,
            payload: Some(payload.clone()),
            cursor: Some((100, 100)),
        };

        assert_eq!(
            visible_update_delta(&applied, &payload, (200, 200)),
            VisibleUpdateDelta {
                move_window: true,
                emit_payload: false,
                show_window: false,
            }
        );
    }

    #[test]
    fn changing_notice_text_only_emits_a_new_payload() {
        let original = CaptureWarningPayload {
            kind: WarningKind::PlaybackWaiting,
            title: "回放正在等待".to_string(),
            message: "请展开下拉框".to_string(),
            detail: "还剩 30 秒".to_string(),
        };
        let mut changed = original.clone();
        changed.detail = "还剩 29 秒".to_string();
        let applied = AppliedWarning {
            visible: true,
            payload: Some(original),
            cursor: Some((100, 100)),
        };

        assert_eq!(
            visible_update_delta(&applied, &changed, (100, 100)),
            VisibleUpdateDelta {
                move_window: false,
                emit_payload: true,
                show_window: false,
            }
        );
    }
}
