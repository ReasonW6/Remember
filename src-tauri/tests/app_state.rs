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

#[test]
fn rejects_playback_reentry_and_preserves_stop_token() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");
    app.start_playback(1, 1.0).expect("play");
    let token = app.stop_token();

    let error = app.start_playback(1, 1.0).expect_err("already playing");
    app.stop_playback();

    assert!(error.contains("cannot play while playing"));
    assert!(token.is_stopped());
    assert_eq!(app.mode(), AppMode::Idle);
}

#[test]
fn rejects_start_recording_unless_idle() {
    let mut recording_app = AppController::new();
    recording_app
        .start_recording("active", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    let error = recording_app
        .start_recording("again", 101, "2026-06-29T00:00:01Z")
        .expect_err("record while recording");

    assert!(error.contains("cannot record while recording"));
    assert_eq!(recording_app.mode(), AppMode::Recording);

    let mut playing_app = AppController::new();
    playing_app.set_recording(recording()).expect("load");
    playing_app.start_playback(1, 1.0).expect("play");

    let error = playing_app
        .start_recording("again", 101, "2026-06-29T00:00:01Z")
        .expect_err("record while playing");

    assert!(error.contains("cannot record while playing"));
    assert_eq!(playing_app.mode(), AppMode::Playing);
}

#[test]
fn rejects_loading_recording_while_recording() {
    let mut app = AppController::new();
    app.start_recording("active", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    let error = app
        .set_recording(recording())
        .expect_err("load while recording");

    assert!(error.contains("cannot load recording while recording"));
    assert_eq!(app.mode(), AppMode::Recording);
    assert!(app.current_recording().is_none());
}

#[test]
fn rejects_loading_recording_while_playing() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");
    app.start_playback(1, 1.0).expect("play");

    let error = app
        .set_recording(Recording::new(
            "replacement",
            "2026-06-29T00:00:01Z",
            Vec::new(),
        ))
        .expect_err("load while playing");

    assert!(error.contains("cannot load recording while playing"));
    assert_eq!(app.mode(), AppMode::Playing);
    assert_eq!(app.current_recording().expect("recording").name, "loaded");
}

#[test]
fn stop_playback_while_recording_leaves_recording_active() {
    let mut app = AppController::new();
    app.start_recording("active", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    app.stop_playback();

    assert_eq!(app.mode(), AppMode::Recording);
    let saved = app.stop_recording(150).expect("stop recording");
    assert_eq!(saved.name, "active");
    assert_eq!(app.mode(), AppMode::Idle);
}

#[test]
fn ui_state_tracks_mode_message_and_recording_summary() {
    let mut app = AppController::new();

    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Idle);
    assert_eq!(state.recording_name, None);
    assert_eq!(state.step_count, 0);
    assert_eq!(state.duration_ms, 0);
    assert_eq!(state.message, "Idle");

    app.set_recording(recording()).expect("load");
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Idle);
    assert_eq!(state.recording_name.as_deref(), Some("loaded"));
    assert_eq!(state.step_count, 1);
    assert_eq!(state.duration_ms, 1);
    assert_eq!(state.message, "Recording loaded");

    app.start_recording("active", 100, "2026-06-29T00:00:00Z")
        .expect("start");
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Recording);
    assert_eq!(state.recording_name, None);
    assert_eq!(state.step_count, 0);
    assert_eq!(state.duration_ms, 0);
    assert_eq!(state.message, "Recording");

    app.stop_recording(150).expect("stop");
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Idle);
    assert_eq!(state.recording_name.as_deref(), Some("active"));
    assert_eq!(state.step_count, 0);
    assert_eq!(state.duration_ms, 50);
    assert_eq!(state.message, "Recording stopped");

    app.start_playback(1, 1.0).expect("play");
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Playing);
    assert_eq!(state.recording_name.as_deref(), Some("active"));
    assert_eq!(state.message, "Playing");

    app.stop_playback();
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Idle);
    assert_eq!(state.recording_name.as_deref(), Some("active"));
    assert_eq!(state.message, "Playback stopped");
}
