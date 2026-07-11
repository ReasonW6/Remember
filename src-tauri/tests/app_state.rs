use remember_lib::app_state::{
    AppController, AppMode, ControlHotkey, ControlHotkeyAction, ControlHotkeyDecision,
    ControlHotkeyModifiers,
};
use remember_lib::model::{KeyState, MacroStep, Recording};
use remember_lib::recorder::RawInputEvent;

fn recording() -> Recording {
    Recording::new(
        "loaded",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::Key {
            elapsed_ms: 1,
            vk_code: 0x41,
            scan_code: 0x1E,
            extended: false,
            state: KeyState::Pressed,
        }],
    )
}

fn hotkey(vk_code: u16) -> ControlHotkey {
    ControlHotkey {
        vk_code,
        modifiers: ControlHotkeyModifiers::default(),
    }
}

fn ctrl_shift_hotkey(vk_code: u16) -> ControlHotkey {
    ControlHotkey {
        vk_code,
        modifiers: ControlHotkeyModifiers {
            ctrl: true,
            shift: true,
            ..ControlHotkeyModifiers::default()
        },
    }
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
fn stopped_recording_stays_pending_until_the_matching_recording_is_saved() {
    let mut app = AppController::new();
    app.start_recording("pending", 100, "2026-06-29T00:00:00Z")
        .expect("start");
    let recording = app.stop_recording(150).expect("stop");

    assert_eq!(app.recording_pending_save(), Some(&recording));

    let stale = Recording::new("stale", "2026-06-29T00:00:01Z", Vec::new());
    app.mark_recording_saved(&stale);
    assert_eq!(app.recording_pending_save(), Some(&recording));

    app.mark_recording_saved(&recording);
    assert!(app.recording_pending_save().is_none());
}

#[test]
fn pending_recording_cannot_be_replaced_by_a_new_recording() {
    let mut app = AppController::new();
    app.start_recording("pending", 100, "2026-06-29T00:00:00Z")
        .expect("start");
    let recording = app.stop_recording(150).expect("stop");

    let error = app
        .start_recording("replacement", 200, "2026-06-29T00:00:01Z")
        .expect_err("pending recording must be preserved");

    assert!(error.contains("current recording has not been saved"));
    assert_eq!(app.current_recording(), Some(&recording));
    assert_eq!(app.recording_pending_save(), Some(&recording));
}

#[test]
fn pending_recording_cannot_be_replaced_by_a_loaded_recording() {
    let mut app = AppController::new();
    app.start_recording("pending", 100, "2026-06-29T00:00:00Z")
        .expect("start");
    let recording = app.stop_recording(150).expect("stop");

    let error = app
        .set_recording(Recording::new(
            "replacement",
            "2026-06-29T00:00:01Z",
            Vec::new(),
        ))
        .expect_err("pending recording must be preserved");

    assert!(error.contains("current recording has not been saved"));
    assert_eq!(app.current_recording(), Some(&recording));
    assert_eq!(app.recording_pending_save(), Some(&recording));
}

#[test]
fn captures_input_while_recording() {
    let mut app = AppController::new();
    app.start_recording("test", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    app.capture_input(RawInputEvent::Key {
        at_ms: 125,
        vk_code: 0x41,
        scan_code: 0x1E,
        extended: false,
        state: KeyState::Pressed,
    });

    let saved = app.stop_recording(150).expect("stop");
    assert_eq!(
        saved.steps,
        vec![MacroStep::Key {
            elapsed_ms: 25,
            vk_code: 0x41,
            scan_code: 0x1E,
            extended: false,
            state: KeyState::Pressed,
        }]
    );
}

#[test]
fn recognizes_default_playback_hotkey_from_keyboard_hook() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");

    let action = app.control_hotkey_action(RawInputEvent::Key {
        at_ms: 125,
        vk_code: 0x7B,
        scan_code: 0x58,
        extended: false,
        state: KeyState::Pressed,
    });

    assert_eq!(action, Some(ControlHotkeyAction::Playback));
}

#[test]
fn recognizes_same_record_stop_hotkey_as_toggle() {
    let mut app = AppController::new();

    let action = app.control_hotkey_action(RawInputEvent::Key {
        at_ms: 125,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Pressed,
    });
    assert_eq!(action, Some(ControlHotkeyAction::Record));

    app.start_recording_from_hotkey("test", 125, "2026-06-29T00:00:00Z")
        .expect("start");
    let release = app.control_hotkey_action(RawInputEvent::Key {
        at_ms: 149,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Released,
    });
    assert_eq!(release, None);
    let action = app.control_hotkey_action(RawInputEvent::Key {
        at_ms: 150,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Pressed,
    });

    assert_eq!(action, Some(ControlHotkeyAction::Stop));
}

#[test]
fn ignores_repeated_control_hotkey_press_until_release() {
    let mut app = AppController::new();

    let first = app.control_hotkey_decision(RawInputEvent::Key {
        at_ms: 100,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Pressed,
    });
    let repeated = app.control_hotkey_decision(RawInputEvent::Key {
        at_ms: 101,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Pressed,
    });

    assert_eq!(first.action, Some(ControlHotkeyAction::Record));
    assert!(repeated.suppress);
    assert_eq!(repeated.action, None);
}

#[test]
fn playback_hotkey_stops_active_playback() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");
    app.start_playback(1, 1.0).expect("play");

    let decision = app.control_hotkey_decision(RawInputEvent::Key {
        at_ms: 125,
        vk_code: 0x7B,
        scan_code: 0x58,
        extended: false,
        state: KeyState::Pressed,
    });

    assert!(decision.suppress);
    assert_eq!(decision.action, Some(ControlHotkeyAction::Stop));
}

#[test]
fn keeps_modifier_state_for_idle_hook_hotkeys() {
    let mut app = AppController::new();
    app.set_control_hotkeys(
        vec![ctrl_shift_hotkey(0x52), hotkey(0x7B)],
        ctrl_shift_hotkey(0x52),
        hotkey(0x7B),
        ctrl_shift_hotkey(0x52),
    );

    let ctrl = RawInputEvent::Key {
        at_ms: 125,
        vk_code: 0x11,
        scan_code: 0x1D,
        extended: false,
        state: KeyState::Pressed,
    };
    let shift = RawInputEvent::Key {
        at_ms: 126,
        vk_code: 0x10,
        scan_code: 0x2A,
        extended: false,
        state: KeyState::Pressed,
    };
    app.control_hotkey_action(ctrl);
    app.capture_input(ctrl);
    app.control_hotkey_action(shift);
    app.capture_input(shift);

    let action = app.control_hotkey_action(RawInputEvent::Key {
        at_ms: 127,
        vk_code: 0x52,
        scan_code: 0x13,
        extended: false,
        state: KeyState::Pressed,
    });

    assert_eq!(action, Some(ControlHotkeyAction::Record));
}

#[test]
fn records_modifier_releases_when_hook_calls_decision_and_filter() {
    let mut app = AppController::new();

    let decision = app.control_hotkey_decision(RawInputEvent::Key {
        at_ms: 100,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Pressed,
    });
    assert_eq!(
        decision,
        ControlHotkeyDecision {
            suppress: true,
            action: Some(ControlHotkeyAction::Record),
        }
    );

    app.start_recording_from_hotkey("test", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    let decision = app.control_hotkey_decision(RawInputEvent::Key {
        at_ms: 101,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Released,
    });
    assert!(decision.suppress);

    for (offset, vk_code, scan_code, state) in [
        (10, 0x10, 0x2A, KeyState::Pressed),
        (11, 0x41, 0x1E, KeyState::Pressed),
        (12, 0x41, 0x1E, KeyState::Released),
        (13, 0x10, 0x2A, KeyState::Released),
    ] {
        let event = RawInputEvent::Key {
            at_ms: 100 + offset,
            vk_code,
            scan_code,
            extended: false,
            state,
        };
        let decision = app.control_hotkey_decision(event);
        assert!(!decision.suppress);
        app.capture_input(event);
    }

    let saved = app.stop_recording(120).expect("stop");
    assert_eq!(
        saved.steps,
        vec![
            MacroStep::Key {
                elapsed_ms: 10,
                vk_code: 0x10,
                scan_code: 0x2A,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 11,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 12,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Released,
            },
            MacroStep::Key {
                elapsed_ms: 13,
                vk_code: 0x10,
                scan_code: 0x2A,
                extended: false,
                state: KeyState::Released,
            },
        ]
    );
}

#[test]
fn ignores_hotkey_pressed_with_extra_modifiers() {
    let mut app = AppController::new();
    app.start_recording("test", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    for (offset, vk_code, scan_code, state) in [
        (10, 0x11, 0x1D, KeyState::Pressed),
        (11, 0x77, 0x42, KeyState::Pressed),
        (12, 0x77, 0x42, KeyState::Released),
        (13, 0x11, 0x1D, KeyState::Released),
    ] {
        let event = RawInputEvent::Key {
            at_ms: 100 + offset,
            vk_code,
            scan_code,
            extended: false,
            state,
        };
        let decision = app.control_hotkey_decision(event);
        assert_eq!(decision.action, None);
        assert!(!decision.suppress);
        app.capture_input(event);
    }

    let saved = app.stop_recording(120).expect("stop");
    assert_eq!(saved.steps.len(), 4);
}

#[test]
fn swallows_playback_hotkey_while_recording_without_action() {
    let mut app = AppController::new();
    app.start_recording("test", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    let decision = app.control_hotkey_decision(RawInputEvent::Key {
        at_ms: 110,
        vk_code: 0x7B,
        scan_code: 0x58,
        extended: false,
        state: KeyState::Pressed,
    });
    assert_eq!(
        decision,
        ControlHotkeyDecision {
            suppress: true,
            action: None,
        }
    );

    let decision = app.control_hotkey_decision(RawInputEvent::Key {
        at_ms: 111,
        vk_code: 0x7B,
        scan_code: 0x58,
        extended: false,
        state: KeyState::Released,
    });
    assert!(decision.suppress);

    let saved = app.stop_recording(120).expect("stop");
    assert_eq!(saved.steps, Vec::new());
}

#[test]
fn suppresses_default_control_hotkeys_while_recording() {
    for (vk_code, scan_code) in [(0x77, 0x42), (0x7B, 0x58)] {
        let mut app = AppController::new();
        app.start_recording("test", 100, "2026-06-29T00:00:00Z")
            .expect("start");

        for (offset, vk_code, scan_code, state) in [
            (10, vk_code, scan_code, KeyState::Pressed),
            (11, vk_code, scan_code, KeyState::Released),
        ] {
            app.capture_input(RawInputEvent::Key {
                at_ms: 100 + offset,
                vk_code,
                scan_code,
                extended: false,
                state,
            });
        }

        let saved = app.stop_recording(120).expect("stop");
        assert_eq!(saved.steps, Vec::new());
    }
}

#[test]
fn suppresses_control_hotkey_modifier_releases_while_recording() {
    let mut app = AppController::new();
    app.set_control_hotkeys(
        vec![ctrl_shift_hotkey(0x52)],
        ctrl_shift_hotkey(0x52),
        hotkey(0x7B),
        ctrl_shift_hotkey(0x52),
    );
    app.start_recording("test", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    for (offset, vk_code, scan_code, state) in [
        (10, 0x11, 0x1D, KeyState::Pressed),
        (11, 0x10, 0x2A, KeyState::Pressed),
        (12, 0x52, 0x13, KeyState::Pressed),
        (13, 0x52, 0x13, KeyState::Released),
        (14, 0x10, 0x2A, KeyState::Released),
        (15, 0x11, 0x1D, KeyState::Released),
    ] {
        app.capture_input(RawInputEvent::Key {
            at_ms: 100 + offset,
            vk_code,
            scan_code,
            extended: false,
            state,
        });
    }

    let saved = app.stop_recording(120).expect("stop");
    assert_eq!(saved.steps, Vec::new());
}

#[test]
fn suppresses_record_hotkey_release_tail_when_started_from_idle_hotkey() {
    let mut app = AppController::new();

    app.capture_input(RawInputEvent::Key {
        at_ms: 110,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Pressed,
    });

    app.start_recording_from_hotkey("test", 120, "2026-06-29T00:00:00Z")
        .expect("start");

    app.capture_input(RawInputEvent::Key {
        at_ms: 121,
        vk_code: 0x77,
        scan_code: 0x42,
        extended: false,
        state: KeyState::Released,
    });

    let saved = app.stop_recording(130).expect("stop");
    assert_eq!(saved.steps, Vec::new());
}

#[test]
fn suppresses_custom_control_hotkey_chords_while_recording() {
    let mut app = AppController::new();
    app.set_control_hotkeys(
        vec![hotkey(0x75), hotkey(0x7B)],
        hotkey(0x75),
        hotkey(0x7B),
        hotkey(0x75),
    );
    app.start_recording("test", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    for (offset, vk_code, scan_code, state) in [
        (10, 0x75, 0x40, KeyState::Pressed),
        (11, 0x75, 0x40, KeyState::Released),
    ] {
        app.capture_input(RawInputEvent::Key {
            at_ms: 100 + offset,
            vk_code,
            scan_code,
            extended: false,
            state,
        });
    }

    let saved = app.stop_recording(120).expect("stop");
    assert_eq!(saved.steps, Vec::new());
}

#[test]
fn suppresses_custom_record_hotkey_release_tail_when_started_from_idle_hotkey() {
    let mut app = AppController::new();
    app.set_control_hotkeys(
        vec![ctrl_shift_hotkey(0x52), hotkey(0x7B)],
        ctrl_shift_hotkey(0x52),
        hotkey(0x7B),
        ctrl_shift_hotkey(0x52),
    );

    for (offset, vk_code, scan_code) in [(10, 0x11, 0x1D), (11, 0x10, 0x2A), (12, 0x52, 0x13)] {
        app.capture_input(RawInputEvent::Key {
            at_ms: 100 + offset,
            vk_code,
            scan_code,
            extended: false,
            state: KeyState::Pressed,
        });
    }

    app.start_recording_from_hotkey("test", 120, "2026-06-29T00:00:00Z")
        .expect("start");

    for (offset, vk_code, scan_code) in [(1, 0x52, 0x13), (2, 0x10, 0x2A), (3, 0x11, 0x1D)] {
        app.capture_input(RawInputEvent::Key {
            at_ms: 120 + offset,
            vk_code,
            scan_code,
            extended: false,
            state: KeyState::Released,
        });
    }

    let saved = app.stop_recording(130).expect("stop");
    assert_eq!(saved.steps, Vec::new());
}

#[test]
fn preserves_non_control_shortcuts_while_recording() {
    let mut app = AppController::new();
    app.start_recording("test", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    for (offset, vk_code, scan_code, state) in [
        (10, 0x11, 0x1D, KeyState::Pressed),
        (11, 0x12, 0x38, KeyState::Pressed),
        (12, 0x41, 0x1E, KeyState::Pressed),
        (13, 0x41, 0x1E, KeyState::Released),
        (14, 0x12, 0x38, KeyState::Released),
        (15, 0x11, 0x1D, KeyState::Released),
    ] {
        app.capture_input(RawInputEvent::Key {
            at_ms: 100 + offset,
            vk_code,
            scan_code,
            extended: false,
            state,
        });
    }

    let saved = app.stop_recording(120).expect("stop");
    assert_eq!(
        saved.steps,
        vec![
            MacroStep::Key {
                elapsed_ms: 10,
                vk_code: 0x11,
                scan_code: 0x1D,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 11,
                vk_code: 0x12,
                scan_code: 0x38,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 12,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 13,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Released,
            },
            MacroStep::Key {
                elapsed_ms: 14,
                vk_code: 0x12,
                scan_code: 0x38,
                extended: false,
                state: KeyState::Released,
            },
            MacroStep::Key {
                elapsed_ms: 15,
                vk_code: 0x11,
                scan_code: 0x1D,
                extended: false,
                state: KeyState::Released,
            },
        ]
    );
}

#[test]
fn ignores_captured_input_while_idle() {
    let mut app = AppController::new();

    app.capture_input(RawInputEvent::Key {
        at_ms: 125,
        vk_code: 0x41,
        scan_code: 0x1E,
        extended: false,
        state: KeyState::Pressed,
    });

    assert!(app.current_recording().is_none());
    let error = app.saveable_recording().expect_err("no recording saved");
    assert!(error.contains("no recording loaded"));
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

    let run = app.start_playback(2, 1.0).expect("play");

    assert_eq!(app.mode(), AppMode::Playing);
    assert_eq!(run.recording.steps.len(), 1);
    assert_eq!(run.settings.loop_count, Some(2));
}

#[test]
fn starts_playback_with_current_settings() {
    let mut app = AppController::new();
    app.set_recording(Recording::new(
        "loaded",
        "2026-06-29T00:00:00Z",
        vec![
            MacroStep::Key {
                elapsed_ms: 0,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 100,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Released,
            },
        ],
    ))
    .expect("load");
    app.set_playback_settings(3, 2.0).expect("settings");

    let run = app.start_playback_with_current_settings().expect("play");

    assert_eq!(run.recording.steps.len(), 2);
    assert_eq!(run.settings.loop_count, Some(3));
    assert_eq!(run.settings.speed_multiplier, 2.0);
    assert_eq!(app.mode(), AppMode::Playing);
}

#[test]
fn stop_playback_stays_playing_until_worker_finishes_cleanup() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");
    let run = app.start_playback(1, 1.0).expect("play");
    let token = app.stop_token();

    app.stop_playback();

    assert!(token.is_stopped());
    assert_eq!(app.mode(), AppMode::Playing);
    assert_eq!(app.ui_state().message, "Stopping playback");

    assert!(app.finish_playback_if_current(run.id, "Playback stopped", false));
    assert_eq!(app.mode(), AppMode::Idle);
}

#[test]
fn stop_active_stops_recording() {
    let mut app = AppController::new();
    app.start_recording("active", 100, "2026-06-29T00:00:00Z")
        .expect("start");

    app.stop_active(150).expect("stop active");

    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Idle);
    assert_eq!(state.recording_name.as_deref(), Some("active"));
    assert_eq!(state.duration_ms, 50);
    assert_eq!(state.message, "Recording stopped");
}

#[test]
fn stop_active_stops_playback_and_requests_stop_token() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");
    app.start_playback(1, 1.0).expect("play");
    let token = app.stop_token();

    app.stop_active(150).expect("stop active");

    assert!(token.is_stopped());
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Playing);
    assert_eq!(state.message, "Stopping playback");
}

#[test]
fn stop_active_while_idle_is_noop() {
    let mut app = AppController::new();

    app.stop_active(150).expect("stop active");

    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Idle);
    assert_eq!(state.message, "Idle");
}

#[test]
fn stale_playback_finish_cannot_mark_idle_over_new_playback() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");

    let first = app.start_playback(1, 1.0).expect("first playback");
    app.stop_playback();
    assert!(app.finish_playback_if_current(first.id, "Playback stopped", false));
    let second = app.start_playback(1, 1.0).expect("second playback");

    let changed = app.finish_playback_if_current(first.id, "Playback finished", false);

    assert!(!changed);
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Playing);
    assert_eq!(state.message, "Playing");

    let changed = app.finish_playback_if_current(second.id, "Playback finished", false);

    assert!(changed);
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Idle);
    assert_eq!(state.message, "Playback finished");
}

#[test]
fn stale_playback_finish_cannot_mark_idle_over_recording() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");

    let playback = app.start_playback(1, 1.0).expect("playback");
    app.stop_playback();
    assert!(app.finish_playback_if_current(playback.id, "Playback stopped", false));
    app.start_recording("active", 100, "2026-06-29T00:00:00Z")
        .expect("start recording");

    let changed = app.finish_playback_if_current(playback.id, "Playback finished", false);

    assert!(!changed);
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Recording);
    assert_eq!(state.message, "Recording");

    let saved = app.stop_recording(150).expect("stop recording");
    assert_eq!(saved.name, "active");
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
    assert_eq!(app.mode(), AppMode::Playing);
    assert_eq!(app.ui_state().message, "Stopping playback");
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

    let run = app.start_playback(1, 1.0).expect("play");
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Playing);
    assert_eq!(state.recording_name.as_deref(), Some("active"));
    assert_eq!(state.message, "Playing");

    app.stop_playback();
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Playing);
    assert_eq!(state.recording_name.as_deref(), Some("active"));
    assert_eq!(state.message, "Stopping playback");

    assert!(app.finish_playback_if_current(run.id, "Playback stopped", false));
    let state = app.ui_state();
    assert_eq!(state.mode, AppMode::Idle);
    assert_eq!(state.message, "Playback stopped");
}

#[test]
fn ui_state_revision_increases_and_marks_playback_errors() {
    let mut app = AppController::new();
    let initial = app.ui_state();
    app.set_recording(recording()).expect("load");
    let loaded = app.ui_state();
    let run = app.start_playback(1, 1.0).expect("play");
    let playing = app.ui_state();

    assert!(loaded.revision > initial.revision);
    assert!(playing.revision > loaded.revision);
    assert!(!playing.message_is_error);

    assert!(app.finish_playback_if_current(run.id, "executor failed", true));
    let failed = app.ui_state();
    assert!(failed.revision > playing.revision);
    assert!(failed.message_is_error);
    assert_eq!(failed.message, "executor failed");
}
