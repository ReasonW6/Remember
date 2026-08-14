use crate::model::{ClientSize, TargetWindowAvailability, TargetWindowId, WindowTarget};
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle(usize);

impl WindowHandle {
    pub const fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ScreenRect {
    pub fn contains(self, point: ScreenPoint) -> bool {
        point.x >= self.left && point.x < self.right && point.y >= self.top && point.y < self.bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowGeometry {
    pub client_origin: ScreenPoint,
    pub client_size: ClientSize,
    pub dpi: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowDisplayState {
    pub visible: bool,
    pub minimized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSnapshot {
    pub handle: WindowHandle,
    pub executable_path: String,
    pub window_class: String,
    pub title: String,
    pub client_origin: ScreenPoint,
    pub client_size: ClientSize,
    pub dpi: u32,
    pub process_id: u32,
    pub visible: bool,
    pub minimized: bool,
}

impl WindowSnapshot {
    pub fn geometry(&self) -> WindowGeometry {
        WindowGeometry {
            client_origin: self.client_origin,
            client_size: self.client_size,
            dpi: self.dpi,
        }
    }

    pub fn to_target(
        &self,
        id: TargetWindowId,
        availability: TargetWindowAvailability,
    ) -> WindowTarget {
        WindowTarget {
            id,
            executable_path: self.executable_path.clone(),
            window_class: self.window_class.clone(),
            title: self.title.clone(),
            client_size: self.client_size,
            dpi: self.dpi,
            availability,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowTargetError {
    UnsupportedPlatform,
    WindowUnavailable {
        handle: WindowHandle,
    },
    WindowLifetimeChanged {
        handle: WindowHandle,
    },
    AccessDenied {
        operation: &'static str,
        process_id: Option<u32>,
    },
    HigherIntegrityTarget {
        process_id: u32,
        current_rid: u32,
        target_rid: u32,
    },
    ApiFailure {
        operation: &'static str,
        code: i32,
        detail: String,
    },
    InvalidGeometry {
        handle: WindowHandle,
    },
    CoordinateOverflow {
        handle: WindowHandle,
        relative_x: i32,
        relative_y: i32,
    },
    ForegroundActivationDenied {
        handle: WindowHandle,
    },
    WindowDisplayTimedOut {
        handle: WindowHandle,
    },
}

impl fmt::Display for WindowTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter.write_str("窗口定位功能仅支持 Windows。"),
            Self::WindowUnavailable { handle } => write!(
                formatter,
                "目标窗口已关闭或不可用（窗口句柄 0x{:X}）。",
                handle.raw()
            ),
            Self::WindowLifetimeChanged { handle } => write!(
                formatter,
                "目标窗口在输入事件排队期间已关闭或被替换（窗口句柄 0x{:X}）。",
                handle.raw()
            ),
            Self::AccessDenied {
                operation,
                process_id: Some(process_id),
            } => write!(
                formatter,
                "无法{operation}（PID {process_id}）：访问被拒绝，请使用管理员身份重试。"
            ),
            Self::AccessDenied {
                operation,
                process_id: None,
            } => write!(
                formatter,
                "无法{operation}：访问被拒绝，请使用管理员身份重试。"
            ),
            Self::HigherIntegrityTarget {
                process_id,
                current_rid,
                target_rid,
            } => write!(
                formatter,
                "无法操作窗口（PID {process_id}）：目标程序的完整性级别（RID {target_rid}）高于 Remember（RID {current_rid}），请使用管理员身份重试。"
            ),
            Self::ApiFailure {
                operation,
                code,
                detail,
            } => write!(
                formatter,
                "{operation}失败（Windows 错误 0x{:08X}）：{detail}",
                *code as u32
            ),
            Self::InvalidGeometry { handle } => write!(
                formatter,
                "目标窗口返回了无效的客户区尺寸（窗口句柄 0x{:X}）。",
                handle.raw()
            ),
            Self::CoordinateOverflow {
                handle,
                relative_x,
                relative_y,
            } => write!(
                formatter,
                "目标窗口的相对坐标 ({relative_x}, {relative_y}) 无法转换为屏幕坐标（窗口句柄 0x{:X}）。",
                handle.raw()
            ),
            Self::ForegroundActivationDenied { handle } => write!(
                formatter,
                "Windows 未能在等待时间内将目标窗口切换到前台（窗口句柄 0x{:X}），请先手动激活该窗口。",
                handle.raw()
            ),
            Self::WindowDisplayTimedOut { handle } => write!(
                formatter,
                "目标窗口已收到显示或恢复请求，但在等待时间内仍处于隐藏或最小化状态（窗口句柄 0x{:X}）。",
                handle.raw()
            ),
        }
    }
}

impl Error for WindowTargetError {}

pub fn window_from_screen_point(
    point: ScreenPoint,
) -> Result<Option<WindowSnapshot>, WindowTargetError> {
    platform::window_from_screen_point(point)
}

pub fn snapshot_window(handle: WindowHandle) -> Result<WindowSnapshot, WindowTargetError> {
    platform::snapshot_window(handle)
}

/// Resolves owned transient roots to their stable owner without reading process metadata.
pub fn normalize_window_handle(
    handle: WindowHandle,
) -> Result<Option<WindowHandle>, WindowTargetError> {
    platform::normalize_window_handle(handle)
}

/// Resolves a root/hit HWND exactly as pointer capture does, then takes a full snapshot.
pub fn snapshot_pointed_window(
    handle: WindowHandle,
) -> Result<Option<WindowSnapshot>, WindowTargetError> {
    platform::snapshot_pointed_window(handle)
}

/// Resolves an event-time foreground root HWND without querying the current foreground again.
pub fn snapshot_foreground_window(
    handle: WindowHandle,
) -> Result<Option<WindowSnapshot>, WindowTargetError> {
    platform::snapshot_foreground_window(handle)
}

pub fn foreground_window() -> Result<Option<WindowSnapshot>, WindowTargetError> {
    platform::foreground_window()
}

pub fn enumerate_target_windows() -> Result<Vec<WindowSnapshot>, WindowTargetError> {
    platform::enumerate_target_windows()
}

pub fn enumerate_matching_windows(
    target: &WindowTarget,
) -> Result<Vec<WindowSnapshot>, WindowTargetError> {
    let mut candidates = platform::enumerate_matching_windows(target)?;
    sort_candidates_for_target(target, &mut candidates);
    Ok(candidates)
}

pub fn refresh_window_geometry(handle: WindowHandle) -> Result<WindowGeometry, WindowTargetError> {
    platform::refresh_window_geometry(handle)
}

pub fn window_display_state(handle: WindowHandle) -> Result<WindowDisplayState, WindowTargetError> {
    platform::window_display_state(handle)
}

pub fn window_is_alive(handle: WindowHandle) -> bool {
    platform::window_is_alive(handle)
}

pub fn restore_and_activate_window(handle: WindowHandle) -> Result<(), WindowTargetError> {
    platform::restore_and_activate_window(handle)
}

pub fn window_relative_to_screen(
    handle: WindowHandle,
    relative_x: i32,
    relative_y: i32,
) -> Result<ScreenPoint, WindowTargetError> {
    let geometry = refresh_window_geometry(handle)?;
    relative_to_screen(handle, geometry.client_origin, relative_x, relative_y)
}

pub fn virtual_desktop_bounds() -> Result<ScreenRect, WindowTargetError> {
    platform::virtual_desktop_bounds()
}

pub fn is_screen_point_reachable(point: ScreenPoint) -> Result<bool, WindowTargetError> {
    Ok(virtual_desktop_bounds()?.contains(point))
}

pub fn matches_recorded_target(target: &WindowTarget, candidate: &WindowSnapshot) -> bool {
    windows_path_eq(&target.executable_path, &candidate.executable_path)
        && target.window_class == candidate.window_class
}

pub fn sort_candidates_for_target(target: &WindowTarget, candidates: &mut [WindowSnapshot]) {
    candidates.sort_by(|left, right| compare_candidate_titles(&target.title, left, right));
}

fn compare_candidate_titles(
    recorded_title: &str,
    left: &WindowSnapshot,
    right: &WindowSnapshot,
) -> Ordering {
    title_match_rank(recorded_title, &left.title)
        .cmp(&title_match_rank(recorded_title, &right.title))
        .then_with(|| left.title.cmp(&right.title))
        .then_with(|| left.handle.raw().cmp(&right.handle.raw()))
}

fn title_match_rank(recorded_title: &str, candidate_title: &str) -> u8 {
    if candidate_title == recorded_title {
        return 0;
    }

    let recorded_folded = recorded_title.to_lowercase();
    let candidate_folded = candidate_title.to_lowercase();
    if candidate_folded == recorded_folded {
        1
    } else if !recorded_folded.is_empty()
        && (candidate_folded.contains(&recorded_folded)
            || recorded_folded.contains(&candidate_folded))
    {
        2
    } else {
        3
    }
}

fn windows_path_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right) || left.to_lowercase() == right.to_lowercase()
}

fn checked_client_size(
    handle: WindowHandle,
    width: i32,
    height: i32,
) -> Result<ClientSize, WindowTargetError> {
    let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
        return Err(WindowTargetError::InvalidGeometry { handle });
    };
    if width == 0 || height == 0 {
        return Err(WindowTargetError::InvalidGeometry { handle });
    }
    Ok(ClientSize { width, height })
}

fn display_state_ready(visible: bool, minimized: bool) -> bool {
    visible && !minimized
}

fn relative_to_screen(
    handle: WindowHandle,
    origin: ScreenPoint,
    relative_x: i32,
    relative_y: i32,
) -> Result<ScreenPoint, WindowTargetError> {
    let x = origin
        .x
        .checked_add(relative_x)
        .ok_or(WindowTargetError::CoordinateOverflow {
            handle,
            relative_x,
            relative_y,
        })?;
    let y = origin
        .y
        .checked_add(relative_y)
        .ok_or(WindowTargetError::CoordinateOverflow {
            handle,
            relative_x,
            relative_y,
        })?;
    Ok(ScreenPoint { x, y })
}

fn is_tooltip_class(window_class: &str) -> bool {
    window_class.eq_ignore_ascii_case("tooltips_class32")
}

fn is_owned_transient_class(window_class: &str) -> bool {
    ["#32768", "ComboLBox", "SysShadow", "IME", "MSCTFIME UI"]
        .iter()
        .any(|candidate| window_class.eq_ignore_ascii_case(candidate))
}

fn is_system_surface_class(window_class: &str) -> bool {
    [
        "#32769",
        "Progman",
        "WorkerW",
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "NotifyIconOverflowWindow",
        "TaskListThumbnailWnd",
        "MultitaskingViewFrame",
    ]
    .iter()
    .any(|candidate| window_class.eq_ignore_ascii_case(candidate))
}

fn is_system_surface(snapshot: &WindowSnapshot) -> bool {
    if is_system_surface_class(&snapshot.window_class) {
        return true;
    }

    let executable_name = snapshot
        .executable_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(&snapshot.executable_path);
    [
        "StartMenuExperienceHost.exe",
        "ShellExperienceHost.exe",
        "SearchHost.exe",
        "SearchApp.exe",
        "TextInputHost.exe",
        "LockApp.exe",
        "sihost.exe",
    ]
    .iter()
    .any(|candidate| executable_name.eq_ignore_ascii_case(candidate))
}

fn is_initial_target(snapshot: &WindowSnapshot) -> bool {
    snapshot.visible
        && !is_tooltip_class(&snapshot.window_class)
        && !is_owned_transient_class(&snapshot.window_class)
        && !is_system_surface(snapshot)
}

#[cfg(any(target_os = "windows", test))]
fn target_integrity_is_readable(current_rid: u32, target_rid: u32) -> bool {
    target_rid <= current_rid
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::{
        ffi::c_void,
        mem::size_of,
        sync::OnceLock,
        thread,
        time::{Duration, Instant},
    };
    use windows::{
        core::{Error as WindowsError, HRESULT, PWSTR},
        Win32::{
            Foundation::{
                CloseHandle, BOOL, ERROR_ACCESS_DENIED, ERROR_INSUFFICIENT_BUFFER, HANDLE, HWND,
                LPARAM, POINT, RECT,
            },
            Graphics::Gdi::ClientToScreen,
            Security::{
                GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation,
                TokenIntegrityLevel, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
            },
            System::Threading::{
                GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
                PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            },
            UI::{
                HiDpi::GetDpiForWindow,
                WindowsAndMessaging::{
                    EnumWindows, GetAncestor, GetClassNameW, GetClientRect, GetForegroundWindow,
                    GetSystemMetrics, GetWindow, GetWindowTextLengthW, GetWindowTextW,
                    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
                    SetForegroundWindow, ShowWindowAsync, WindowFromPoint, GA_ROOT, GW_OWNER,
                    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
                    SW_RESTORE, SW_SHOW,
                },
            },
        },
    };

    const MAX_PROCESS_PATH_CHARS: usize = 32_768;
    const WINDOW_STATE_TIMEOUT: Duration = Duration::from_millis(500);
    const WINDOW_STATE_POLL_INTERVAL: Duration = Duration::from_millis(10);
    static CURRENT_PROCESS_INTEGRITY_RID: OnceLock<Result<u32, WindowTargetError>> =
        OnceLock::new();

    pub(super) fn window_from_screen_point(
        point: ScreenPoint,
    ) -> Result<Option<WindowSnapshot>, WindowTargetError> {
        let hit = unsafe {
            WindowFromPoint(POINT {
                x: point.x,
                y: point.y,
            })
        };
        if hit.0.is_null() {
            return Ok(None);
        }

        snapshot_pointed_window(window_handle(hit))
    }

    pub(super) fn snapshot_window(
        handle: WindowHandle,
    ) -> Result<WindowSnapshot, WindowTargetError> {
        snapshot_hwnd(hwnd(handle))
    }

    pub(super) fn normalize_window_handle(
        handle: WindowHandle,
    ) -> Result<Option<WindowHandle>, WindowTargetError> {
        Ok(normalize_target_window(hwnd(handle))?.map(window_handle))
    }

    pub(super) fn snapshot_pointed_window(
        handle: WindowHandle,
    ) -> Result<Option<WindowSnapshot>, WindowTargetError> {
        let Some(target) = normalize_target_window(hwnd(handle))? else {
            return Ok(None);
        };
        let snapshot = snapshot_hwnd(target)?;
        if is_system_surface(&snapshot) {
            Ok(None)
        } else {
            Ok(Some(snapshot))
        }
    }

    pub(super) fn snapshot_foreground_window(
        handle: WindowHandle,
    ) -> Result<Option<WindowSnapshot>, WindowTargetError> {
        let Some(target) = normalize_target_window(hwnd(handle))? else {
            return Ok(None);
        };
        let snapshot = snapshot_hwnd(target)?;
        if is_initial_target(&snapshot) {
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }

    pub(super) fn foreground_window() -> Result<Option<WindowSnapshot>, WindowTargetError> {
        let foreground = unsafe { GetForegroundWindow() };
        if foreground.0.is_null() {
            return Ok(None);
        }
        snapshot_foreground_window(window_handle(foreground))
    }

    pub(super) fn enumerate_target_windows() -> Result<Vec<WindowSnapshot>, WindowTargetError> {
        struct EnumerationContext {
            snapshots: Vec<WindowSnapshot>,
        }

        unsafe extern "system" fn visit_window(raw: HWND, state: LPARAM) -> BOOL {
            let context = &mut *(state.0 as *mut EnumerationContext);
            if !IsWindow(raw).as_bool() || !IsWindowVisible(raw).as_bool() {
                return BOOL(1);
            }

            let window_class = match read_window_class(raw) {
                Ok(window_class) => window_class,
                Err(_) => return BOOL(1),
            };
            if is_tooltip_class(&window_class)
                || is_owned_transient_class(&window_class)
                || is_system_surface_class(&window_class)
            {
                return BOOL(1);
            }

            if let Ok(snapshot) = snapshot_hwnd(raw) {
                if is_initial_target(&snapshot) {
                    context.snapshots.push(snapshot);
                }
            }
            BOOL(1)
        }

        let mut context = EnumerationContext {
            snapshots: Vec::new(),
        };
        unsafe {
            EnumWindows(
                Some(visit_window),
                LPARAM((&mut context as *mut EnumerationContext) as isize),
            )
        }
        .map_err(|error| windows_api_error("枚举可用目标窗口", None, error))?;
        Ok(context.snapshots)
    }

    pub(super) fn enumerate_matching_windows(
        target: &WindowTarget,
    ) -> Result<Vec<WindowSnapshot>, WindowTargetError> {
        struct EnumerationContext {
            executable_path: String,
            window_class: String,
            candidates: Vec<WindowSnapshot>,
        }

        unsafe extern "system" fn visit_window(raw: HWND, state: LPARAM) -> BOOL {
            let context = &mut *(state.0 as *mut EnumerationContext);
            if !IsWindow(raw).as_bool() {
                return BOOL(1);
            }

            let window_class = match read_window_class(raw) {
                Ok(window_class) => window_class,
                Err(_) => return BOOL(1),
            };
            if window_class != context.window_class
                || is_tooltip_class(&window_class)
                || is_owned_transient_class(&window_class)
                || is_system_surface_class(&window_class)
            {
                return BOOL(1);
            }

            match snapshot_hwnd(raw) {
                Ok(snapshot)
                    if !is_system_surface(&snapshot)
                        && windows_path_eq(&snapshot.executable_path, &context.executable_path) =>
                {
                    context.candidates.push(snapshot);
                }
                Ok(_) => {}
                // A same-class window can belong to an unrelated, inaccessible process.
                // It is not evidence that the recorded target is unreadable, so keep
                // enumerating and let the deferred binder wait for a real candidate.
                Err(_) => {}
            }
            BOOL(1)
        }

        let mut context = EnumerationContext {
            executable_path: target.executable_path.clone(),
            window_class: target.window_class.clone(),
            candidates: Vec::new(),
        };
        unsafe {
            EnumWindows(
                Some(visit_window),
                LPARAM((&mut context as *mut EnumerationContext) as isize),
            )
        }
        .map_err(|error| windows_api_error("枚举顶层窗口", None, error))?;

        Ok(context.candidates)
    }

    pub(super) fn refresh_window_geometry(
        handle: WindowHandle,
    ) -> Result<WindowGeometry, WindowTargetError> {
        read_geometry(hwnd(handle), handle)
    }

    pub(super) fn window_display_state(
        handle: WindowHandle,
    ) -> Result<WindowDisplayState, WindowTargetError> {
        let raw = hwnd(handle);
        ensure_alive(raw, handle)?;
        Ok(WindowDisplayState {
            visible: unsafe { IsWindowVisible(raw).as_bool() },
            minimized: unsafe { IsIconic(raw).as_bool() },
        })
    }

    pub(super) fn window_is_alive(handle: WindowHandle) -> bool {
        unsafe { IsWindow(hwnd(handle)).as_bool() }
    }

    pub(super) fn restore_and_activate_window(
        handle: WindowHandle,
    ) -> Result<(), WindowTargetError> {
        let raw = hwnd(handle);
        ensure_alive(raw, handle)?;

        let minimized = unsafe { IsIconic(raw).as_bool() };
        let visible = unsafe { IsWindowVisible(raw).as_bool() };
        let display_request = if minimized {
            Some(SW_RESTORE)
        } else if !visible {
            Some(SW_SHOW)
        } else {
            None
        };
        if let Some(command) = display_request {
            // This return value describes the window's previous visibility, not whether the
            // asynchronous request succeeded. Hidden windows legitimately return false.
            unsafe {
                let _ = ShowWindowAsync(raw, command);
            }
            let deadline = Instant::now() + WINDOW_STATE_TIMEOUT;
            loop {
                ensure_alive(raw, handle)?;
                if display_state_ready(unsafe { IsWindowVisible(raw).as_bool() }, unsafe {
                    IsIconic(raw).as_bool()
                }) {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(WindowTargetError::WindowDisplayTimedOut { handle });
                }
                thread::sleep(WINDOW_STATE_POLL_INTERVAL);
            }
        }

        ensure_alive(raw, handle)?;
        if unsafe { GetForegroundWindow() }.0 == raw.0 {
            return Ok(());
        }
        let _ = unsafe { SetForegroundWindow(raw) };
        let deadline = Instant::now() + WINDOW_STATE_TIMEOUT;
        loop {
            ensure_alive(raw, handle)?;
            if unsafe { GetForegroundWindow() }.0 == raw.0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(WindowTargetError::ForegroundActivationDenied { handle });
            }
            thread::sleep(WINDOW_STATE_POLL_INTERVAL);
        }
    }

    pub(super) fn virtual_desktop_bounds() -> Result<ScreenRect, WindowTargetError> {
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        if width <= 0 || height <= 0 {
            return Err(WindowTargetError::ApiFailure {
                operation: "读取虚拟桌面范围",
                code: 0,
                detail: format!("系统返回了无效尺寸 {width} × {height}"),
            });
        }
        let Some(right) = left.checked_add(width) else {
            return Err(WindowTargetError::ApiFailure {
                operation: "读取虚拟桌面范围",
                code: 0,
                detail: "虚拟桌面横向范围溢出".to_string(),
            });
        };
        let Some(bottom) = top.checked_add(height) else {
            return Err(WindowTargetError::ApiFailure {
                operation: "读取虚拟桌面范围",
                code: 0,
                detail: "虚拟桌面纵向范围溢出".to_string(),
            });
        };
        Ok(ScreenRect {
            left,
            top,
            right,
            bottom,
        })
    }

    fn normalize_target_window(hit: HWND) -> Result<Option<HWND>, WindowTargetError> {
        let root = unsafe { GetAncestor(hit, GA_ROOT) };
        let mut current = if root.0.is_null() { hit } else { root };

        for _ in 0..8 {
            let window_class = read_window_class(current)?;
            if is_tooltip_class(&window_class) {
                return Ok(None);
            }
            if !is_owned_transient_class(&window_class) {
                break;
            }

            let Some(owner) = (unsafe { GetWindow(current, GW_OWNER).ok() }) else {
                break;
            };
            let owner_root = unsafe { GetAncestor(owner, GA_ROOT) };
            let next = if owner_root.0.is_null() {
                owner
            } else {
                owner_root
            };
            if next.0 == current.0 {
                break;
            }
            current = next;
        }

        let window_class = read_window_class(current)?;
        if is_tooltip_class(&window_class) || is_system_surface_class(&window_class) {
            Ok(None)
        } else {
            Ok(Some(current))
        }
    }

    fn snapshot_hwnd(raw: HWND) -> Result<WindowSnapshot, WindowTargetError> {
        let handle = window_handle(raw);
        ensure_alive(raw, handle)?;
        let process_id = read_process_id(raw, handle)?;
        let executable_path = read_process_path(process_id)?;
        let window_class = read_window_class(raw)?;
        let title = read_window_title(raw, handle)?;
        let geometry = read_geometry(raw, handle)?;
        ensure_alive(raw, handle)?;

        Ok(WindowSnapshot {
            handle,
            executable_path,
            window_class,
            title,
            client_origin: geometry.client_origin,
            client_size: geometry.client_size,
            dpi: geometry.dpi,
            process_id,
            visible: unsafe { IsWindowVisible(raw).as_bool() },
            minimized: unsafe { IsIconic(raw).as_bool() },
        })
    }

    fn read_process_id(raw: HWND, handle: WindowHandle) -> Result<u32, WindowTargetError> {
        let mut process_id = 0;
        let thread_id = unsafe { GetWindowThreadProcessId(raw, Some(&mut process_id)) };
        if thread_id == 0 || process_id == 0 {
            if !window_is_alive(handle) {
                return Err(WindowTargetError::WindowUnavailable { handle });
            }
            return Err(WindowTargetError::ApiFailure {
                operation: "读取窗口进程标识",
                code: WindowsError::from_win32().code().0,
                detail: "系统没有返回窗口所属进程".to_string(),
            });
        }
        Ok(process_id)
    }

    fn read_process_path(process_id: u32) -> Result<String, WindowTargetError> {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
            .map(OwnedProcessHandle)
            .map_err(|error| windows_api_error("读取窗口程序路径", Some(process_id), error))?;
        ensure_process_integrity_is_readable(process.0, process_id)?;

        let mut capacity = 512usize;
        loop {
            let mut buffer = vec![0u16; capacity];
            let mut length = buffer.len() as u32;
            let result = unsafe {
                QueryFullProcessImageNameW(
                    process.0,
                    PROCESS_NAME_WIN32,
                    PWSTR(buffer.as_mut_ptr()),
                    &mut length,
                )
            };
            match result {
                Ok(()) => {
                    buffer.truncate(length as usize);
                    return Ok(String::from_utf16_lossy(&buffer));
                }
                Err(error)
                    if error.code() == HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0)
                        && capacity < MAX_PROCESS_PATH_CHARS =>
                {
                    capacity = (capacity * 2)
                        .max(length as usize + 1)
                        .min(MAX_PROCESS_PATH_CHARS);
                }
                Err(error) => {
                    return Err(windows_api_error(
                        "读取窗口程序路径",
                        Some(process_id),
                        error,
                    ));
                }
            }
        }
    }

    fn ensure_process_integrity_is_readable(
        process: HANDLE,
        process_id: u32,
    ) -> Result<(), WindowTargetError> {
        let current_rid = current_process_integrity_rid()?;
        let target_rid = read_process_integrity_rid(process, Some(process_id))?;
        if target_integrity_is_readable(current_rid, target_rid) {
            Ok(())
        } else {
            Err(WindowTargetError::HigherIntegrityTarget {
                process_id,
                current_rid,
                target_rid,
            })
        }
    }

    fn current_process_integrity_rid() -> Result<u32, WindowTargetError> {
        CURRENT_PROCESS_INTEGRITY_RID
            .get_or_init(|| read_process_integrity_rid(unsafe { GetCurrentProcess() }, None))
            .clone()
    }

    fn read_process_integrity_rid(
        process: HANDLE,
        process_id: Option<u32>,
    ) -> Result<u32, WindowTargetError> {
        let operation = "读取进程完整性级别";
        let mut token = HANDLE::default();
        unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }
            .map_err(|error| windows_api_error(operation, process_id, error))?;
        let token = OwnedTokenHandle(token);

        let mut required_length = 0u32;
        let size_result = unsafe {
            GetTokenInformation(token.0, TokenIntegrityLevel, None, 0, &mut required_length)
        };
        if let Err(error) = size_result {
            if error.code() != HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0) {
                return Err(windows_api_error(operation, process_id, error));
            }
        }
        if required_length < size_of::<TOKEN_MANDATORY_LABEL>() as u32 {
            return Err(WindowTargetError::ApiFailure {
                operation,
                code: 0,
                detail: "系统返回了无效的完整性级别缓冲区大小".to_string(),
            });
        }

        let word_size = size_of::<usize>();
        let word_count = (required_length as usize).div_ceil(word_size);
        let mut buffer = vec![0usize; word_count];
        unsafe {
            GetTokenInformation(
                token.0,
                TokenIntegrityLevel,
                Some(buffer.as_mut_ptr().cast()),
                required_length,
                &mut required_length,
            )
        }
        .map_err(|error| windows_api_error(operation, process_id, error))?;

        let label = unsafe { &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>() };
        if label.Label.Sid.0.is_null() {
            return Err(WindowTargetError::ApiFailure {
                operation,
                code: 0,
                detail: "系统没有返回完整性级别 SID".to_string(),
            });
        }
        let count = unsafe { GetSidSubAuthorityCount(label.Label.Sid) };
        if count.is_null() || unsafe { *count } == 0 {
            return Err(WindowTargetError::ApiFailure {
                operation,
                code: 0,
                detail: "完整性级别 SID 没有子权限".to_string(),
            });
        }
        let rid =
            unsafe { GetSidSubAuthority(label.Label.Sid, u32::from((*count).saturating_sub(1))) };
        if rid.is_null() {
            return Err(WindowTargetError::ApiFailure {
                operation,
                code: 0,
                detail: "无法读取完整性级别 RID".to_string(),
            });
        }
        Ok(unsafe { *rid })
    }

    fn read_window_class(raw: HWND) -> Result<String, WindowTargetError> {
        let mut buffer = [0u16; 256];
        let length = unsafe { GetClassNameW(raw, &mut buffer) };
        if length <= 0 {
            let handle = window_handle(raw);
            if !window_is_alive(handle) {
                return Err(WindowTargetError::WindowUnavailable { handle });
            }
            return Err(windows_api_error(
                "读取窗口类名",
                None,
                WindowsError::from_win32(),
            ));
        }
        Ok(String::from_utf16_lossy(&buffer[..length as usize]))
    }

    fn read_window_title(raw: HWND, handle: WindowHandle) -> Result<String, WindowTargetError> {
        let length = unsafe { GetWindowTextLengthW(raw) };
        if length <= 0 {
            ensure_alive(raw, handle)?;
            return Ok(String::new());
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = unsafe { GetWindowTextW(raw, &mut buffer) };
        if copied <= 0 {
            ensure_alive(raw, handle)?;
            return Ok(String::new());
        }
        Ok(String::from_utf16_lossy(&buffer[..copied as usize]))
    }

    fn read_geometry(raw: HWND, handle: WindowHandle) -> Result<WindowGeometry, WindowTargetError> {
        ensure_alive(raw, handle)?;
        let mut client_rect = RECT::default();
        unsafe { GetClientRect(raw, &mut client_rect) }
            .map_err(|error| windows_api_error("读取窗口客户区尺寸", None, error))?;
        let width = client_rect.right.checked_sub(client_rect.left);
        let height = client_rect.bottom.checked_sub(client_rect.top);
        let (Some(width), Some(height)) = (width, height) else {
            return Err(WindowTargetError::InvalidGeometry { handle });
        };
        let client_size = checked_client_size(handle, width, height)?;

        let mut client_origin = POINT { x: 0, y: 0 };
        if !unsafe { ClientToScreen(raw, &mut client_origin).as_bool() } {
            ensure_alive(raw, handle)?;
            return Err(windows_api_error(
                "读取窗口客户区位置",
                None,
                WindowsError::from_win32(),
            ));
        }
        let dpi = unsafe { GetDpiForWindow(raw) };
        if dpi == 0 {
            ensure_alive(raw, handle)?;
            return Err(WindowTargetError::ApiFailure {
                operation: "读取窗口 DPI",
                code: WindowsError::from_win32().code().0,
                detail: "系统返回了无效 DPI".to_string(),
            });
        }

        Ok(WindowGeometry {
            client_origin: ScreenPoint {
                x: client_origin.x,
                y: client_origin.y,
            },
            client_size,
            dpi,
        })
    }

    fn ensure_alive(raw: HWND, handle: WindowHandle) -> Result<(), WindowTargetError> {
        if unsafe { IsWindow(raw).as_bool() } {
            Ok(())
        } else {
            Err(WindowTargetError::WindowUnavailable { handle })
        }
    }

    fn windows_api_error(
        operation: &'static str,
        process_id: Option<u32>,
        error: WindowsError,
    ) -> WindowTargetError {
        if error.code() == HRESULT::from_win32(ERROR_ACCESS_DENIED.0) {
            WindowTargetError::AccessDenied {
                operation,
                process_id,
            }
        } else {
            WindowTargetError::ApiFailure {
                operation,
                code: error.code().0,
                detail: error.to_string(),
            }
        }
    }

    fn hwnd(handle: WindowHandle) -> HWND {
        HWND(handle.raw() as *mut c_void)
    }

    fn window_handle(raw: HWND) -> WindowHandle {
        WindowHandle::from_raw(raw.0 as usize)
    }

    struct OwnedProcessHandle(HANDLE);

    impl Drop for OwnedProcessHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    struct OwnedTokenHandle(HANDLE);

    impl Drop for OwnedTokenHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub(super) fn window_from_screen_point(
        _point: ScreenPoint,
    ) -> Result<Option<WindowSnapshot>, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn snapshot_window(
        _handle: WindowHandle,
    ) -> Result<WindowSnapshot, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn normalize_window_handle(
        _handle: WindowHandle,
    ) -> Result<Option<WindowHandle>, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn snapshot_pointed_window(
        _handle: WindowHandle,
    ) -> Result<Option<WindowSnapshot>, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn snapshot_foreground_window(
        _handle: WindowHandle,
    ) -> Result<Option<WindowSnapshot>, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn foreground_window() -> Result<Option<WindowSnapshot>, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn enumerate_target_windows() -> Result<Vec<WindowSnapshot>, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn enumerate_matching_windows(
        _target: &WindowTarget,
    ) -> Result<Vec<WindowSnapshot>, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn refresh_window_geometry(
        _handle: WindowHandle,
    ) -> Result<WindowGeometry, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn window_display_state(
        _handle: WindowHandle,
    ) -> Result<WindowDisplayState, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn window_is_alive(_handle: WindowHandle) -> bool {
        false
    }

    pub(super) fn restore_and_activate_window(
        _handle: WindowHandle,
    ) -> Result<(), WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }

    pub(super) fn virtual_desktop_bounds() -> Result<ScreenRect, WindowTargetError> {
        Err(WindowTargetError::UnsupportedPlatform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(title: &str) -> WindowTarget {
        WindowTarget {
            id: TargetWindowId(7),
            executable_path: r"C:\Program Files\Example\example.exe".to_string(),
            window_class: "ExampleWindow".to_string(),
            title: title.to_string(),
            client_size: ClientSize {
                width: 800,
                height: 600,
            },
            dpi: 96,
            availability: TargetWindowAvailability::Initial,
        }
    }

    fn snapshot(
        raw: usize,
        executable_path: &str,
        window_class: &str,
        title: &str,
    ) -> WindowSnapshot {
        WindowSnapshot {
            handle: WindowHandle::from_raw(raw),
            executable_path: executable_path.to_string(),
            window_class: window_class.to_string(),
            title: title.to_string(),
            client_origin: ScreenPoint { x: 10, y: 20 },
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
    fn identity_match_uses_windows_path_case_rules_but_exact_class() {
        let recorded = target("Recorded title");
        let renamed = snapshot(
            1,
            &recorded.executable_path,
            &recorded.window_class,
            "A later title",
        );
        assert!(matches_recorded_target(&recorded, &renamed));

        let different_case_path = snapshot(
            2,
            r"C:\PROGRAM FILES\Example\example.exe",
            &recorded.window_class,
            &recorded.title,
        );
        assert!(matches_recorded_target(&recorded, &different_case_path));

        let unicode_case_path = snapshot(
            3,
            r"C:\用户\ÄPP\example.exe",
            &recorded.window_class,
            &recorded.title,
        );
        let mut unicode_target = recorded.clone();
        unicode_target.executable_path = r"c:\用户\äpp\EXAMPLE.EXE".to_string();
        assert!(matches_recorded_target(&unicode_target, &unicode_case_path));

        let different_class = snapshot(
            4,
            &recorded.executable_path,
            "examplewindow",
            &recorded.title,
        );
        assert!(!matches_recorded_target(&recorded, &different_class));
    }

    #[test]
    fn title_only_orders_candidates_and_never_filters_them() {
        let recorded = target("Quarterly report");
        let mut candidates = vec![
            snapshot(
                3,
                &recorded.executable_path,
                &recorded.window_class,
                "Unrelated",
            ),
            snapshot(
                2,
                &recorded.executable_path,
                &recorded.window_class,
                "Quarterly report - edited",
            ),
            snapshot(
                1,
                &recorded.executable_path,
                &recorded.window_class,
                "Quarterly report",
            ),
        ];

        sort_candidates_for_target(&recorded, &mut candidates);

        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].handle, WindowHandle::from_raw(1));
        assert_eq!(candidates[1].handle, WindowHandle::from_raw(2));
        assert_eq!(candidates[2].handle, WindowHandle::from_raw(3));
    }

    #[test]
    fn shell_tooltip_and_transient_classes_are_distinguished() {
        assert!(is_system_surface_class("Progman"));
        assert!(is_system_surface_class("Shell_TrayWnd"));
        assert!(is_tooltip_class("tooltips_class32"));
        assert!(is_owned_transient_class("#32768"));
        assert!(is_owned_transient_class("ComboLBox"));
        assert!(!is_system_surface_class("#32770"));
    }

    #[test]
    fn shell_host_processes_are_system_surfaces_without_excluding_explorer_windows() {
        let start_menu = snapshot(
            1,
            r"C:\Windows\SystemApps\StartMenuExperienceHost.exe",
            "Windows.UI.Core.CoreWindow",
            "Start",
        );
        assert!(is_system_surface(&start_menu));

        let explorer_window = snapshot(2, r"C:\Windows\explorer.exe", "CabinetWClass", "Documents");
        assert!(!is_system_surface(&explorer_window));
    }

    #[test]
    fn initial_targets_exclude_hidden_and_shell_host_windows() {
        let normal = snapshot(1, r"C:\Example\example.exe", "ExampleWindow", "Example");
        assert!(is_initial_target(&normal));

        let mut hidden = normal.clone();
        hidden.visible = false;
        assert!(!is_initial_target(&hidden));

        let shell = snapshot(
            2,
            r"C:\Windows\SystemApps\ShellExperienceHost.exe",
            "Windows.UI.Core.CoreWindow",
            "Action center",
        );
        assert!(!is_initial_target(&shell));
    }

    #[test]
    fn access_denied_message_recommends_administrator_mode() {
        let message = WindowTargetError::AccessDenied {
            operation: "读取窗口程序路径",
            process_id: Some(9001),
        }
        .to_string();

        assert!(message.contains("PID 9001"));
        assert!(message.contains("访问被拒绝"));
        assert!(message.contains("管理员身份"));
    }

    #[test]
    fn integrity_comparison_allows_equal_or_lower_targets_only() {
        assert!(target_integrity_is_readable(0x2000, 0x1000));
        assert!(target_integrity_is_readable(0x2000, 0x2000));
        assert!(!target_integrity_is_readable(0x2000, 0x3000));
    }

    #[test]
    fn higher_integrity_message_explains_the_administrator_remedy() {
        let message = WindowTargetError::HigherIntegrityTarget {
            process_id: 9001,
            current_rid: 0x2000,
            target_rid: 0x3000,
        }
        .to_string();

        assert!(message.contains("PID 9001"));
        assert!(message.contains("完整性级别"));
        assert!(message.contains("管理员身份"));
    }

    #[test]
    fn virtual_desktop_bounds_include_negative_monitors_and_exclude_far_edge() {
        let bounds = ScreenRect {
            left: -1920,
            top: -200,
            right: 2560,
            bottom: 1440,
        };

        assert!(bounds.contains(ScreenPoint { x: -1920, y: -200 }));
        assert!(bounds.contains(ScreenPoint { x: 2559, y: 1439 }));
        assert!(!bounds.contains(ScreenPoint { x: 2560, y: 1439 }));
        assert!(!bounds.contains(ScreenPoint { x: 0, y: 1440 }));
    }

    #[test]
    fn relative_coordinate_conversion_detects_overflow() {
        let handle = WindowHandle::from_raw(5);
        assert_eq!(
            relative_to_screen(handle, ScreenPoint { x: -100, y: 50 }, 25, -10),
            Ok(ScreenPoint { x: -75, y: 40 })
        );
        assert!(matches!(
            relative_to_screen(handle, ScreenPoint { x: i32::MAX, y: 0 }, 1, 0),
            Err(WindowTargetError::CoordinateOverflow { .. })
        ));
    }

    #[test]
    fn zero_sized_client_areas_are_invalid_geometry() {
        let handle = WindowHandle::from_raw(6);
        assert_eq!(
            checked_client_size(handle, 800, 600),
            Ok(ClientSize {
                width: 800,
                height: 600
            })
        );
        assert_eq!(
            checked_client_size(handle, 0, 600),
            Err(WindowTargetError::InvalidGeometry { handle })
        );
        assert_eq!(
            checked_client_size(handle, 800, 0),
            Err(WindowTargetError::InvalidGeometry { handle })
        );
    }

    #[test]
    fn async_show_readiness_uses_observed_window_state() {
        assert!(display_state_ready(true, false));
        assert!(!display_state_ready(false, false));
        assert!(!display_state_ready(true, true));
        assert!(!display_state_ready(false, true));
    }

    #[test]
    fn snapshot_converts_to_recording_target_without_screen_origin() {
        let snapshot = snapshot(9, r"C:\Example\example.exe", "ExampleWindow", "Example");
        let target = snapshot.to_target(TargetWindowId(4), TargetWindowAvailability::Deferred);

        assert_eq!(target.id, TargetWindowId(4));
        assert_eq!(target.executable_path, snapshot.executable_path);
        assert_eq!(target.window_class, snapshot.window_class);
        assert_eq!(target.title, snapshot.title);
        assert_eq!(target.client_size, snapshot.client_size);
        assert_eq!(target.dpi, snapshot.dpi);
        assert_eq!(target.availability, TargetWindowAvailability::Deferred);
    }
}
