use remember_lib::model::{ButtonState, KeyState, MacroStep, MouseButton};
use remember_lib::recorder::{RawInputEvent, Recorder};

#[test]
fn records_key_press_and_release() {
    let mut recorder = Recorder::new(50);
    recorder
        .start("keys", 1_000, "2026-06-29T00:00:00Z")
        .expect("start");

    recorder.capture(RawInputEvent::Key {
        at_ms: 1_010,
        vk_code: 0x41,
        scan_code: 0x1E,
        extended: false,
        state: KeyState::Pressed,
    });
    recorder.capture(RawInputEvent::Key {
        at_ms: 1_040,
        vk_code: 0x41,
        scan_code: 0x1E,
        extended: false,
        state: KeyState::Released,
    });

    let recording = recorder.stop(1_050).expect("stop");

    recording.validate().expect("recording validates");
    assert_eq!(recording.steps.len(), 2);
    assert_eq!(recording.duration_ms, 50);
    assert!(matches!(
        recording.steps[0],
        MacroStep::Key {
            elapsed_ms: 10,
            state: KeyState::Pressed,
            ..
        }
    ));
    assert!(matches!(
        recording.steps[1],
        MacroStep::Key {
            elapsed_ms: 40,
            state: KeyState::Released,
            ..
        }
    ));
}

#[test]
fn samples_mouse_moves_at_configured_interval() {
    let mut recorder = Recorder::new(50);
    recorder
        .start("mouse", 1_000, "2026-06-29T00:00:00Z")
        .expect("start");

    recorder.capture(RawInputEvent::MouseMove {
        at_ms: 1_010,
        x: 10,
        y: 10,
    });
    recorder.capture(RawInputEvent::MouseMove {
        at_ms: 1_020,
        x: 20,
        y: 20,
    });
    recorder.capture(RawInputEvent::MouseMove {
        at_ms: 1_061,
        x: 30,
        y: 30,
    });

    let recording = recorder.stop(1_070).expect("stop");

    recording.validate().expect("recording validates");
    assert_eq!(recording.steps.len(), 2);
    assert!(matches!(
        recording.steps[0],
        MacroStep::MouseMove {
            elapsed_ms: 10,
            x: 10,
            y: 10
        }
    ));
    assert!(matches!(
        recording.steps[1],
        MacroStep::MouseMove {
            elapsed_ms: 61,
            x: 30,
            y: 30
        }
    ));
}

#[test]
fn records_every_move_while_any_mouse_button_is_pressed_then_resumes_sampling() {
    for button in [
        MouseButton::Left,
        MouseButton::Right,
        MouseButton::Middle,
        MouseButton::X1,
        MouseButton::X2,
    ] {
        let mut recorder = Recorder::new(50);
        recorder
            .start("drag", 1_000, "2026-06-29T00:00:00Z")
            .expect("start");

        recorder.capture(RawInputEvent::MouseButton {
            at_ms: 1_010,
            x: 10,
            y: 10,
            button,
            state: ButtonState::Pressed,
        });
        recorder.capture(RawInputEvent::MouseMove {
            at_ms: 1_011,
            x: 11,
            y: 11,
        });
        recorder.capture(RawInputEvent::MouseMove {
            at_ms: 1_012,
            x: 12,
            y: 12,
        });
        recorder.capture(RawInputEvent::MouseButton {
            at_ms: 1_013,
            x: 13,
            y: 13,
            button,
            state: ButtonState::Released,
        });
        recorder.capture(RawInputEvent::MouseMove {
            at_ms: 1_020,
            x: 20,
            y: 20,
        });
        recorder.capture(RawInputEvent::MouseMove {
            at_ms: 1_062,
            x: 62,
            y: 62,
        });

        let recording = recorder.stop(1_070).expect("stop");
        recording.validate().expect("recording validates");

        let moves = recording
            .steps
            .iter()
            .filter_map(|step| match step {
                MacroStep::MouseMove { elapsed_ms, x, y } => Some((*elapsed_ms, *x, *y)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            moves,
            vec![(11, 11, 11), (12, 12, 12), (62, 62, 62)],
            "button {button:?}"
        );
    }
}

#[test]
fn preserves_click_position_even_after_recent_move() {
    let mut recorder = Recorder::new(50);
    recorder
        .start("click", 1_000, "2026-06-29T00:00:00Z")
        .expect("start");

    recorder.capture(RawInputEvent::MouseMove {
        at_ms: 1_010,
        x: 10,
        y: 10,
    });
    recorder.capture(RawInputEvent::MouseButton {
        at_ms: 1_020,
        x: 20,
        y: 20,
        button: MouseButton::Left,
        state: ButtonState::Pressed,
    });

    let recording = recorder.stop(1_030).expect("stop");

    recording.validate().expect("recording validates");
    assert!(matches!(
        recording.steps.last(),
        Some(MacroStep::MouseButton {
            elapsed_ms: 20,
            x: 20,
            y: 20,
            button: MouseButton::Left,
            state: ButtonState::Pressed
        })
    ));
}

#[test]
fn ignores_out_of_order_events_and_preserves_valid_recording() {
    let mut recorder = Recorder::new(50);
    recorder
        .start("stale", 1_000, "2026-06-29T00:00:00Z")
        .expect("start");

    recorder.capture(RawInputEvent::Key {
        at_ms: 1_050,
        vk_code: 0x41,
        scan_code: 0x1E,
        extended: false,
        state: KeyState::Pressed,
    });
    recorder.capture(RawInputEvent::Key {
        at_ms: 1_040,
        vk_code: 0x41,
        scan_code: 0x1E,
        extended: false,
        state: KeyState::Released,
    });
    recorder.capture(RawInputEvent::MouseMove {
        at_ms: 1_030,
        x: 10,
        y: 10,
    });
    recorder.capture(RawInputEvent::MouseButton {
        at_ms: 1_060,
        x: 20,
        y: 20,
        button: MouseButton::Left,
        state: ButtonState::Released,
    });

    let recording = recorder.stop(1_070).expect("stop");

    recording.validate().expect("recording validates");
    assert_eq!(recording.steps.len(), 2);
    assert!(matches!(
        recording.steps[0],
        MacroStep::Key {
            elapsed_ms: 50,
            state: KeyState::Pressed,
            ..
        }
    ));
    assert!(matches!(
        recording.steps[1],
        MacroStep::MouseButton {
            elapsed_ms: 60,
            x: 20,
            y: 20,
            button: MouseButton::Left,
            state: ButtonState::Released
        }
    ));
}

#[test]
fn stop_duration_is_at_least_final_step_elapsed() {
    let mut recorder = Recorder::new(50);
    recorder
        .start("early stop", 1_000, "2026-06-29T00:00:00Z")
        .expect("start");

    recorder.capture(RawInputEvent::Key {
        at_ms: 1_060,
        vk_code: 0x41,
        scan_code: 0x1E,
        extended: false,
        state: KeyState::Pressed,
    });

    let recording = recorder.stop(1_050).expect("stop");

    recording.validate().expect("recording validates");
    assert_eq!(recording.duration_ms, 60);
    assert!(matches!(
        recording.steps.last(),
        Some(MacroStep::Key {
            elapsed_ms: 60,
            state: KeyState::Pressed,
            ..
        })
    ));
}

#[test]
fn cannot_start_twice_without_stopping() {
    let mut recorder = Recorder::new(50);
    recorder
        .start("first", 1_000, "2026-06-29T00:00:00Z")
        .expect("start");

    let error = recorder
        .start("second", 1_001, "2026-06-29T00:00:00Z")
        .expect_err("second start fails");

    assert!(error.contains("already recording"));
}
