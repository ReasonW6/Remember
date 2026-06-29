use crate::storage::TargetWindow;

#[cfg(target_os = "windows")]
mod platform {
    use super::unknown_target_window;
    use crate::storage::TargetWindow;
    use std::{
        ffi::{c_void, OsString},
        os::windows::ffi::OsStringExt,
        path::Path,
    };

    type Handle = *mut c_void;
    type EnumWindowsProc = Option<unsafe extern "system" fn(hwnd: Handle, lparam: isize) -> i32>;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const GA_ROOT: u32 = 2;
    const SW_RESTORE: i32 = 9;

    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(callback: EnumWindowsProc, lparam: isize) -> i32;
        fn GetForegroundWindow() -> Handle;
        fn GetAncestor(hwnd: Handle, flags: u32) -> Handle;
        fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
        fn GetWindowTextW(hwnd: Handle, text: *mut u16, max_count: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: Handle, process_id: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: Handle) -> i32;
        fn SetForegroundWindow(hwnd: Handle) -> i32;
        fn ShowWindow(hwnd: Handle, command_show: i32) -> i32;
        fn WindowFromPoint(point: Point) -> Handle;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CloseHandle(handle: Handle) -> i32;
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            exe_name: *mut u16,
            size: *mut u32,
        ) -> i32;
    }

    pub fn active_window_target() -> TargetWindow {
        let hwnd = unsafe { GetForegroundWindow() };
        target_from_hwnd(hwnd)
    }

    pub fn target_window_at_point(x: i32, y: i32) -> TargetWindow {
        let hwnd = unsafe { WindowFromPoint(Point { x, y }) };
        target_from_hwnd(top_level_window(hwnd))
    }

    pub fn focus_target_window(target: &TargetWindow) -> Result<(), String> {
        if !has_known_process(&target.process) {
            return Err("录制流程缺少可验证的目标进程".to_string());
        }

        let mut request = TargetWindowRequest {
            target,
            hwnd: std::ptr::null_mut(),
        };
        unsafe {
            EnumWindows(
                Some(enum_target_window),
                &mut request as *mut TargetWindowRequest as isize,
            );
        }

        if request.hwnd.is_null() {
            return Err(format!(
                "未找到录制目标窗口：{} / {}",
                target.process, target.title
            ));
        }

        unsafe {
            ShowWindow(request.hwnd, SW_RESTORE);
            if SetForegroundWindow(request.hwnd) == 0 {
                return Err(format!(
                    "系统拒绝切换到录制目标窗口：{} / {}",
                    target.process, target.title
                ));
            }
        }

        Ok(())
    }

    struct TargetWindowRequest<'a> {
        target: &'a TargetWindow,
        hwnd: Handle,
    }

    unsafe extern "system" fn enum_target_window(hwnd: Handle, lparam: isize) -> i32 {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }

        let request = &mut *(lparam as *mut TargetWindowRequest);
        let candidate = target_from_hwnd(hwnd);
        if target_window_matches(request.target, &candidate) {
            request.hwnd = hwnd;
            return 0;
        }

        1
    }

    fn target_window_matches(expected: &TargetWindow, candidate: &TargetWindow) -> bool {
        if !same_process(&expected.process, &candidate.process) {
            return false;
        }

        if has_known_title(&expected.title) && has_known_title(&candidate.title) {
            return same_title(&expected.title, &candidate.title);
        }

        true
    }

    fn top_level_window(hwnd: Handle) -> Handle {
        if hwnd.is_null() {
            return hwnd;
        }

        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        if root.is_null() {
            hwnd
        } else {
            root
        }
    }

    fn target_from_hwnd(hwnd: Handle) -> TargetWindow {
        if hwnd.is_null() {
            return unknown_target_window();
        }

        let title = window_title(hwnd);
        let process = process_name(hwnd);
        let size = window_size(hwnd);
        let matched = !title.is_empty() || process != "N/A";

        TargetWindow {
            title: if title.is_empty() {
                "未知活动窗口".to_string()
            } else {
                title
            },
            process,
            size,
            matched,
        }
    }

    fn window_title(hwnd: Handle) -> String {
        let mut buffer = [0u16; 512];
        let length =
            unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) }.max(0);
        String::from_utf16_lossy(&buffer[..length as usize])
            .trim()
            .to_string()
    }

    fn process_name(hwnd: Handle) -> String {
        let mut process_id = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut process_id) };
        if process_id == 0 {
            return "N/A".to_string();
        }

        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        if process.is_null() {
            return format!("PID {process_id}");
        }

        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let ok =
            unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
        unsafe { CloseHandle(process) };

        if ok == 0 || length == 0 {
            return format!("PID {process_id}");
        }

        let full_path = OsString::from_wide(&buffer[..length as usize])
            .to_string_lossy()
            .to_string();
        Path::new(&full_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&full_path)
            .to_string()
    }

    fn has_known_process(process: &str) -> bool {
        let process = process.trim();
        !process.is_empty() && process != "N/A" && !process.starts_with("PID ")
    }

    fn same_process(left: &str, right: &str) -> bool {
        left.trim().eq_ignore_ascii_case(right.trim())
    }

    fn has_known_title(title: &str) -> bool {
        let title = title.trim();
        !title.is_empty()
            && title != "N/A"
            && title != "未知活动窗口"
            && title != "尚未捕获活动窗口"
            && !is_unstable_child_window_title(title)
    }

    fn is_unstable_child_window_title(title: &str) -> bool {
        title.eq_ignore_ascii_case("Chrome Legacy Window")
    }

    fn same_title(left: &str, right: &str) -> bool {
        left.trim().eq_ignore_ascii_case(right.trim())
    }

    fn window_size(hwnd: Handle) -> String {
        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let ok = unsafe { GetWindowRect(hwnd, &mut rect) };
        if ok == 0 {
            return "N/A".to_string();
        }

        let width = (rect.right - rect.left).max(0);
        let height = (rect.bottom - rect.top).max(0);
        format!("{width} x {height}")
    }
}

pub fn active_window_target() -> TargetWindow {
    #[cfg(target_os = "windows")]
    {
        platform::active_window_target()
    }

    #[cfg(not(target_os = "windows"))]
    {
        unknown_target_window()
    }
}

pub fn target_window_at_point(x: i32, y: i32) -> TargetWindow {
    #[cfg(target_os = "windows")]
    {
        platform::target_window_at_point(x, y)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (x, y);
        unknown_target_window()
    }
}

pub fn focus_target_window(target: &TargetWindow) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        platform::focus_target_window(target)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = target;
        Err("目标窗口切换只支持 Windows".to_string())
    }
}

fn unknown_target_window() -> TargetWindow {
    TargetWindow {
        title: "尚未捕获活动窗口".to_string(),
        process: "N/A".to_string(),
        size: "N/A".to_string(),
        matched: false,
    }
}
