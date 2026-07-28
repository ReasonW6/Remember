use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PrivilegeState {
    pub is_elevated: bool,
}

pub fn state() -> Result<PrivilegeState, String> {
    Ok(PrivilegeState {
        is_elevated: is_elevated()?,
    })
}

#[cfg(target_os = "windows")]
pub fn restart_as_administrator(app: &AppHandle) -> Result<(), String> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::HWND,
            UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        },
    };

    if is_elevated()? {
        return Err("already running as administrator".to_string());
    }

    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let operation: Vec<u16> = OsStr::new("runas")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let executable: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let parameters: Vec<u16> = OsStr::new(crate::single_instance::ELEVATED_RESTART_ARG)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let parent_address = app
        .get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .map(|handle| handle.0 as usize)
        .unwrap_or_default();
    let parent = HWND(parent_address as *mut std::ffi::c_void);

    let result = unsafe {
        ShellExecuteW(
            parent,
            PCWSTR(operation.as_ptr()),
            PCWSTR(executable.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as usize <= 32 {
        return Err("administrator restart was cancelled or failed".to_string());
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn restart_as_administrator(_app: &AppHandle) -> Result<(), String> {
    Err("administrator restart is Windows-only".to_string())
}

#[cfg(target_os = "windows")]
fn is_elevated() -> Result<bool, String> {
    use std::mem::size_of;
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|error| error.to_string())?;
    }

    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned_length = 0;
    let result = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned_length,
        )
    };
    unsafe {
        let _ = CloseHandle(token);
    }
    result.map_err(|error| error.to_string())?;

    Ok(elevation.TokenIsElevated != 0)
}

#[cfg(not(target_os = "windows"))]
fn is_elevated() -> Result<bool, String> {
    Ok(false)
}
