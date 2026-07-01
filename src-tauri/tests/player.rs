use remember_lib::model::{ButtonState, KeyState, MacroStep, MouseButton, Recording};
use remember_lib::player::{
    build_playback_plan, play_actions, scaled_delay_ms, PlaybackAction, PlaybackSettings,
    StepExecutor, StopToken,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
    fail_on_call: Arc<Mutex<Option<usize>>>,
}

impl FakeExecutor {
    fn failing_on(call_number: usize) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_on_call: Arc::new(Mutex::new(Some(call_number))),
        }
    }

    fn record_call(&self, call: String) -> Result<(), String> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(call);
        let call_number = calls.len();
        drop(calls);

        let should_fail = self
            .fail_on_call
            .lock()
            .unwrap()
            .map(|fail_on_call| fail_on_call == call_number)
            .unwrap_or(false);

        if should_fail {
            Err("executor failed".to_string())
        } else {
            Ok(())
        }
    }
}

impl StepExecutor for FakeExecutor {
    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        self.record_call(format!("move:{x}:{y}"))
    }

    fn mouse_button(
        &self,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> Result<(), String> {
        self.record_call(format!("button:{x}:{y}:{button:?}:{state:?}"))
    }

    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String> {
        self.record_call(format!("wheel:{x}:{y}:{delta}"))
    }

    fn key(&self, vk_code: u16, scan_code: u16, state: KeyState) -> Result<(), String> {
        self.record_call(format!("key:{vk_code}:{scan_code}:{state:?}"))
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

#[test]
fn delayed_action_can_be_stopped_before_full_delay() {
    let plan = vec![PlaybackAction {
        loop_index: 0,
        step_index: 0,
        delay_ms: 1_000,
        step: MacroStep::Wait { elapsed_ms: 1_000 },
    }];
    let token = StopToken::default();
    let play_token = token.clone();

    let started = Instant::now();
    let handle = thread::spawn(move || {
        let fake = FakeExecutor::default();
        play_actions(&plan, &fake, &play_token)
    });
    thread::sleep(Duration::from_millis(50));
    token.request_stop();

    let result = handle.join().unwrap();
    let elapsed = started.elapsed();

    assert_eq!(result, Err("playback stopped".to_string()));
    assert!(
        elapsed < Duration::from_millis(400),
        "stop should interrupt delay promptly, elapsed: {elapsed:?}"
    );
}

#[test]
fn executor_error_after_key_press_releases_key_before_returning_error() {
    let fake = FakeExecutor::failing_on(2);
    let calls = fake.calls.clone();
    let token = StopToken::default();
    let plan = vec![
        PlaybackAction {
            loop_index: 0,
            step_index: 0,
            delay_ms: 0,
            step: MacroStep::Key {
                elapsed_ms: 0,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Pressed,
            },
        },
        PlaybackAction {
            loop_index: 0,
            step_index: 1,
            delay_ms: 0,
            step: MacroStep::MouseMove {
                elapsed_ms: 1,
                x: 10,
                y: 20,
            },
        },
    ];

    let result = play_actions(&plan, &fake, &token);

    assert_eq!(result, Err("executor failed".to_string()));
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["key:65:30:Pressed", "move:10:20", "key:65:30:Released"]
    );
}

#[test]
fn normal_completion_releases_tracked_key_presses() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let token = StopToken::default();
    let plan = vec![PlaybackAction {
        loop_index: 0,
        step_index: 0,
        delay_ms: 0,
        step: MacroStep::Key {
            elapsed_ms: 0,
            vk_code: 0x41,
            scan_code: 0x1E,
            state: KeyState::Pressed,
        },
    }];

    play_actions(&plan, &fake, &token).expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["key:65:30:Pressed", "key:65:30:Released"]
    );
}

#[test]
fn normal_completion_releases_tracked_mouse_presses() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let token = StopToken::default();
    let plan = vec![PlaybackAction {
        loop_index: 0,
        step_index: 0,
        delay_ms: 0,
        step: MacroStep::MouseButton {
            elapsed_ms: 0,
            x: 42,
            y: 84,
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        },
    }];

    play_actions(&plan, &fake, &token).expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["button:42:84:Left:Pressed", "button:42:84:Left:Released"]
    );
}

#[test]
fn stop_after_mouse_button_press_releases_button_at_press_coordinates() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let token = StopToken::default();
    let play_token = token.clone();
    let plan = vec![
        PlaybackAction {
            loop_index: 0,
            step_index: 0,
            delay_ms: 0,
            step: MacroStep::MouseButton {
                elapsed_ms: 0,
                x: 42,
                y: 84,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            },
        },
        PlaybackAction {
            loop_index: 0,
            step_index: 1,
            delay_ms: 1_000,
            step: MacroStep::Wait { elapsed_ms: 1_000 },
        },
    ];

    let handle = thread::spawn(move || play_actions(&plan, &fake, &play_token));
    thread::sleep(Duration::from_millis(50));
    token.request_stop();

    let result = handle.join().unwrap();

    assert_eq!(result, Err("playback stopped".to_string()));
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["button:42:84:Left:Pressed", "button:42:84:Left:Released"]
    );
}
