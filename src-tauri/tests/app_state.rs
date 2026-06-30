use remember_lib::app_state::{AppController, AppMode};
use remember_lib::model::{KeyState, MacroStep, Recording};

fn recording() -> Recording {
    Recording::new(
        "loaded",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::Key {
            elapsed_ms: 1,
            vk_code: 0x41,
            scan_code: 0x1E,
            state: KeyState::Pressed,
        }],
    )
}

#[test]
fn starts_and_stops_recording() {
    let mut app = AppController::new();

    app.start_recording("test", 100, "2026-06-29T00:00:00Z")
        .expect("start");
    assert_eq!(app.mode(), AppMode::Recording);

    let saved = app.stop_recording(150).expect("stop");
    assert_eq!(app.mode(), AppMode::Idle);
    assert_eq!(saved.name, "test");
}

#[test]
fn rejects_play_without_recording() {
    let mut app = AppController::new();

    let error = app.start_playback(1, 1.0).expect_err("no recording");

    assert!(error.contains("no recording loaded"));
}

#[test]
fn loads_recording_and_starts_playback() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");

    let plan = app.start_playback(2, 1.0).expect("play");

    assert_eq!(app.mode(), AppMode::Playing);
    assert_eq!(plan.len(), 2);
}

#[test]
fn stop_playback_returns_to_idle() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");
    app.start_playback(1, 1.0).expect("play");

    app.stop_playback();

    assert_eq!(app.mode(), AppMode::Idle);
}
