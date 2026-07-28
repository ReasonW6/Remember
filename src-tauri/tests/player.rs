use remember_lib::model::{ButtonState, KeyState, MacroStep, MouseButton, Recording};
use remember_lib::player::{
    play_actions, play_recording, scaled_delay_ms, PlaybackAction, PlaybackSettings, StepExecutor,
    StopToken,
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
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 250,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Released,
            },
        ],
    )
}

#[test]
fn validates_loop_count_and_speed() {
    assert!(PlaybackSettings::new(Some(1), 1.0).is_ok());
    assert!(PlaybackSettings::new(None, 1.0).is_ok());
    assert!(PlaybackSettings::new(Some(0), 1.0).is_err());
    assert!(PlaybackSettings::new(Some(1), 0.0).is_err());
}

#[test]
fn scales_delay_by_speed_multiplier() {
    assert_eq!(scaled_delay_ms(200, 1.0), 200);
    assert_eq!(scaled_delay_ms(200, 2.0), 100);
    assert_eq!(scaled_delay_ms(200, 0.5), 400);
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
    stop_on_call: Option<(usize, StopToken)>,
}

impl FakeExecutor {
    fn failing_on(call_number: usize) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_on_call: Arc::new(Mutex::new(Some(call_number))),
            stop_on_call: None,
        }
    }

    fn stopping_on(call_number: usize, stop_token: StopToken) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_on_call: Arc::new(Mutex::new(None)),
            stop_on_call: Some((call_number, stop_token)),
        }
    }

    fn record_call(&self, call: String) -> Result<(), String> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(call);
        let call_number = calls.len();
        drop(calls);

        if let Some((stop_on_call, stop_token)) = &self.stop_on_call {
            if *stop_on_call == call_number {
                stop_token.request_stop();
            }
        }

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

    fn key(
        &self,
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
    ) -> Result<(), String> {
        self.record_call(format!("key:{vk_code}:{scan_code}:{extended}:{state:?}"))
    }

    fn release_mouse_button(&self, button: MouseButton) -> Result<(), String> {
        self.record_call(format!("release-button:{button:?}"))
    }
}

#[test]
fn finite_playback_does_not_expand_large_loop_counts() {
    let stop_token = StopToken::default();
    let fake = FakeExecutor::stopping_on(1, stop_token.clone());
    let calls = fake.calls.clone();
    let recording = Recording::new(
        "large loop",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::MouseMove {
            elapsed_ms: 0,
            x: 1,
            y: 2,
        }],
    );
    let settings = PlaybackSettings::new(Some(u32::MAX), 1.0).expect("settings");

    let result = play_recording(&recording, settings, &fake, &stop_token);

    assert_eq!(result, Err("playback stopped".to_string()));
    assert_eq!(calls.lock().unwrap().as_slice(), ["move:1:2"]);
}

#[test]
fn unmatched_inputs_are_released_at_each_loop_boundary() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let recording = Recording::new(
        "unmatched inputs",
        "2026-06-29T00:00:00Z",
        vec![
            MacroStep::Key {
                elapsed_ms: 0,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::MouseButton {
                elapsed_ms: 0,
                x: 10,
                y: 20,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            },
        ],
    );

    play_recording(
        &recording,
        PlaybackSettings::new(Some(2), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    )
    .expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "key:65:30:false:Pressed",
            "button:10:20:Left:Pressed",
            "key:65:30:false:Released",
            "release-button:Left",
            "key:65:30:false:Pressed",
            "button:10:20:Left:Pressed",
            "key:65:30:false:Released",
            "release-button:Left",
        ]
    );
}

#[test]
fn loop_boundary_release_failure_is_returned() {
    let fake = FakeExecutor::failing_on(2);
    let recording = Recording::new(
        "release failure",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::Key {
            elapsed_ms: 0,
            vk_code: 0x41,
            scan_code: 0x1E,
            extended: false,
            state: KeyState::Pressed,
        }],
    );

    let result = play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    );

    assert_eq!(
        result,
        Err("input cleanup failed at playback loop boundary: executor failed".to_string())
    );
}

#[test]
fn zero_duration_repeated_playback_is_throttled() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let recording = Recording::new(
        "zero duration",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::MouseMove {
            elapsed_ms: 0,
            x: 1,
            y: 2,
        }],
    );
    let started = Instant::now();

    play_recording(
        &recording,
        PlaybackSettings::new(Some(3), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    )
    .expect("play");

    assert!(
        started.elapsed() >= Duration::from_millis(15),
        "three zero-duration loops must include two throttle intervals"
    );
    assert_eq!(calls.lock().unwrap().len(), 3);
}

#[test]
fn infinite_playback_runs_until_stopped() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let recording = Recording::new(
        "infinite",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::MouseMove {
            elapsed_ms: 10,
            x: 1,
            y: 2,
        }],
    );
    let token = StopToken::default();
    let play_token = token.clone();

    let handle = thread::spawn(move || {
        play_recording(
            &recording,
            PlaybackSettings::new(None, 1.0).expect("settings"),
            &fake,
            &play_token,
        )
    });
    thread::sleep(Duration::from_millis(45));
    token.request_stop();

    assert_eq!(handle.join().unwrap(), Err("playback stopped".to_string()));
    assert!(calls.lock().unwrap().len() >= 2);
}

#[test]
fn looped_playback_preserves_recorded_trailing_duration() {
    let fake = FakeExecutor::default();
    let recording = Recording {
        version: 1,
        name: "tail".to_string(),
        created_at: "2026-06-29T00:00:00Z".to_string(),
        duration_ms: 40,
        steps: vec![MacroStep::Wait { elapsed_ms: 0 }],
    };
    let settings = PlaybackSettings::new(Some(2), 1.0).expect("settings");
    let started = Instant::now();

    play_recording(&recording, settings, &fake, &StopToken::default()).expect("play");

    assert!(started.elapsed() >= Duration::from_millis(70));
}

#[test]
fn play_actions_dispatches_steps_to_executor() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let token = StopToken::default();

    play_recording(
        &recording(),
        PlaybackSettings::new(Some(1), 1000.0).expect("settings"),
        &fake,
        &token,
    )
    .expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["key:65:30:false:Pressed", "key:65:30:false:Released"]
    );
}

#[test]
fn delayed_action_can_be_stopped_before_full_delay() {
    let plan = vec![PlaybackAction {
        loop_index: 0,
        step_index: 0,
        delay_ms: 30_000,
        step: MacroStep::Wait { elapsed_ms: 30_000 },
    }];
    let token = StopToken::default();
    let play_token = token.clone();

    let handle = thread::spawn(move || {
        let fake = FakeExecutor::default();
        play_actions(&plan, &fake, &play_token)
    });
    thread::sleep(Duration::from_millis(50));
    let stop_requested = Instant::now();
    token.request_stop();

    let result = handle.join().unwrap();
    let stop_elapsed = stop_requested.elapsed();

    assert_eq!(result, Err("playback stopped".to_string()));
    assert!(
        stop_elapsed < Duration::from_millis(250),
        "stop should wake a long delay promptly, elapsed after request: {stop_elapsed:?}"
    );
}

#[test]
fn delayed_action_does_not_execute_before_recorded_delay() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let plan = vec![PlaybackAction {
        loop_index: 0,
        step_index: 0,
        delay_ms: 80,
        step: MacroStep::MouseMove {
            elapsed_ms: 80,
            x: 10,
            y: 20,
        },
    }];
    let started = Instant::now();

    play_actions(&plan, &fake, &StopToken::default()).expect("play");

    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(80),
        "action executed before its recorded delay: {elapsed:?}"
    );
    assert_eq!(calls.lock().unwrap().as_slice(), ["move:10:20"]);
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
                extended: false,
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
        [
            "key:65:30:false:Pressed",
            "move:10:20",
            "key:65:30:false:Released"
        ]
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
            extended: true,
            state: KeyState::Pressed,
        },
    }];

    play_actions(&plan, &fake, &token).expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["key:65:30:true:Pressed", "key:65:30:true:Released"]
    );
}

#[test]
fn normal_completion_returns_synthetic_release_error() {
    let fake = FakeExecutor::failing_on(2);
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
            extended: false,
            state: KeyState::Pressed,
        },
    }];

    let result = play_actions(&plan, &fake, &token);

    assert_eq!(result, Err("executor failed".to_string()));
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["key:65:30:false:Pressed", "key:65:30:false:Released"]
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
        ["button:42:84:Left:Pressed", "release-button:Left"]
    );
}

#[test]
fn stop_after_mouse_button_press_releases_without_moving_cursor() {
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
        ["button:42:84:Left:Pressed", "release-button:Left"]
    );
}
