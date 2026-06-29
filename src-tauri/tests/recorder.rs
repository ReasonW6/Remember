use remember_lib::recorder::{RecordedMouseButton, RecorderState, ScreenRect};
use remember_lib::storage::{DragPathPoint, FlowStep, TargetWindow};

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
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");
    recorder
        .record_mouse_click_at(120, 240, RecordedMouseButton::Left, started_at_ms + 100)
        .expect("click without target metadata should be recorded");

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
fn stop_recording_combines_fast_nearby_left_clicks_into_double_click_step() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_mouse_click_at(120, 240, RecordedMouseButton::Left, started_at_ms + 250)
        .expect("first click should be recorded");
    recorder
        .record_mouse_click_at(122, 242, RecordedMouseButton::Left, started_at_ms + 430)
        .expect("second click should be recorded");
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
            assert_eq!(action, "双击");
            assert_eq!(target, "(120, 240) [屏幕绝对]");
            assert_eq!((*x, *y), (120, 240));
            assert_eq!(*delay_ms, 250);
            assert!(note.contains("双击"));
        }
        step => panic!("expected double-click step, got {step:?}"),
    }
    match &stopped.flow.steps[1] {
        FlowStep::Click {
            action, delay_ms, ..
        } => {
            assert_eq!(action, "右键单击");
            assert_eq!(*delay_ms, 470);
        }
        step => panic!("expected right-click step, got {step:?}"),
    }
}

#[test]
fn stop_recording_converts_mouse_wheel_to_scroll_steps_with_wait_timing() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_mouse_scroll_at(220, 340, 0, -120, started_at_ms + 350)
        .expect("vertical scroll should be recorded");
    recorder
        .record_mouse_scroll_at(220, 340, 120, 0, started_at_ms + 800)
        .expect("horizontal scroll should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 2);
    match &stopped.flow.steps[0] {
        FlowStep::Scroll {
            x,
            y,
            action,
            delta_x,
            delta_y,
            delay_ms,
            note,
            ..
        } => {
            assert_eq!(action, "滚动");
            assert_eq!((*x, *y), (Some(220), Some(340)));
            assert_eq!((*delta_x, *delta_y), (0, -120));
            assert_eq!(*delay_ms, 350);
            assert!(note.contains("鼠标滚轮"));
        }
        step => panic!("expected vertical scroll step, got {step:?}"),
    }
    match &stopped.flow.steps[1] {
        FlowStep::Scroll {
            x,
            y,
            delta_x,
            delta_y,
            delay_ms,
            ..
        } => {
            assert_eq!((*x, *y), (Some(220), Some(340)));
            assert_eq!((*delta_x, *delta_y), (120, 0));
            assert_eq!(*delay_ms, 450);
        }
        step => panic!("expected horizontal scroll step, got {step:?}"),
    }
}

#[test]
fn stop_recording_converts_mouse_drag_to_drag_step_with_duration() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_mouse_drag_at(
            120,
            240,
            420,
            520,
            RecordedMouseButton::Left,
            started_at_ms + 300,
            started_at_ms + 820,
        )
        .expect("left drag should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 1);
    match &stopped.flow.steps[0] {
        FlowStep::Drag {
            action,
            target,
            start_x,
            start_y,
            end_x,
            end_y,
            duration_ms,
            delay_ms,
            note,
            ..
        } => {
            assert_eq!(action, "左键拖拽");
            assert_eq!(target, "(120, 240) -> (420, 520) [屏幕绝对]");
            assert_eq!((*start_x, *start_y, *end_x, *end_y), (120, 240, 420, 520));
            assert_eq!(*delay_ms, 300);
            assert_eq!(*duration_ms, 520);
            assert!(note.contains("鼠标拖拽"));
        }
        step => panic!("expected drag step, got {step:?}"),
    }
}

#[test]
fn stop_recording_preserves_mouse_drag_path_points() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_mouse_drag_path_at(
            120,
            240,
            420,
            520,
            RecordedMouseButton::Left,
            started_at_ms + 300,
            started_at_ms + 820,
            vec![
                (120, 240, started_at_ms + 300),
                (260, 360, started_at_ms + 520),
                (420, 520, started_at_ms + 820),
            ],
        )
        .expect("left drag path should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 1);
    match &stopped.flow.steps[0] {
        FlowStep::Drag {
            path, duration_ms, ..
        } => {
            assert_eq!(*duration_ms, 520);
            assert_eq!(
                path,
                &vec![
                    DragPathPoint {
                        x: 120,
                        y: 240,
                        elapsed_ms: 0,
                    },
                    DragPathPoint {
                        x: 260,
                        y: 360,
                        elapsed_ms: 220,
                    },
                    DragPathPoint {
                        x: 420,
                        y: 520,
                        elapsed_ms: 520,
                    },
                ]
            );
        }
        step => panic!("expected drag step with path, got {step:?}"),
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
fn stop_recording_excludes_mouse_clicks_captured_on_remember_window() {
    let mut recorder = RecorderState::default();
    let remember_window = TargetWindow {
        title: "Remember".to_string(),
        process: "remember.exe".to_string(),
        size: "536 x 209".to_string(),
        matched: true,
    };
    let target_window = TargetWindow {
        title: "Report - Notepad".to_string(),
        process: "notepad.exe".to_string(),
        size: "1024 x 768".to_string(),
        matched: true,
    };
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_mouse_click_at_target(
            500,
            500,
            RecordedMouseButton::Left,
            started_at_ms + 100,
            remember_window,
        )
        .expect("remember click should be collected before filtering");
    recorder
        .record_mouse_click_at_target(
            620,
            640,
            RecordedMouseButton::Left,
            started_at_ms + 350,
            target_window,
        )
        .expect("target click should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 1);
    match &stopped.flow.steps[0] {
        FlowStep::Click { x, y, delay_ms, .. } => {
            assert_eq!((*x, *y), (620, 640));
            assert_eq!(*delay_ms, 350);
        }
        step => panic!("expected target click, got {step:?}"),
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
fn stop_recording_splits_text_input_when_target_window_changes() {
    let mut recorder = RecorderState::default();
    let first_target_window = TargetWindow {
        title: "First - Notepad".to_string(),
        process: "notepad.exe".to_string(),
        size: "1024 x 768".to_string(),
        matched: true,
    };
    let second_target_window = TargetWindow {
        title: "Second - Notepad".to_string(),
        process: "notepad.exe".to_string(),
        size: "1024 x 768".to_string(),
        matched: true,
    };
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_text_input_at_target("A", started_at_ms + 100, first_target_window)
        .expect("first window text should be recorded");
    recorder
        .record_text_input_at_target("B", started_at_ms + 180, second_target_window)
        .expect("second window text should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 2);
    match &stopped.flow.steps[0] {
        FlowStep::Type { text, delay_ms, .. } => {
            assert_eq!(text, "A");
            assert_eq!(*delay_ms, 100);
        }
        step => panic!("expected first type step, got {step:?}"),
    }
    match &stopped.flow.steps[1] {
        FlowStep::Type { text, delay_ms, .. } => {
            assert_eq!(text, "B");
            assert_eq!(*delay_ms, 80);
        }
        step => panic!("expected second type step, got {step:?}"),
    }
}

#[test]
fn stop_recording_excludes_keyboard_input_inside_remember_windows() {
    let mut recorder = RecorderState::default();
    let remember_window = TargetWindow {
        title: "Remember".to_string(),
        process: "remember.exe".to_string(),
        size: "536 x 209".to_string(),
        matched: true,
    };
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_text_input_at_target("x", started_at_ms + 100, remember_window)
        .expect("remember window text should be collected before filtering");

    let stopped = recorder.stop().expect("recording should stop");

    assert!(stopped.flow.steps.is_empty());
    assert!(!stopped.flow.target_window.matched);
}

#[test]
fn stop_recording_excludes_sensitive_keyboard_input() {
    let mut recorder = RecorderState::default();
    let password_window = TargetWindow {
        title: "Password - Browser".to_string(),
        process: "browser.exe".to_string(),
        size: "1024 x 768".to_string(),
        matched: true,
    };
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_text_input_at_target("secret", started_at_ms + 100, password_window)
        .expect("sensitive text should be collected before filtering");

    let stopped = recorder.stop().expect("recording should stop");

    assert!(stopped.flow.steps.is_empty());
    assert!(!stopped.flow.target_window.matched);
}

#[test]
fn stop_recording_excludes_high_risk_hotkeys() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_hotkey_at(
            vec!["Alt".to_string(), "F4".to_string()],
            started_at_ms + 100,
        )
        .expect("unsafe hotkey should be collected before filtering");

    let stopped = recorder.stop().expect("recording should stop");

    assert!(stopped.flow.steps.is_empty());
    assert!(!stopped.flow.target_window.matched);
}

#[test]
fn stop_recording_records_plain_control_keys_with_wait_timing() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");
    let started_at_ms = recorder
        .active_started_at_ms()
        .expect("active session should expose start time");

    recorder
        .record_text_input_at("A", started_at_ms + 100)
        .expect("typed character should be recorded");
    recorder
        .record_key_press_at("Enter", started_at_ms + 360)
        .expect("enter should be recorded");
    recorder
        .record_key_press_at("Backspace", started_at_ms + 620)
        .expect("backspace should be recorded");

    let stopped = recorder.stop().expect("recording should stop");

    assert_eq!(stopped.flow.steps.len(), 3);
    match &stopped.flow.steps[1] {
        FlowStep::Key {
            action,
            key,
            delay_ms,
            note,
            ..
        } => {
            assert_eq!(action, "按键");
            assert_eq!(key, "Enter");
            assert_eq!(*delay_ms, 260);
            assert!(note.contains("键盘按键"));
        }
        step => panic!("expected key step, got {step:?}"),
    }
    match &stopped.flow.steps[2] {
        FlowStep::Key { key, delay_ms, .. } => {
            assert_eq!(key, "Backspace");
            assert_eq!(*delay_ms, 260);
        }
        step => panic!("expected key step, got {step:?}"),
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
fn stop_recording_returns_empty_flow_and_closes_session() {
    let mut recorder = RecorderState::default();
    recorder.start().expect("recording should start");

    let stopped = recorder.stop().expect("recording should stop");

    assert!(!recorder.is_recording());
    assert_eq!(stopped.label, "已停止");
    assert!(stopped.flow.name.starts_with("recording-"));
    assert!(stopped.flow.display_name.contains("空录制"));
    assert!(!stopped.flow.target_window.matched);
    assert!(stopped.flow.steps.is_empty());

    assert!(recorder.stop().is_err());
}
