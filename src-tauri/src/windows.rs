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

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

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
        fn GetForegroundWindow() -> Handle;
        fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
        fn GetWindowTextW(hwnd: Handle, text: *mut u16, max_count: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: Handle, process_id: *mut u32) -> u32;
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
        target_from_hwnd(hwnd)
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

fn unknown_target_window() -> TargetWindow {
    TargetWindow {
        title: "尚未捕获活动窗口".to_string(),
        process: "N/A".to_string(),
        size: "N/A".to_string(),
        matched: false,
    }
}
