use crate::settings_bundle::{MainWindowPosition, MainWindowPreferences};
use tauri::{LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const MAIN_WINDOW_LABEL: &str = "main";
const COMPACT_WIDTH: f64 = 360.0;
const COMPACT_HEIGHT: f64 = 134.0;
const EXPANDED_WIDTH: f64 = 420.0;
const EXPANDED_HEIGHT: f64 = 520.0;
const MIN_VISIBLE_WIDTH: i32 = 48;
const MIN_VISIBLE_HEIGHT: i32 = 32;
const FALLBACK_MARGIN: i32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorBounds {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

pub fn restore(app: &tauri::AppHandle, preferences: MainWindowPreferences) -> Result<(), String> {
    let window = main_window(app)?;
    set_compact_size(&window, preferences.compact)?;
    if let Some(saved_position) = preferences.position {
        let window_size = window.outer_size().map_err(|error| error.to_string())?;
        let monitors = window
            .available_monitors()
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|monitor| MonitorBounds {
                position: *monitor.position(),
                size: *monitor.size(),
            })
            .collect::<Vec<_>>();
        let primary = window
            .primary_monitor()
            .map_err(|error| error.to_string())?
            .map(|monitor| MonitorBounds {
                position: *monitor.position(),
                size: *monitor.size(),
            });
        let position = visible_or_fallback_position(
            PhysicalPosition::new(saved_position.x, saved_position.y),
            window_size,
            &monitors,
            primary,
        );
        window
            .set_position(position)
            .map_err(|error| error.to_string())?;
    }
    window.show().map_err(|error| error.to_string())
}

pub fn set_compact(app: &tauri::AppHandle, compact: bool) -> Result<(), String> {
    set_compact_size(&main_window(app)?, compact)
}

pub fn current_position(app: &tauri::AppHandle) -> Result<MainWindowPosition, String> {
    let position = main_window(app)?
        .outer_position()
        .map_err(|error| error.to_string())?;
    Ok(MainWindowPosition {
        x: position.x,
        y: position.y,
    })
}

fn main_window(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "主窗口不可用。".to_string())
}

fn set_compact_size(window: &WebviewWindow, compact: bool) -> Result<(), String> {
    let (width, height) = if compact {
        (COMPACT_WIDTH, COMPACT_HEIGHT)
    } else {
        (EXPANDED_WIDTH, EXPANDED_HEIGHT)
    };
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|error| error.to_string())
}

fn visible_or_fallback_position(
    saved: PhysicalPosition<i32>,
    window_size: PhysicalSize<u32>,
    monitors: &[MonitorBounds],
    primary: Option<MonitorBounds>,
) -> PhysicalPosition<i32> {
    if monitors
        .iter()
        .any(|monitor| visible_overlap(saved, window_size, *monitor))
    {
        return saved;
    }

    primary
        .or_else(|| monitors.first().copied())
        .map(|monitor| {
            PhysicalPosition::new(
                monitor.position.x.saturating_add(FALLBACK_MARGIN),
                monitor.position.y.saturating_add(FALLBACK_MARGIN),
            )
        })
        .unwrap_or(saved)
}

fn visible_overlap(
    position: PhysicalPosition<i32>,
    window_size: PhysicalSize<u32>,
    monitor: MonitorBounds,
) -> bool {
    let window_right = i64::from(position.x) + i64::from(window_size.width);
    let window_bottom = i64::from(position.y) + i64::from(window_size.height);
    let monitor_right = i64::from(monitor.position.x) + i64::from(monitor.size.width);
    let monitor_bottom = i64::from(monitor.position.y) + i64::from(monitor.size.height);
    let overlap_width =
        window_right.min(monitor_right) - i64::from(position.x.max(monitor.position.x));
    let overlap_height =
        window_bottom.min(monitor_bottom) - i64::from(position.y.max(monitor.position.y));
    overlap_width >= i64::from(MIN_VISIBLE_WIDTH) && overlap_height >= i64::from(MIN_VISIBLE_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, width: u32, height: u32) -> MonitorBounds {
        MonitorBounds {
            position: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(width, height),
        }
    }

    #[test]
    fn keeps_a_saved_position_that_is_visible_on_a_negative_monitor() {
        let monitors = [monitor(-1920, 0, 1920, 1080), monitor(0, 0, 2560, 1440)];
        assert_eq!(
            visible_or_fallback_position(
                PhysicalPosition::new(-1800, 120),
                PhysicalSize::new(420, 520),
                &monitors,
                Some(monitors[1]),
            ),
            PhysicalPosition::new(-1800, 120)
        );
    }

    #[test]
    fn moves_an_offscreen_saved_position_to_the_primary_monitor() {
        let monitors = [monitor(0, 0, 2560, 1440)];
        assert_eq!(
            visible_or_fallback_position(
                PhysicalPosition::new(-32000, -32000),
                PhysicalSize::new(360, 134),
                &monitors,
                Some(monitors[0]),
            ),
            PhysicalPosition::new(32, 32)
        );
    }

    #[test]
    fn preserves_a_partly_offscreen_window_with_a_visible_area() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(
            visible_or_fallback_position(
                PhysicalPosition::new(1870, 1040),
                PhysicalSize::new(360, 134),
                &monitors,
                Some(monitors[0]),
            ),
            PhysicalPosition::new(1870, 1040)
        );
    }
}
