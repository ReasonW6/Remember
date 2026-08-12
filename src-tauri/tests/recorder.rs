use remember_lib::model::{
    ButtonState, ClientSize, KeyState, MacroStep, MouseButton, PointerPosition,
    TargetWindowAvailability, TargetWindowId, WindowPointerIntent, RECORDING_VERSION_V1,
    RECORDING_VERSION_V2,
};
use remember_lib::recorder::{
    CaptureOutcome, CaptureSurface, CapturedWindow, RawInputEvent, Recorder, WindowInstanceId,
    MAX_RECORDING_STEPS,
};
use remember_lib::storage::recording_to_json;

fn captured_window(
    hwnd: usize,
    generation: u64,
    origin: (i32, i32),
    availability: TargetWindowAvailability,
) -> CapturedWindow {
    CapturedWindow {
        instance: WindowInstanceId { hwnd, generation },
        executable_path: format!(r"C:\Apps\app-{hwnd}.exe"),
        window_class: format!("AppWindow{hwnd}"),
        title: format!("Window {hwnd}"),
        client_origin_x: origin.0,
        client_origin_y: origin.1,
        client_size: ClientSize {
            width: 800,
            height: 600,
        },
        dpi: 96,
        availability,
        direct_pointer_target: true,
    }
}

fn window_surface(window: CapturedWindow, intent: WindowPointerIntent) -> CaptureSurface {
    CaptureSurface::Window { window, intent }
}

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
    assert_eq!(recording.version, RECORDING_VERSION_V1);
    assert!(recording.targets.is_empty());
    let json = recording_to_json(&recording).expect("serialize legacy recording");
    assert!(!json.contains("\"targets\""));
    assert!(!json.contains("pointer_"));
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
fn v2_records_explicit_screen_and_multi_window_positions_with_stable_target_ids() {
    let mut recorder = Recorder::new(50);
    recorder
        .start_window_relative("relative", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");

    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_010,
                x: -20,
                y: 40,
            },
            CaptureSurface::Screen,
        ),
        CaptureOutcome::Continue
    );

    let first = captured_window(10, 1, (100, 200), TargetWindowAvailability::Initial);
    recorder.capture_with_surface(
        RawInputEvent::MouseMove {
            at_ms: 1_020,
            x: 150,
            y: 260,
        },
        window_surface(first.clone(), WindowPointerIntent::Foreground),
    );

    let mut moved_first = first;
    moved_first.client_origin_x = 120;
    moved_first.client_origin_y = 230;
    recorder.capture_with_surface(
        RawInputEvent::MouseButton {
            at_ms: 1_025,
            x: 170,
            y: 280,
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        },
        window_surface(moved_first, WindowPointerIntent::ActivationClick),
    );

    let second = captured_window(10, 4, (1_000, 50), TargetWindowAvailability::Deferred);
    recorder.capture_with_surface(
        RawInputEvent::MouseWheel {
            at_ms: 1_030,
            x: 1_320,
            y: 290,
            delta: -120,
        },
        window_surface(second, WindowPointerIntent::BackgroundWheel),
    );

    let recording = recorder.stop(1_040).expect("stop v2");

    recording.validate().expect("v2 recording validates");
    assert_eq!(recording.version, RECORDING_VERSION_V2);
    assert_eq!(recording.targets.len(), 2);
    assert_eq!(recording.targets[0].id, TargetWindowId(1));
    assert_eq!(
        recording.targets[0].availability,
        TargetWindowAvailability::Initial
    );
    assert_eq!(recording.targets[1].id, TargetWindowId(2));
    assert_eq!(
        recording.targets[1].availability,
        TargetWindowAvailability::Deferred
    );
    assert_eq!(
        recording.steps,
        vec![
            MacroStep::PointerMove {
                elapsed_ms: 10,
                position: PointerPosition::ScreenRelative { x: -20, y: 40 },
            },
            MacroStep::PointerMove {
                elapsed_ms: 20,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(1),
                    x: 50,
                    y: 60,
                    intent: WindowPointerIntent::Foreground,
                },
            },
            MacroStep::PointerButton {
                elapsed_ms: 25,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(1),
                    x: 50,
                    y: 50,
                    intent: WindowPointerIntent::ActivationClick,
                },
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            },
            MacroStep::PointerWheel {
                elapsed_ms: 30,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(2),
                    x: 320,
                    y: 240,
                    intent: WindowPointerIntent::BackgroundWheel,
                },
                delta: -120,
            },
        ]
    );
}

#[test]
fn v2_window_adjustment_locks_the_starting_origin_and_accepts_the_resulting_resize() {
    let mut recorder = Recorder::new(50);
    recorder
        .start_window_relative("window adjustment", 1_000, "2026-08-12T00:00:00Z")
        .expect("start v2");
    let window = captured_window(10, 1, (100, 200), TargetWindowAvailability::Initial);

    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::MouseButton {
                at_ms: 1_010,
                x: 120,
                y: 220,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            },
            window_surface(window.clone(), WindowPointerIntent::Foreground),
        ),
        CaptureOutcome::Continue
    );

    let mut adjusted = window;
    adjusted.client_origin_x = 200;
    adjusted.client_origin_y = 300;
    adjusted.client_size = ClientSize {
        width: 700,
        height: 500,
    };
    for (at_ms, x, y, state) in [
        (1_011, 250, 320, None),
        (1_012, 260, 330, None),
        (1_020, 260, 330, Some(ButtonState::Released)),
    ] {
        let event = state.map_or(RawInputEvent::MouseMove { at_ms, x, y }, |state| {
            RawInputEvent::MouseButton {
                at_ms,
                x,
                y,
                button: MouseButton::Left,
                state,
            }
        });
        assert_eq!(
            recorder.capture_with_surface(
                event,
                window_surface(adjusted.clone(), WindowPointerIntent::Foreground),
            ),
            CaptureOutcome::Continue
        );
    }

    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_080,
                x: 250,
                y: 320,
            },
            window_surface(adjusted, WindowPointerIntent::Foreground),
        ),
        CaptureOutcome::Continue
    );

    let recording = recorder.stop(1_090).expect("stop v2");
    recording.validate().expect("window adjustment validates");
    assert!(matches!(
        recording.steps[0],
        MacroStep::PointerButton {
            position: PointerPosition::WindowRelative {
                x: 20,
                y: 20,
                intent: WindowPointerIntent::WindowAdjustment,
                ..
            },
            state: ButtonState::Pressed,
            ..
        }
    ));
    assert!(matches!(
        recording.steps[1],
        MacroStep::PointerMove {
            position: PointerPosition::WindowRelative {
                x: 150,
                y: 120,
                intent: WindowPointerIntent::WindowAdjustment,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        recording.steps[2],
        MacroStep::PointerMove {
            position: PointerPosition::WindowRelative {
                x: 160,
                y: 130,
                intent: WindowPointerIntent::WindowAdjustment,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        recording.steps[3],
        MacroStep::PointerButton {
            position: PointerPosition::WindowRelative {
                x: 160,
                y: 130,
                intent: WindowPointerIntent::WindowAdjustment,
                ..
            },
            state: ButtonState::Released,
            ..
        }
    ));
    assert!(matches!(
        recording.steps[4],
        MacroStep::PointerMove {
            position: PointerPosition::WindowRelative {
                x: 50,
                y: 20,
                intent: WindowPointerIntent::Foreground,
                ..
            },
            ..
        }
    ));
}

#[test]
fn v2_keyboard_sequence_keeps_its_starting_target_across_focus_changes() {
    let mut recorder = Recorder::new(50);
    recorder
        .start_window_relative("keys", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");
    let first = captured_window(10, 1, (0, 0), TargetWindowAvailability::Initial);
    let second = captured_window(20, 1, (900, 0), TargetWindowAvailability::Initial);

    let key = |at_ms, vk_code, scan_code, state| RawInputEvent::Key {
        at_ms,
        vk_code,
        scan_code,
        extended: false,
        state,
    };
    assert_eq!(
        recorder.capture_with_surface(
            key(1_010, 0x12, 0x38, KeyState::Pressed),
            window_surface(first.clone(), WindowPointerIntent::Foreground),
        ),
        CaptureOutcome::Continue
    );
    assert_eq!(
        recorder.capture_with_surface(
            key(1_020, 0x09, 0x0F, KeyState::Pressed),
            CaptureSurface::Unreadable("focus is changing".to_string()),
        ),
        CaptureOutcome::Continue,
        "a readable sequence must not be interrupted when Alt+Tab changes focus"
    );
    recorder.capture_with_surface(
        key(1_030, 0x09, 0x0F, KeyState::Released),
        window_surface(second.clone(), WindowPointerIntent::Foreground),
    );
    recorder.capture_with_surface(
        key(1_040, 0x12, 0x38, KeyState::Released),
        window_surface(second.clone(), WindowPointerIntent::Foreground),
    );
    recorder.capture_with_surface(
        key(1_050, 0x42, 0x30, KeyState::Pressed),
        window_surface(second.clone(), WindowPointerIntent::Foreground),
    );
    recorder.capture_with_surface(
        key(1_060, 0x42, 0x30, KeyState::Released),
        window_surface(second, WindowPointerIntent::Foreground),
    );

    let recording = recorder.stop(1_070).expect("stop v2");
    recording.validate().expect("v2 recording validates");
    let target_ids = recording
        .steps
        .iter()
        .map(|step| match step {
            MacroStep::TargetedKey { target_id, .. } => *target_id,
            other => panic!("unexpected step {other:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(recording.targets.len(), 2);
    assert_eq!(
        target_ids,
        vec![
            Some(TargetWindowId(1)),
            Some(TargetWindowId(1)),
            Some(TargetWindowId(1)),
            Some(TargetWindowId(1)),
            Some(TargetWindowId(2)),
            Some(TargetWindowId(2)),
        ]
    );
}

#[test]
fn v2_skips_a_keyboard_sequence_that_starts_on_an_unreadable_target() {
    let mut recorder = Recorder::new(50);
    recorder
        .start_window_relative("skipped keys", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");
    let window = captured_window(10, 1, (0, 0), TargetWindowAvailability::Initial);

    let key = |at_ms, vk_code, state| RawInputEvent::Key {
        at_ms,
        vk_code,
        scan_code: vk_code,
        extended: false,
        state,
    };
    assert_eq!(
        recorder.capture_with_surface(
            key(1_010, 0x11, KeyState::Pressed),
            CaptureSurface::Unreadable("administrator access required".to_string()),
        ),
        CaptureOutcome::Continue
    );
    recorder.capture_with_surface(
        key(1_020, 0x43, KeyState::Pressed),
        window_surface(window.clone(), WindowPointerIntent::Foreground),
    );
    recorder.capture_with_surface(
        key(1_030, 0x43, KeyState::Released),
        window_surface(window.clone(), WindowPointerIntent::Foreground),
    );
    assert_eq!(
        recorder.capture_with_surface(
            key(1_040, 0x11, KeyState::Released),
            window_surface(window.clone(), WindowPointerIntent::Foreground),
        ),
        CaptureOutcome::Continue
    );
    recorder.capture_with_surface(
        key(1_050, 0x44, KeyState::Pressed),
        window_surface(window.clone(), WindowPointerIntent::Foreground),
    );
    recorder.capture_with_surface(
        key(1_060, 0x44, KeyState::Released),
        window_surface(window, WindowPointerIntent::Foreground),
    );

    let recording = recorder.stop(1_070).expect("stop v2");
    recording.validate().expect("v2 recording validates");
    assert_eq!(recording.targets.len(), 1);
    assert_eq!(recording.steps.len(), 2);
    assert!(recording.steps.iter().all(|step| matches!(
        step,
        MacroStep::TargetedKey {
            vk_code: 0x44,
            target_id: Some(TargetWindowId(1)),
            ..
        }
    )));
}

#[test]
fn v2_ignores_orphan_key_releases_without_creating_a_sequence_or_target() {
    let mut recorder = Recorder::new(50);
    recorder
        .start_window_relative("orphan release", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");
    let window = captured_window(10, 1, (0, 0), TargetWindowAvailability::Initial);
    let key = |at_ms, state| RawInputEvent::Key {
        at_ms,
        vk_code: 0x41,
        scan_code: 0x1E,
        extended: false,
        state,
    };

    assert_eq!(
        recorder.capture_with_surface(
            key(1_010, KeyState::Released),
            window_surface(window.clone(), WindowPointerIntent::Foreground),
        ),
        CaptureOutcome::Continue
    );
    assert_eq!(
        recorder.capture_with_surface(
            key(1_020, KeyState::Released),
            CaptureSurface::Unreadable("not readable".to_string()),
        ),
        CaptureOutcome::Continue
    );
    recorder.capture_with_surface(
        key(1_030, KeyState::Pressed),
        window_surface(window.clone(), WindowPointerIntent::Foreground),
    );
    recorder.capture_with_surface(
        key(1_040, KeyState::Released),
        window_surface(window, WindowPointerIntent::Foreground),
    );

    let recording = recorder.stop(1_050).expect("stop v2");
    recording.validate().expect("v2 recording validates");
    assert_eq!(recording.targets.len(), 1);
    assert_eq!(recording.steps.len(), 2);
    assert!(matches!(
        recording.steps[0],
        MacroStep::TargetedKey {
            elapsed_ms: 30,
            state: KeyState::Pressed,
            ..
        }
    ));
}

#[test]
fn unreadable_keyboard_sequences_do_not_keep_the_pointer_warning_visible() {
    let mut recorder = Recorder::new(50);
    recorder
        .start_window_relative("warning ownership", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");

    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_010,
                x: 10,
                y: 10,
            },
            CaptureSurface::Unreadable("pointer target is unreadable".to_string()),
        ),
        CaptureOutcome::UnreadableStarted("pointer target is unreadable".to_string())
    );
    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::Key {
                at_ms: 1_020,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            },
            CaptureSurface::Unreadable("keyboard target is unreadable".to_string()),
        ),
        CaptureOutcome::Continue
    );
    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_030,
                x: 30,
                y: 30,
            },
            CaptureSurface::Screen,
        ),
        CaptureOutcome::UnreadableEnded,
        "the pointer warning ends even while the skipped key sequence remains held"
    );
    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::Key {
                at_ms: 1_040,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Released,
            },
            CaptureSurface::Screen,
        ),
        CaptureOutcome::Continue
    );

    let recording = recorder.stop(1_050).expect("stop v2");
    recording.validate().expect("v2 recording validates");
    assert_eq!(
        recording.steps,
        vec![MacroStep::PointerMove {
            elapsed_ms: 30,
            position: PointerPosition::ScreenRelative { x: 30, y: 30 },
        }]
    );
}

#[test]
fn v2_safely_splits_a_drag_around_an_unreadable_pointer_interval() {
    let mut recorder = Recorder::new(50);
    recorder
        .start_window_relative("split drag", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");

    recorder.capture_with_surface(
        RawInputEvent::MouseMove {
            at_ms: 1_010,
            x: 10,
            y: 10,
        },
        CaptureSurface::Screen,
    );
    recorder.capture_with_surface(
        RawInputEvent::MouseButton {
            at_ms: 1_020,
            x: 10,
            y: 10,
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        },
        CaptureSurface::Screen,
    );
    recorder.capture_with_surface(
        RawInputEvent::MouseMove {
            at_ms: 1_021,
            x: 15,
            y: 16,
        },
        CaptureSurface::Screen,
    );
    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_030,
                x: 20,
                y: 20,
            },
            CaptureSurface::Unreadable("higher-integrity window".to_string()),
        ),
        CaptureOutcome::UnreadableStarted("higher-integrity window".to_string())
    );
    recorder.capture_with_surface(
        RawInputEvent::MouseMove {
            at_ms: 1_040,
            x: 500,
            y: 500,
        },
        CaptureSurface::Unreadable("higher-integrity window".to_string()),
    );

    let window = captured_window(10, 1, (100, 200), TargetWindowAvailability::Deferred);
    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_050,
                x: 130,
                y: 240,
            },
            window_surface(window.clone(), WindowPointerIntent::Foreground),
        ),
        CaptureOutcome::UnreadableEnded
    );
    recorder.capture_with_surface(
        RawInputEvent::MouseButton {
            at_ms: 1_060,
            x: 135,
            y: 245,
            button: MouseButton::Left,
            state: ButtonState::Released,
        },
        window_surface(window, WindowPointerIntent::Foreground),
    );

    let recording = recorder.stop(1_070).expect("stop v2");
    recording.validate().expect("v2 recording validates");
    assert_eq!(recording.steps.len(), 7);
    assert_eq!(
        recording.steps[3],
        MacroStep::PointerButton {
            elapsed_ms: 30,
            position: PointerPosition::ScreenRelative { x: 15, y: 16 },
            button: MouseButton::Left,
            state: ButtonState::Released,
        }
    );
    assert_eq!(
        recording.steps[4],
        MacroStep::PointerButton {
            elapsed_ms: 50,
            position: PointerPosition::WindowRelative {
                target_id: TargetWindowId(1),
                x: 30,
                y: 40,
                intent: WindowPointerIntent::Foreground,
            },
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        }
    );
    assert_eq!(
        recording.steps[5],
        MacroStep::PointerMove {
            elapsed_ms: 50,
            position: PointerPosition::WindowRelative {
                target_id: TargetWindowId(1),
                x: 30,
                y: 40,
                intent: WindowPointerIntent::Foreground,
            },
        }
    );
    assert!(recording
        .steps
        .iter()
        .all(|step| !matches!(step.elapsed_ms(), 31..=49)));
}

#[test]
fn v2_does_not_resume_a_mouse_button_released_inside_an_unreadable_interval() {
    let mut recorder = Recorder::new(50);
    recorder
        .start_window_relative("released drag", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");
    recorder.capture_with_surface(
        RawInputEvent::MouseButton {
            at_ms: 1_010,
            x: 10,
            y: 10,
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        },
        CaptureSurface::Screen,
    );
    recorder.capture_with_surface(
        RawInputEvent::MouseMove {
            at_ms: 1_020,
            x: 20,
            y: 20,
        },
        CaptureSurface::Unreadable("unreadable".to_string()),
    );
    recorder.capture_with_surface(
        RawInputEvent::MouseButton {
            at_ms: 1_030,
            x: 30,
            y: 30,
            button: MouseButton::Left,
            state: ButtonState::Released,
        },
        CaptureSurface::Unreadable("unreadable".to_string()),
    );
    recorder.capture_with_surface(
        RawInputEvent::MouseMove {
            at_ms: 1_040,
            x: 40,
            y: 40,
        },
        CaptureSurface::Screen,
    );

    let recording = recorder.stop(1_050).expect("stop v2");
    recording.validate().expect("v2 recording validates");
    let presses = recording
        .steps
        .iter()
        .filter(|step| {
            matches!(
                step,
                MacroStep::PointerButton {
                    state: ButtonState::Pressed,
                    ..
                }
            )
        })
        .count();
    assert_eq!(presses, 1);
    assert!(matches!(
        recording.steps.last(),
        Some(MacroStep::PointerMove {
            elapsed_ms: 40,
            position: PointerPosition::ScreenRelative { x: 40, y: 40 },
        })
    ));
}

#[test]
fn v2_stops_before_recording_a_window_identity_or_dpi_change() {
    for (label, mutate, expected) in [
        (
            "path",
            (|window: &mut CapturedWindow| window.executable_path.push_str(".changed"))
                as fn(&mut CapturedWindow),
            "程序路径",
        ),
        (
            "class",
            (|window: &mut CapturedWindow| window.window_class.push_str("Changed"))
                as fn(&mut CapturedWindow),
            "窗口类",
        ),
        (
            "dpi",
            (|window: &mut CapturedWindow| window.dpi = 144) as fn(&mut CapturedWindow),
            "DPI",
        ),
    ] {
        let mut recorder = Recorder::new(0);
        recorder
            .start_window_relative(label, 1_000, "2026-08-09T00:00:00Z")
            .expect("start v2");
        let window = captured_window(10, 1, (100, 200), TargetWindowAvailability::Initial);
        recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_010,
                x: 120,
                y: 230,
            },
            window_surface(window.clone(), WindowPointerIntent::Foreground),
        );
        let mut changed = window;
        mutate(&mut changed);

        let outcome = recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_020,
                x: 125,
                y: 235,
            },
            window_surface(changed, WindowPointerIntent::Foreground),
        );

        assert!(matches!(
            outcome,
            CaptureOutcome::StopRecording(reason) if reason.contains(expected)
        ));
        let recording = recorder.stop(1_030).expect("stop after incompatibility");
        recording.validate().expect("valid prefix is retained");
        assert_eq!(recording.steps.len(), 1, "{label}");
    }
}

#[test]
fn v2_allows_a_known_window_title_to_change() {
    let mut recorder = Recorder::new(0);
    recorder
        .start_window_relative("title", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");
    let window = captured_window(10, 1, (100, 200), TargetWindowAvailability::Initial);
    recorder.capture_with_surface(
        RawInputEvent::MouseMove {
            at_ms: 1_010,
            x: 120,
            y: 230,
        },
        window_surface(window.clone(), WindowPointerIntent::Foreground),
    );
    let mut renamed = window;
    renamed.title = "A different document title".to_string();

    assert_eq!(
        recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_020,
                x: 125,
                y: 235,
            },
            window_surface(renamed, WindowPointerIntent::Foreground),
        ),
        CaptureOutcome::Continue
    );
    let recording = recorder.stop(1_030).expect("stop v2");
    recording.validate().expect("v2 recording validates");
    assert_eq!(recording.steps.len(), 2);
    assert_eq!(recording.targets[0].title, "Window 10");
}

#[test]
fn v2_rejects_a_new_target_with_an_empty_client_area() {
    for (width, height) in [(0, 600), (800, 0)] {
        let mut recorder = Recorder::new(0);
        recorder
            .start_window_relative("empty client", 1_000, "2026-08-09T00:00:00Z")
            .expect("start v2");
        let mut window = captured_window(10, 1, (100, 200), TargetWindowAvailability::Initial);
        window.client_size = ClientSize { width, height };

        let outcome = recorder.capture_with_surface(
            RawInputEvent::MouseMove {
                at_ms: 1_010,
                x: 120,
                y: 230,
            },
            window_surface(window, WindowPointerIntent::Foreground),
        );

        assert!(matches!(
            outcome,
            CaptureOutcome::StopRecording(reason)
                if reason.contains("客户区尺寸无效")
        ));
        let recording = recorder.stop(1_020).expect("stop after invalid target");
        recording.validate().expect("empty prefix validates");
        assert!(recording.targets.is_empty());
        assert!(recording.steps.is_empty());
    }
}

#[test]
fn v2_rejects_relative_coordinate_overflow_before_adding_a_target_or_step() {
    let mut recorder = Recorder::new(0);
    recorder
        .start_window_relative("overflow", 1_000, "2026-08-09T00:00:00Z")
        .expect("start v2");
    let window = captured_window(
        10,
        1,
        (i32::MIN, i32::MIN),
        TargetWindowAvailability::Initial,
    );

    let outcome = recorder.capture_with_surface(
        RawInputEvent::MouseMove {
            at_ms: 1_010,
            x: i32::MAX,
            y: i32::MAX,
        },
        window_surface(window, WindowPointerIntent::Foreground),
    );

    assert!(matches!(
        outcome,
        CaptureOutcome::StopRecording(reason) if reason.contains("超出支持范围")
    ));
    let recording = recorder.stop(1_020).expect("stop after overflow");
    recording.validate().expect("empty v2 prefix validates");
    assert!(recording.targets.is_empty());
    assert!(recording.steps.is_empty());
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

#[test]
fn truncates_recording_growth_at_step_limit_without_losing_captured_steps() {
    let mut recorder = Recorder::new(50);
    recorder
        .start("bounded", 0, "2026-06-29T00:00:00Z")
        .expect("start");

    for index in 0..=MAX_RECORDING_STEPS {
        recorder.capture(RawInputEvent::MouseWheel {
            at_ms: index as u64,
            x: 0,
            y: 0,
            delta: 1,
        });
    }

    let recording = recorder
        .stop(MAX_RECORDING_STEPS as u64)
        .expect("bounded recording remains saveable");

    assert_eq!(recording.steps.len(), MAX_RECORDING_STEPS);
    assert!(recorder.last_stop_was_truncated());

    recorder
        .start("next", 0, "2026-06-29T00:00:00Z")
        .expect("start next recording");
    assert!(!recorder.last_stop_was_truncated());
}
