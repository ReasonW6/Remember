use remember_lib::hotkeys::{
    trigger_emergency_stop, EmergencyHotkeyOutcome, EMERGENCY_HOTKEY_LABEL,
};
use remember_lib::player::{PlaybackFinishReason, PlaybackOptions, PlayerState};
use remember_lib::storage::{Flow, FlowStep, TargetWindow};
use std::{sync::mpsc, thread, time::Duration};

#[test]
fn emergency_hotkey_uses_the_documented_shortcut_label() {
    assert_eq!(EMERGENCY_HOTKEY_LABEL, "Ctrl + Alt + S");
}

#[test]
fn emergency_hotkey_trigger_interrupts_active_playback() {
    let mut player = PlayerState::default();
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            wait_only_flow(800),
            PlaybackOptions {
                speed_multiplier: 1.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("playback should start");

    thread::sleep(Duration::from_millis(30));
    let outcome = trigger_emergency_stop(&mut player);

    let EmergencyHotkeyOutcome::Stopped(payload) = outcome else {
        panic!("expected hotkey to emergency-stop playback, got {outcome:?}");
    };
    assert_eq!(payload.reason, PlaybackFinishReason::EmergencyStopped);
    assert!(payload.message.contains("紧急停止"));

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("emergency hotkey should interrupt playback");

    assert_eq!(finished.reason, PlaybackFinishReason::EmergencyStopped);
    assert_eq!(finished.completed_steps, 0);
}

#[test]
fn emergency_hotkey_trigger_is_safe_when_playback_is_idle() {
    let mut player = PlayerState::default();

    let outcome = trigger_emergency_stop(&mut player);

    assert_eq!(outcome, EmergencyHotkeyOutcome::NotPlaying);
}

fn wait_only_flow(duration_ms: u64) -> Flow {
    Flow {
        version: 1,
        name: "wait-only".to_string(),
        display_name: "Wait Only".to_string(),
        target_window: TargetWindow {
            title: "Test".to_string(),
            process: "test.exe".to_string(),
            size: "800 x 600".to_string(),
            matched: true,
        },
        steps: vec![FlowStep::Wait {
            id: 1,
            action: "等待".to_string(),
            duration_ms,
            delay_ms: duration_ms,
            note: "safe wait".to_string(),
        }],
    }
}
