use crate::app_state::AppMode;
use tauri::{AppHandle, Manager, PhysicalPosition};

const WINDOW_LABEL: &str = "activity-indicator";
const SCREEN_MARGIN_PX: i32 = 12;

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| "activity indicator window is unavailable".to_string())?;

    window
        .set_ignore_cursor_events(true)
        .map_err(|error| error.to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        };

        let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) } + SCREEN_MARGIN_PX;
        let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) } + SCREEN_MARGIN_PX;
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|error| error.to_string())?;
    }

    window.hide().map_err(|error| error.to_string())
}

pub fn sync(app: &AppHandle, mode: AppMode, enabled: bool) -> Result<(), String> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| "activity indicator window is unavailable".to_string())?;

    if enabled && should_show(mode) {
        window.show()
    } else {
        window.hide()
    }
    .map_err(|error| error.to_string())
}

fn should_show(mode: AppMode) -> bool {
    matches!(mode, AppMode::Recording | AppMode::Playing)
}

#[cfg(test)]
mod tests {
    use super::should_show;
    use crate::app_state::AppMode;

    #[test]
    fn indicator_is_visible_only_while_recording_or_playing() {
        assert!(!should_show(AppMode::Idle));
        assert!(should_show(AppMode::Recording));
        assert!(should_show(AppMode::Playing));
    }
}
