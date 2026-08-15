use remember_lib::model::{
    ButtonState, ClientSize, ControlBounds, KeyState, MacroStep, MouseButton, PointerPosition,
    PointerSemanticAction, Recording, TargetWindowAvailability, TargetWindowId,
    WindowPointerIntent, WindowTarget,
};
use remember_lib::player::{
    play_actions, play_recording, scaled_delay_ms, PlaybackAction, PlaybackSettings, StepExecutor,
    StopToken,
};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
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

fn window_target(id: u32, availability: TargetWindowAvailability) -> WindowTarget {
    WindowTarget {
        id: TargetWindowId(id),
        executable_path: format!(r"C:\Apps\Target{id}.exe"),
        window_class: format!("TargetWindow{id}"),
        title: format!("Target {id}"),
        client_size: ClientSize {
            width: 800,
            height: 600,
        },
        dpi: 96,
        availability,
    }
}

#[test]
fn validates_loop_count_speed_and_loop_delay() {
    assert!(PlaybackSettings::new(Some(1), 1.0).is_ok());
    assert!(PlaybackSettings::new(None, 1.0).is_ok());
    assert!(PlaybackSettings::new(Some(0), 1.0).is_err());
    assert!(PlaybackSettings::new(Some(1), 0.0).is_err());
    assert_eq!(
        PlaybackSettings::new_with_loop_delay(Some(2), 1.0, 250)
            .expect("settings")
            .loop_delay_ms,
        250
    );
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
    window_pauses: Arc<Mutex<VecDeque<u64>>>,
}

impl FakeExecutor {
    fn failing_on(call_number: usize) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_on_call: Arc::new(Mutex::new(Some(call_number))),
            stop_on_call: None,
            window_pauses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn stopping_on(call_number: usize, stop_token: StopToken) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_on_call: Arc::new(Mutex::new(None)),
            stop_on_call: Some((call_number, stop_token)),
            window_pauses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn with_window_pauses(pauses: impl IntoIterator<Item = u64>) -> Self {
        Self {
            window_pauses: Arc::new(Mutex::new(pauses.into_iter().collect())),
            ..Self::default()
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

    fn record_window_call(&self, call: String) -> Result<u64, String> {
        self.record_call(call)?;
        Ok(self.window_pauses.lock().unwrap().pop_front().unwrap_or(0))
    }
}

impl StepExecutor for FakeExecutor {
    fn prepare_window_relative_playback(&self) -> Result<(), String> {
        self.record_call("prepare".to_string())
    }

    fn begin_window_relative_loop(&self, targets: &[WindowTarget]) -> Result<(), String> {
        self.record_call(format!(
            "loop:{}",
            targets
                .iter()
                .map(|target| target.id.0.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ))
    }

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

    fn window_mouse_move(
        &self,
        target: &WindowTarget,
        x: i32,
        y: i32,
        intent: WindowPointerIntent,
        input_held: bool,
    ) -> Result<u64, String> {
        self.record_window_call(format!(
            "window-move:{}:{x}:{y}:{intent:?}:held={input_held}",
            target.id.0
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn window_mouse_button(
        &self,
        target: &WindowTarget,
        x: i32,
        y: i32,
        intent: WindowPointerIntent,
        button: MouseButton,
        state: ButtonState,
        input_held: bool,
    ) -> Result<u64, String> {
        self.record_window_call(format!(
            "window-button:{}:{x}:{y}:{intent:?}:{button:?}:{state:?}:held={input_held}",
            target.id.0
        ))
    }

    fn window_mouse_wheel(
        &self,
        target: &WindowTarget,
        x: i32,
        y: i32,
        intent: WindowPointerIntent,
        delta: i32,
        input_held: bool,
    ) -> Result<u64, String> {
        self.record_window_call(format!(
            "window-wheel:{}:{x}:{y}:{intent:?}:{delta}:held={input_held}",
            target.id.0
        ))
    }

    fn window_select_combo_option(
        &self,
        target: &WindowTarget,
        control_id: i32,
        control_bounds: ControlBounds,
        option_name: &str,
        input_held: bool,
    ) -> Result<u64, String> {
        self.record_window_call(format!(
            "window-select:{}:{control_id}:{}:{}:{}:{}:{option_name}:held={input_held}",
            target.id.0,
            control_bounds.x,
            control_bounds.y,
            control_bounds.width,
            control_bounds.height,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn targeted_key(
        &self,
        target: &WindowTarget,
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
        sequence_start: bool,
        input_held: bool,
    ) -> Result<u64, String> {
        self.record_window_call(format!(
            "target-key:{}:{vk_code}:{scan_code}:{extended}:{state:?}:{sequence_start}:held={input_held}",
            target.id.0
        ))
    }

    fn release_mouse_button(&self, button: MouseButton) -> Result<(), String> {
        self.record_call(format!("release-button:{button:?}"))
    }
}

#[test]
fn semantic_combo_click_selects_by_name_once_without_holding_a_mouse_button() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let action = PointerSemanticAction::SelectComboOption {
        control_id: 1_042,
        control_bounds: ControlBounds {
            x: 70,
            y: 220,
            width: 493,
            height: 31,
        },
        option_name: "Mihomo".to_string(),
    };
    let recording = Recording::new_window_relative(
        "semantic combo",
        "2026-08-15T00:00:00Z",
        vec![window_target(7, TargetWindowAvailability::Deferred)],
        vec![
            MacroStep::PointerButton {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(7),
                    x: 126,
                    y: 297,
                    intent: WindowPointerIntent::Foreground,
                },
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                semantic_action: Some(action.clone()),
            },
            MacroStep::PointerButton {
                elapsed_ms: 1,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(7),
                    x: 126,
                    y: 297,
                    intent: WindowPointerIntent::Foreground,
                },
                button: MouseButton::Left,
                state: ButtonState::Released,
                semantic_action: Some(action),
            },
        ],
    );

    play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    )
    .expect("semantic playback");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "prepare",
            "loop:7",
            "window-select:7:1042:70:220:493:31:Mihomo:held=false",
        ]
    );
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
        Err("回放循环结束时释放残留输入失败：executor failed".to_string())
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
        targets: Vec::new(),
        steps: vec![MacroStep::Wait { elapsed_ms: 0 }],
    };
    let settings = PlaybackSettings::new(Some(2), 1.0).expect("settings");
    let started = Instant::now();

    play_recording(&recording, settings, &fake, &StopToken::default()).expect("play");

    assert!(started.elapsed() >= Duration::from_millis(70));
}

#[test]
fn loop_delay_is_unscaled_and_waits_between_repeated_loops() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let recording = Recording::new(
        "loop delay",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::MouseMove {
            elapsed_ms: 0,
            x: 1,
            y: 2,
        }],
    );
    let settings = PlaybackSettings::new_with_loop_delay(Some(2), 1000.0, 35).expect("settings");
    let started = Instant::now();

    play_recording(&recording, settings, &fake, &StopToken::default()).expect("play");

    assert!(
        started.elapsed() >= Duration::from_millis(30),
        "the configured wall-clock delay must not be divided by playback speed"
    );
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[test]
fn loop_delay_can_be_stopped_without_waiting_for_the_next_loop() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let recording = Recording::new(
        "stoppable loop delay",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::MouseMove {
            elapsed_ms: 0,
            x: 1,
            y: 2,
        }],
    );
    let token = StopToken::default();
    let play_token = token.clone();
    let handle = thread::spawn(move || {
        play_recording(
            &recording,
            PlaybackSettings::new_with_loop_delay(Some(2), 1.0, 30_000).expect("settings"),
            &fake,
            &play_token,
        )
    });

    let first_loop_deadline = Instant::now() + Duration::from_secs(1);
    while calls.lock().unwrap().is_empty() && Instant::now() < first_loop_deadline {
        thread::yield_now();
    }
    assert_eq!(calls.lock().unwrap().len(), 1);

    let stop_requested = Instant::now();
    token.request_stop();
    assert_eq!(handle.join().unwrap(), Err("playback stopped".to_string()));
    assert!(
        stop_requested.elapsed() < Duration::from_millis(250),
        "stop should interrupt the delay before the next loop"
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
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

#[test]
fn v2_screen_relative_steps_use_legacy_executor_methods() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let recording = Recording::new_window_relative(
        "screen actions",
        "2026-06-29T00:00:00Z",
        Vec::new(),
        vec![
            MacroStep::PointerMove {
                elapsed_ms: 0,
                position: PointerPosition::ScreenRelative { x: 10, y: 20 },
            },
            MacroStep::PointerButton {
                elapsed_ms: 0,
                position: PointerPosition::ScreenRelative { x: 11, y: 21 },
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                semantic_action: None,
            },
            MacroStep::PointerButton {
                elapsed_ms: 0,
                position: PointerPosition::ScreenRelative { x: 11, y: 21 },
                button: MouseButton::Left,
                state: ButtonState::Released,
                semantic_action: None,
            },
            MacroStep::PointerWheel {
                elapsed_ms: 0,
                position: PointerPosition::ScreenRelative { x: 12, y: 22 },
                delta: 120,
            },
            MacroStep::TargetedKey {
                elapsed_ms: 0,
                target_id: None,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::TargetedKey {
                elapsed_ms: 0,
                target_id: None,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Released,
            },
        ],
    );

    play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    )
    .expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "prepare",
            "loop:",
            "move:10:20",
            "button:11:21:Left:Pressed",
            "button:11:21:Left:Released",
            "wheel:12:22:120",
            "key:65:30:false:Pressed",
            "key:65:30:false:Released",
        ]
    );
}

#[test]
fn v2_dispatches_window_actions_with_their_intents() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let target = window_target(7, TargetWindowAvailability::Initial);
    let recording = Recording::new_window_relative(
        "window actions",
        "2026-06-29T00:00:00Z",
        vec![target],
        vec![
            MacroStep::PointerMove {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(7),
                    x: 30,
                    y: 40,
                    intent: WindowPointerIntent::Foreground,
                },
            },
            MacroStep::PointerButton {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(7),
                    x: 31,
                    y: 41,
                    intent: WindowPointerIntent::ActivationClick,
                },
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                semantic_action: None,
            },
            MacroStep::PointerButton {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(7),
                    x: 32,
                    y: 42,
                    intent: WindowPointerIntent::DropRelease,
                },
                button: MouseButton::Left,
                state: ButtonState::Released,
                semantic_action: None,
            },
            MacroStep::PointerWheel {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(7),
                    x: 33,
                    y: 43,
                    intent: WindowPointerIntent::BackgroundWheel,
                },
                delta: -120,
            },
        ],
    );

    play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    )
    .expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "prepare",
            "loop:7",
            "window-move:7:30:40:Foreground:held=false",
            "window-button:7:31:41:ActivationClick:Left:Pressed:held=false",
            "window-button:7:32:42:DropRelease:Left:Released:held=true",
            "window-wheel:7:33:43:BackgroundWheel:-120:held=false",
        ]
    );
}

#[test]
fn v2_prepares_without_prebinding_targets_and_notifies_every_loop() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let recording = Recording::new_window_relative(
        "target lifecycle",
        "2026-06-29T00:00:00Z",
        vec![
            window_target(1, TargetWindowAvailability::Initial),
            window_target(2, TargetWindowAvailability::Deferred),
        ],
        vec![MacroStep::Wait { elapsed_ms: 0 }],
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
        ["prepare", "loop:1,2", "loop:1,2"]
    );
}

#[test]
fn targeted_keyboard_marks_only_the_first_press_of_each_sequence() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let target = window_target(3, TargetWindowAvailability::Initial);
    let key = |elapsed_ms, vk_code, state| MacroStep::TargetedKey {
        elapsed_ms,
        target_id: Some(TargetWindowId(3)),
        vk_code,
        scan_code: vk_code,
        extended: false,
        state,
    };
    let recording = Recording::new_window_relative(
        "keyboard sequences",
        "2026-06-29T00:00:00Z",
        vec![target],
        vec![
            key(0, 0x41, KeyState::Pressed),
            key(0, 0x12, KeyState::Pressed),
            key(0, 0x41, KeyState::Released),
            key(0, 0x12, KeyState::Released),
            key(0, 0x42, KeyState::Pressed),
            key(0, 0x42, KeyState::Released),
        ],
    );

    play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    )
    .expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "prepare",
            "loop:3",
            "target-key:3:65:65:false:Pressed:true:held=false",
            "target-key:3:18:18:false:Pressed:false:held=true",
            "target-key:3:65:65:false:Released:false:held=true",
            "target-key:3:18:18:false:Released:false:held=true",
            "target-key:3:66:66:false:Pressed:true:held=false",
            "target-key:3:66:66:false:Released:false:held=true",
        ]
    );
}

#[test]
fn window_binding_pause_shifts_the_remaining_loop_timeline() {
    let fake = FakeExecutor::with_window_pauses([40, 0]);
    let target = window_target(4, TargetWindowAvailability::Deferred);
    let recording = Recording::new_window_relative(
        "binding pause",
        "2026-06-29T00:00:00Z",
        vec![target],
        vec![
            MacroStep::PointerMove {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(4),
                    x: 1,
                    y: 2,
                    intent: WindowPointerIntent::Foreground,
                },
            },
            MacroStep::PointerMove {
                elapsed_ms: 10,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(4),
                    x: 3,
                    y: 4,
                    intent: WindowPointerIntent::Foreground,
                },
            },
        ],
    );
    let started = Instant::now();

    play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    )
    .expect("play");

    assert!(
        started.elapsed() >= Duration::from_millis(45),
        "reported binding time must postpone subsequent steps"
    );
}

#[test]
fn v2_executor_error_releases_a_targeted_key_with_safe_generic_cleanup() {
    let fake = FakeExecutor::failing_on(4);
    let calls = fake.calls.clone();
    let target = window_target(5, TargetWindowAvailability::Initial);
    let recording = Recording::new_window_relative(
        "targeted cleanup",
        "2026-06-29T00:00:00Z",
        vec![target],
        vec![
            MacroStep::TargetedKey {
                elapsed_ms: 0,
                target_id: Some(TargetWindowId(5)),
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::PointerMove {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(5),
                    x: 9,
                    y: 10,
                    intent: WindowPointerIntent::Foreground,
                },
            },
        ],
    );

    let result = play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).expect("settings"),
        &fake,
        &StopToken::default(),
    );

    assert_eq!(result, Err("executor failed".to_string()));
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "prepare",
            "loop:5",
            "target-key:5:65:30:false:Pressed:true:held=false",
            "window-move:5:9:10:Foreground:held=true",
            "key:65:30:false:Released",
        ]
    );
}

#[derive(Default)]
struct TargetBindingFailureExecutor {
    calls: Arc<Mutex<Vec<String>>>,
    binding_attempts: Arc<AtomicUsize>,
}

impl TargetBindingFailureExecutor {
    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

impl StepExecutor for TargetBindingFailureExecutor {
    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        self.record(format!("move:{x}:{y}"));
        Ok(())
    }

    fn mouse_button(
        &self,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> Result<(), String> {
        self.record(format!("button:{x}:{y}:{button:?}:{state:?}"));
        Ok(())
    }

    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String> {
        self.record(format!("wheel:{x}:{y}:{delta}"));
        Ok(())
    }

    fn key(
        &self,
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
    ) -> Result<(), String> {
        self.record(format!("key:{vk_code}:{scan_code}:{extended}:{state:?}"));
        Ok(())
    }

    fn window_mouse_move(
        &self,
        target: &WindowTarget,
        x: i32,
        y: i32,
        _intent: WindowPointerIntent,
        input_held: bool,
    ) -> Result<u64, String> {
        self.record(format!(
            "window-target-move:{}:{x}:{y}:held={input_held}",
            target.id.0
        ));
        self.binding_attempts.fetch_add(1, Ordering::SeqCst);
        Err("automatic window binding failed".to_string())
    }

    fn release_mouse_button(&self, button: MouseButton) -> Result<(), String> {
        self.record(format!("release-button:{button:?}"));
        Ok(())
    }
}

#[test]
fn automatic_target_binding_failure_during_drag_releases_the_mouse() {
    let fake = TargetBindingFailureExecutor::default();
    let calls = fake.calls.clone();
    let binding_attempts = fake.binding_attempts.clone();
    let recording = Recording::new_window_relative(
        "drag target binding failure",
        "2026-06-29T00:00:00Z",
        vec![window_target(8, TargetWindowAvailability::Deferred)],
        vec![
            MacroStep::PointerButton {
                elapsed_ms: 0,
                position: PointerPosition::ScreenRelative { x: 1, y: 2 },
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                semantic_action: None,
            },
            MacroStep::PointerMove {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(8),
                    x: 3,
                    y: 4,
                    intent: WindowPointerIntent::Foreground,
                },
            },
        ],
    );

    let result = play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).unwrap(),
        &fake,
        &StopToken::default(),
    );

    assert_eq!(result, Err("automatic window binding failed".to_string()));
    assert_eq!(binding_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "button:1:2:Left:Pressed",
            "window-target-move:8:3:4:held=true",
            "release-button:Left",
        ]
    );
}

#[test]
fn automatic_target_binding_failure_while_a_key_is_held_releases_the_key() {
    let fake = TargetBindingFailureExecutor::default();
    let calls = fake.calls.clone();
    let binding_attempts = fake.binding_attempts.clone();
    let recording = Recording::new_window_relative(
        "keyboard target binding failure",
        "2026-06-29T00:00:00Z",
        vec![window_target(9, TargetWindowAvailability::Deferred)],
        vec![
            MacroStep::TargetedKey {
                elapsed_ms: 0,
                target_id: None,
                vk_code: 0x11,
                scan_code: 0x1D,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::PointerMove {
                elapsed_ms: 0,
                position: PointerPosition::WindowRelative {
                    target_id: TargetWindowId(9),
                    x: 5,
                    y: 6,
                    intent: WindowPointerIntent::Foreground,
                },
            },
        ],
    );

    let result = play_recording(
        &recording,
        PlaybackSettings::new(Some(1), 1.0).unwrap(),
        &fake,
        &StopToken::default(),
    );

    assert_eq!(result, Err("automatic window binding failed".to_string()));
    assert_eq!(binding_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "key:17:29:false:Pressed",
            "window-target-move:9:5:6:held=true",
            "key:17:29:false:Released",
        ]
    );
}
