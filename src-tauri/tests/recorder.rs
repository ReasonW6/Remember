use remember_lib::recorder::{RecordedMouseButton, RecorderState, ScreenRect};
use remember_lib::storage::{FlowStep, TargetWindow};

#[test]
fn start_recording_opens_one_active_session() {
    let mut recorder = RecorderState::default();

    let first = recorder.start().expect("first recording should start");

    assert!(recorder.is_recording());
    assert!(first.started_at > 0);
    assert_eq!(first.label, "录制中");
    assert!(first.safety_warning.contains("密码"));
    assert!(first.safety_warning.contains("敏感"));

    let second = recorder.start();
    assert!(second.is_err());
}

#[test]
fn stop_recording_uses_session_target_window_metadata() {
    let mut recorder = RecorderState::default();
    let target_window = TargetWindow {
        title: "Quarterly Report - Notepad".to_string(),
        process: "notepad.exe".to_string(),
        size: "1024 x 768".to_string(),
        matched: true,
    };

    recorder
        .start_with_target_window(target_window.clone())
        .expect("recording should start with target metadata");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.target_window, target_window);
}

#[test]
fn stop_recording_uses_first_recorded_action_target_window_metadata() {
    let mut recorder = RecorderState::default();
    let control_window = TargetWindow {
        title: "Remember".to_string(),
        process: "remember.exe".to_string(),
        size: "536 x 209".to_string(),
        matched: true,
    };
    let target_window = TargetWindow {
        title: "remember-acceptance-smoke.txt - Notepad".to_string(),
        process: "Notepad.exe".to_string(),
        size: "830 x 920".to_string(),
        matched: true,
    };

    recorder
        .start_with_target_window(control_window)
        .expect("recording should start from the control window");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_mouse_click_at_target(
            500,
            500,
            RecordedMouseButton::Left,
            started_at_ms + 300,
            target_window.clone(),
        )
        .expect("target click should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.target_window, target_window);
}

#[test]
fn stop_recording_converts_mouse_clicks_to_steps_with_wait_timing() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_mouse_click_at(120, 240, RecordedMouseButton::Left, started_at_ms + 250)
        .expect("left click should be recorded");
    recorder
        .record_mouse_click_at(300, 480, RecordedMouseButton::Right, started_at_ms + 900)
        .expect("right click should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 2);
    match &stopped.flow.steps[0] {
        FlowStep::Click {
            action,
            target,
            x,
            y,
            delay_ms,
            note,
            ..
        } => {
            assert_eq!(action, "左键单击");
            assert_eq!(target, "(120, 240) [屏幕绝对]");
            assert_eq!((*x, *y), (120, 240));
            assert_eq!(*delay_ms, 250);
            assert!(note.contains("鼠标点击"));
        }
        step => panic!("expected first click step, got {step:?}"),
    }
    match &stopped.flow.steps[1] {
        FlowStep::Click {
            action,
            x,
            y,
            delay_ms,
            ..
        } => {
            assert_eq!(action, "右键单击");
            assert_eq!((*x, *y), (300, 480));
            assert_eq!(*delay_ms, 650);
        }
        step => panic!("expected second click step, got {step:?}"),
    }
}

#[test]
fn stop_recording_excludes_clicks_inside_app_windows() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_mouse_click_at(50, 50, RecordedMouseButton::Left, started_at_ms + 100)
        .expect("app click should be collected before filtering");
    recorder
        .record_mouse_click_at(500, 500, RecordedMouseButton::Left, started_at_ms + 350)
        .expect("target click should be recorded");

    let stopped = recorder
        .stop_excluding_regions(&[ScreenRect {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        }])
        .expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 1);
    match &stopped.flow.steps[0] {
        FlowStep::Click { x, y, delay_ms, .. } => {
            assert_eq!((*x, *y), (500, 500));
            assert_eq!(*delay_ms, 350);
        }
        step => panic!("expected outside click, got {step:?}"),
    }
}

#[test]
fn stop_recording_merges_text_input_and_records_hotkeys_with_wait_timing() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_text_input_at("R", started_at_ms + 100)
        .expect("first typed character should be recorded");
    recorder
        .record_text_input_at("e", started_at_ms + 180)
        .expect("second typed character should be recorded");
    recorder
        .record_text_input_at("p", started_at_ms + 260)
        .expect("third typed character should be recorded");
    recorder
        .record_hotkey_at(
            vec!["Ctrl".to_string(), "S".to_string()],
            started_at_ms + 900,
        )
        .expect("hotkey should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 2);
    match &stopped.flow.steps[0] {
        FlowStep::Type {
            action,
            text,
            delay_ms,
            note,
            ..
        } => {
            assert_eq!(action, "文本输入");
            assert_eq!(text, "Rep");
            assert_eq!(*delay_ms, 100);
            assert!(note.contains("键盘输入"));
        }
        step => panic!("expected merged type step, got {step:?}"),
    }
    match &stopped.flow.steps[1] {
        FlowStep::Hotkey {
            action,
            keys,
            delay_ms,
            note,
            ..
        } => {
            assert_eq!(action, "快捷键");
            assert_eq!(keys, &["Ctrl", "S"]);
            assert_eq!(*delay_ms, 640);
            assert!(note.contains("快捷键"));
        }
        step => panic!("expected hotkey step, got {step:?}"),
    }
}

#[test]
fn stop_recording_orders_mouse_text_and_hotkey_events_by_capture_time() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_text_input_at("H", started_at_ms + 350)
        .expect("typed character should be recorded");
    recorder
        .record_mouse_click_at(120, 240, RecordedMouseButton::Left, started_at_ms + 100)
        .expect("mouse click should be recorded");
    recorder
        .record_text_input_at("i", started_at_ms + 430)
        .expect("second typed character should be recorded");
    recorder
        .record_hotkey_at(
            vec!["Ctrl".to_string(), "Shift".to_string(), "S".to_string()],
            started_at_ms + 900,
        )
        .expect("hotkey should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 3);
    match &stopped.flow.steps[0] {
        FlowStep::Click { x, y, delay_ms, .. } => {
            assert_eq!((*x, *y), (120, 240));
            assert_eq!(*delay_ms, 100);
        }
        step => panic!("expected click step first, got {step:?}"),
    }
    match &stopped.flow.steps[1] {
        FlowStep::Type { text, delay_ms, .. } => {
            assert_eq!(text, "Hi");
            assert_eq!(*delay_ms, 250);
        }
        step => panic!("expected type step second, got {step:?}"),
    }
    match &stopped.flow.steps[2] {
        FlowStep::Hotkey { keys, delay_ms, .. } => {
            assert_eq!(keys, &["Ctrl", "Shift", "S"]);
            assert_eq!(*delay_ms, 470);
        }
        step => panic!("expected hotkey step third, got {step:?}"),
    }
}

#[test]
fn stop_recording_returns_safe_placeholder_flow_and_closes_session() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");

    let stopped = recorder.stop().expect("recording should stop");

    assert!(!recorder.is_recording());
    assert_eq!(stopped.label, "已停止");
    assert!(stopped.flow.name.starts_with("recording-"));
    assert!(stopped.flow.display_name.contains("安全占位"));
    assert!(!stopped.flow.target_window.matched);
    assert_eq!(stopped.flow.steps.len(), 1);

    match &stopped.flow.steps[0] {
        FlowStep::Wait {
            action,
            duration_ms,
            note,
            ..
        } => {
            assert_eq!(action, "等待");
            assert_eq!(*duration_ms, 500);
            assert!(note.contains("尚未捕获真实输入"));
        }
        step => panic!("expected safe wait placeholder, got {step:?}"),
    }

    assert!(recorder.stop().is_err());
}
