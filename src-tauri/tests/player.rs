use remember_lib::model::{KeyState, MacroStep, Recording};
use remember_lib::player::{build_playback_plan, scaled_delay_ms, PlaybackSettings, StopToken};

fn recording() -> Recording {
    Recording::new(
        "keys",
        "2026-06-29T00:00:00Z",
        vec![
            MacroStep::Key {
                elapsed_ms: 100,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 250,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Released,
            },
        ],
    )
}

#[test]
fn validates_loop_count_and_speed() {
    assert!(PlaybackSettings::new(1, 1.0).is_ok());
    assert!(PlaybackSettings::new(0, 1.0).is_err());
    assert!(PlaybackSettings::new(1, 0.0).is_err());
}

#[test]
fn scales_delay_by_speed_multiplier() {
    assert_eq!(scaled_delay_ms(200, 1.0), 200);
    assert_eq!(scaled_delay_ms(200, 2.0), 100);
    assert_eq!(scaled_delay_ms(200, 0.5), 400);
}

#[test]
fn builds_looped_playback_plan_with_step_deltas() {
    let settings = PlaybackSettings::new(2, 2.0).expect("settings");
    let plan = build_playback_plan(&recording(), settings);

    assert_eq!(plan.len(), 4);
    assert_eq!(plan[0].loop_index, 0);
    assert_eq!(plan[0].step_index, 0);
    assert_eq!(plan[0].delay_ms, 50);
    assert_eq!(plan[1].delay_ms, 75);
    assert_eq!(plan[2].loop_index, 1);
}

#[test]
fn stop_token_defaults_to_not_stopped() {
    let token = StopToken::default();
    assert!(!token.is_stopped());
    token.request_stop();
    assert!(token.is_stopped());
}
