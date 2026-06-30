use remember_lib::model::{ButtonState, KeyState, MacroStep, MouseButton, Recording};
use remember_lib::player::{
    build_playback_plan, play_actions, scaled_delay_ms, PlaybackSettings, StepExecutor, StopToken,
};
use std::sync::{Arc, Mutex};

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

#[derive(Default)]
struct FakeExecutor {
    calls: Arc<Mutex<Vec<String>>>,
}

impl StepExecutor for FakeExecutor {
    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("move:{x}:{y}"));
        Ok(())
    }

    fn mouse_button(
        &self,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("button:{x}:{y}:{button:?}:{state:?}"));
        Ok(())
    }

    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("wheel:{x}:{y}:{delta}"));
        Ok(())
    }

    fn key(&self, vk_code: u16, scan_code: u16, state: KeyState) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("key:{vk_code}:{scan_code}:{state:?}"));
        Ok(())
    }
}

#[test]
fn play_actions_dispatches_steps_to_executor() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let settings = PlaybackSettings::new(1, 1000.0).expect("settings");
    let plan = build_playback_plan(&recording(), settings);
    let token = StopToken::default();

    play_actions(&plan, &fake, &token).expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["key:65:30:Pressed", "key:65:30:Released"]
    );
}
