use crate::app_state::AppMode;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::thread;
use tauri::{
    webview::PageLoadEvent, AppHandle, Manager, PhysicalPosition, WebviewWindow,
    WebviewWindowBuilder,
};

const WINDOW_LABEL: &str = "activity-indicator";
const SCREEN_MARGIN_PX: i32 = 12;
static CREATE_LOCK: Mutex<()> = Mutex::new(());
static PAGE_READY: AtomicBool = AtomicBool::new(false);
static CONFIGURED: AtomicBool = AtomicBool::new(false);
static SHOULD_BE_VISIBLE: AtomicBool = AtomicBool::new(false);
static CREATION_SCHEDULED: AtomicBool = AtomicBool::new(false);

pub fn setup(app: &AppHandle) -> Result<(), String> {
    PAGE_READY.store(false, Ordering::Release);
    CONFIGURED.store(false, Ordering::Release);
    SHOULD_BE_VISIBLE.store(false, Ordering::Release);
    CREATION_SCHEDULED.store(false, Ordering::Release);

    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        configure_window(&window)?;
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn configure_window(window: &WebviewWindow) -> Result<(), String> {
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

    CONFIGURED.store(true, Ordering::Release);
    Ok(())
}

pub fn sync(app: &AppHandle, mode: AppMode, enabled: bool) -> Result<(), String> {
    let visible = enabled && should_show(mode);
    SHOULD_BE_VISIBLE.store(visible, Ordering::Release);

    if !visible {
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            window.hide().map_err(|error| error.to_string())?;
        }
        return Ok(());
    }

    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        if !CONFIGURED.load(Ordering::Acquire) {
            configure_window(&window)?;
        }
        return show_when_ready(&window);
    }

    schedule_window_creation(app)
}

fn schedule_window_creation(app: &AppHandle) -> Result<(), String> {
    if CREATION_SCHEDULED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }

    let app_for_thread = app.clone();
    let spawn_result = thread::Builder::new()
        .name("remember-indicator-create".to_string())
        .spawn(move || {
            let app_for_main = app_for_thread.clone();
            let scheduling_result = app_for_thread.run_on_main_thread(move || {
                if SHOULD_BE_VISIBLE.load(Ordering::Acquire) {
                    if let Err(error) = ensure_window(&app_for_main) {
                        eprintln!("Remember activity indicator could not be created: {error}");
                    }
                }
                CREATION_SCHEDULED.store(false, Ordering::Release);
            });
            if let Err(error) = scheduling_result {
                CREATION_SCHEDULED.store(false, Ordering::Release);
                eprintln!("Remember activity indicator could not be scheduled: {error}");
            }
        });
    if let Err(error) = spawn_result {
        CREATION_SCHEDULED.store(false, Ordering::Release);
        return Err(error.to_string());
    }
    Ok(())
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
        .map_err(|_| "activity indicator creation lock poisoned".to_string())?;
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
        .ok_or_else(|| "activity indicator window configuration is unavailable".to_string())?;
    let window = WebviewWindowBuilder::from_config(app, &config)
        .map_err(|error| error.to_string())?
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                PAGE_READY.store(true, Ordering::Release);
                if let Err(error) = show_when_ready(&window) {
                    eprintln!("Remember activity indicator could not become visible: {error}");
                }
            }
        })
        .build()
        .map_err(|error| error.to_string())?;
    configure_window(&window)?;
    Ok(window)
}

fn show_when_ready(window: &WebviewWindow) -> Result<(), String> {
    if PAGE_READY.load(Ordering::Acquire)
        && CONFIGURED.load(Ordering::Acquire)
        && SHOULD_BE_VISIBLE.load(Ordering::Acquire)
    {
        window.show().map_err(|error| error.to_string())?;
    }
    Ok(())
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
