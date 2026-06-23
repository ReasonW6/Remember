use remember_lib::playback_input::{PlaybackInput, PlaybackMouseButton};
use remember_lib::player::{PlaybackFinishReason, PlaybackOptions, PlayerState};
use remember_lib::storage::{Flow, FlowStep, TargetWindow};
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

#[test]
fn wait_playback_applies_speed_and_loop_count_until_finished() {
    let mut player = PlayerState::default();
    let (sender, receiver) = mpsc::channel();

    let started = player
        .start(
            wait_only_flow(20),
            PlaybackOptions {
                speed_multiplier: 2.0,
                loop_count: 2,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("wait playback should start");

    assert!(player.is_playing());
    assert_eq!(started.status, "playing");
    assert_eq!(started.loop_count, 2);

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("wait playback should finish");

    assert_eq!(finished.reason, PlaybackFinishReason::Completed);
    assert_eq!(finished.completed_steps, 2);
    assert_eq!(finished.skipped_steps, 0);
    assert_eq!(finished.loop_count, 2);
    assert!(!player.is_playing());
}

#[test]
fn mixed_playback_replays_click_type_and_hotkey_steps() {
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(FakePlaybackInput {
        active_window: target_window(),
        clicks: Arc::clone(&clicks),
        typed_texts: Arc::clone(&typed_texts),
        hotkeys: Arc::clone(&hotkeys),
        scrolls: Arc::clone(&scrolls),
    }));
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            mixed_flow(),
            PlaybackOptions {
                speed_multiplier: 5.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("mixed playback should start");

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("mixed playback should finish");

    assert_eq!(finished.reason, PlaybackFinishReason::Completed);
    assert_eq!(finished.completed_steps, 5);
    assert_eq!(finished.skipped_steps, 0);
    assert_eq!(
        *clicks.lock().expect("clicks should lock"),
        vec![RecordedClick {
            button: PlaybackMouseButton::Left,
            x: 100,
            y: 200,
        }]
    );
    assert_eq!(
        *typed_texts.lock().expect("typed texts should lock"),
        vec!["safe".to_string()]
    );
    assert_eq!(
        *hotkeys.lock().expect("hotkeys should lock"),
        vec![vec!["Ctrl".to_string(), "S".to_string()]]
    );
    assert_eq!(
        *scrolls.lock().expect("scrolls should lock"),
        vec![RecordedScroll {
            delta_x: 0,
            delta_y: -120,
        }]
    );
    assert!(finished.message.contains("步骤"));
}

#[test]
fn click_playback_safety_stops_when_flow_target_is_unmatched() {
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(FakePlaybackInput {
        active_window: target_window(),
        clicks: Arc::clone(&clicks),
        typed_texts,
        hotkeys,
        scrolls,
    }));
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            click_only_flow(unmatched_target_window()),
            PlaybackOptions {
                speed_multiplier: 5.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("click playback should start");

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("unsafe click playback should stop");

    assert_eq!(finished.completed_steps, 0);
    assert_eq!(finished.skipped_steps, 1);
    assert!(finished.message.contains("安全停止"));
    assert!(finished.message.contains("目标窗口"));
    assert!(clicks.lock().expect("clicks should lock").is_empty());
}

#[test]
fn click_playback_safety_stops_when_active_window_differs() {
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(FakePlaybackInput {
        active_window: TargetWindow {
            title: "Different".to_string(),
            process: "different.exe".to_string(),
            size: "800 x 600".to_string(),
            matched: true,
        },
        clicks: Arc::clone(&clicks),
        typed_texts,
        hotkeys,
        scrolls,
    }));
    let (sender, receiver) = mpsc::channel();

    let started = player
        .start(
            click_only_flow(target_window()),
            PlaybackOptions {
                speed_multiplier: 5.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("click playback should start");

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("mismatched click playback should stop");

    assert_eq!(finished.run_id, started.run_id);
    assert_eq!(finished.completed_steps, 0);
    assert_eq!(finished.skipped_steps, 1);
    assert!(finished.message.contains("安全停止"));
    assert!(finished.message.contains("不同"));
    assert!(clicks.lock().expect("clicks should lock").is_empty());
}

#[test]
fn input_playback_checks_target_after_step_delay() {
    let active_window = Arc::new(Mutex::new(TargetWindow {
        title: "Remember".to_string(),
        process: "remember.exe".to_string(),
        size: "536 x 209".to_string(),
        matched: true,
    }));
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(SwitchingPlaybackInput {
        active_window: Arc::clone(&active_window),
        clicks: Arc::clone(&clicks),
        typed_texts,
        hotkeys,
        scrolls,
    }));
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            click_only_flow(target_window()),
            PlaybackOptions {
                speed_multiplier: 1.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("delayed click playback should start");

    thread::sleep(Duration::from_millis(3));
    *active_window.lock().expect("active window should lock") = target_window();

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("delayed click playback should finish");

    assert_eq!(finished.reason, PlaybackFinishReason::Completed);
    assert_eq!(
        *clicks.lock().expect("clicks should lock"),
        vec![RecordedClick {
            button: PlaybackMouseButton::Left,
            x: 100,
            y: 200,
        }]
    );
}

#[test]
fn type_playback_safety_stops_when_active_window_differs() {
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(FakePlaybackInput {
        active_window: TargetWindow {
            title: "Different".to_string(),
            process: "different.exe".to_string(),
            size: "800 x 600".to_string(),
            matched: true,
        },
        clicks,
        typed_texts: Arc::clone(&typed_texts),
        hotkeys,
        scrolls,
    }));
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            type_only_flow(target_window()),
            PlaybackOptions {
                speed_multiplier: 5.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("type playback should start");

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("mismatched type playback should stop");

    assert_eq!(finished.reason, PlaybackFinishReason::SafetyStopped);
    assert_eq!(finished.completed_steps, 0);
    assert_eq!(finished.skipped_steps, 1);
    assert!(finished.message.contains("安全停止"));
    assert!(finished.message.contains("不同"));
    assert!(typed_texts
        .lock()
        .expect("typed texts should lock")
        .is_empty());
}

#[test]
fn hotkey_playback_safety_stops_when_active_window_differs() {
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(FakePlaybackInput {
        active_window: TargetWindow {
            title: "Different".to_string(),
            process: "different.exe".to_string(),
            size: "800 x 600".to_string(),
            matched: true,
        },
        clicks,
        typed_texts,
        hotkeys: Arc::clone(&hotkeys),
        scrolls,
    }));
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            hotkey_only_flow(target_window(), 10),
            PlaybackOptions {
                speed_multiplier: 5.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("hotkey playback should start");

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("mismatched hotkey playback should stop");

    assert_eq!(finished.reason, PlaybackFinishReason::SafetyStopped);
    assert_eq!(finished.completed_steps, 0);
    assert_eq!(finished.skipped_steps, 1);
    assert!(finished.message.contains("安全停止"));
    assert!(finished.message.contains("不同"));
    assert!(hotkeys.lock().expect("hotkeys should lock").is_empty());
}

#[test]
fn stop_interrupts_hotkey_playback_before_sending_keys() {
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(FakePlaybackInput {
        active_window: target_window(),
        clicks,
        typed_texts,
        hotkeys: Arc::clone(&hotkeys),
        scrolls,
    }));
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            hotkey_only_flow(target_window(), 800),
            PlaybackOptions {
                speed_multiplier: 1.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("hotkey playback should start");

    thread::sleep(Duration::from_millis(30));
    let stopped = player.stop().expect("playback should stop");

    assert_eq!(stopped.status, "stopped");

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("stop should interrupt hotkey playback");

    assert_eq!(finished.reason, PlaybackFinishReason::Stopped);
    assert_eq!(finished.completed_steps, 0);
    assert!(hotkeys.lock().expect("hotkeys should lock").is_empty());
}

#[test]
fn scroll_playback_safety_stops_when_active_window_differs() {
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(FakePlaybackInput {
        active_window: TargetWindow {
            title: "Different".to_string(),
            process: "different.exe".to_string(),
            size: "800 x 600".to_string(),
            matched: true,
        },
        clicks,
        typed_texts,
        hotkeys,
        scrolls: Arc::clone(&scrolls),
    }));
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            scroll_only_flow(target_window(), 10),
            PlaybackOptions {
                speed_multiplier: 5.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("scroll playback should start");

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("mismatched scroll playback should stop");

    assert_eq!(finished.reason, PlaybackFinishReason::SafetyStopped);
    assert_eq!(finished.completed_steps, 0);
    assert_eq!(finished.skipped_steps, 1);
    assert!(finished.message.contains("安全停止"));
    assert!(finished.message.contains("不同"));
    assert!(scrolls.lock().expect("scrolls should lock").is_empty());
}

#[test]
fn stop_interrupts_scroll_playback_before_sending_wheel() {
    let clicks = Arc::new(Mutex::new(Vec::new()));
    let typed_texts = Arc::new(Mutex::new(Vec::new()));
    let hotkeys = Arc::new(Mutex::new(Vec::new()));
    let scrolls = Arc::new(Mutex::new(Vec::new()));
    let mut player = PlayerState::with_input(Arc::new(FakePlaybackInput {
        active_window: target_window(),
        clicks,
        typed_texts,
        hotkeys,
        scrolls: Arc::clone(&scrolls),
    }));
    let (sender, receiver) = mpsc::channel();

    player
        .start(
            scroll_only_flow(target_window(), 800),
            PlaybackOptions {
                speed_multiplier: 1.0,
                loop_count: 1,
            },
            move |payload| sender.send(payload).expect("finished payload should send"),
        )
        .expect("scroll playback should start");

    thread::sleep(Duration::from_millis(30));
    let stopped = player.stop().expect("playback should stop");

    assert_eq!(stopped.status, "stopped");

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("stop should interrupt scroll playback");

    assert_eq!(finished.reason, PlaybackFinishReason::Stopped);
    assert_eq!(finished.completed_steps, 0);
    assert!(scrolls.lock().expect("scrolls should lock").is_empty());
}

#[test]
fn stop_interrupts_long_wait_playback() {
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
        .expect("long wait playback should start");

    thread::sleep(Duration::from_millis(30));
    let stopped = player.stop().expect("playback should stop");

    assert_eq!(stopped.status, "stopped");
    assert!(stopped.message.contains("停止"));

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("stop should interrupt wait playback");

    assert_eq!(finished.reason, PlaybackFinishReason::Stopped);
    assert_eq!(finished.completed_steps, 0);
    assert!(!player.is_playing());
}

#[test]
fn emergency_stop_interrupts_with_emergency_reason() {
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
        .expect("long wait playback should start");

    thread::sleep(Duration::from_millis(30));
    let stopped = player
        .emergency_stop()
        .expect("playback should emergency stop");

    assert_eq!(stopped.status, "stopped");
    assert!(stopped.message.contains("紧急停止"));

    let finished = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("emergency stop should interrupt wait playback");

    assert_eq!(finished.reason, PlaybackFinishReason::EmergencyStopped);
    assert_eq!(finished.completed_steps, 0);
    assert!(!player.is_playing());
}

#[test]
fn playback_rejects_zero_loop_count_until_infinite_loop_confirmation_exists() {
    let mut player = PlayerState::default();
    let (sender, _receiver) = mpsc::channel();

    let result = player.start(
        wait_only_flow(1),
        PlaybackOptions {
            speed_multiplier: 1.0,
            loop_count: 0,
        },
        move |payload| sender.send(payload).expect("finished payload should send"),
    );

    assert!(result.is_err());
    assert!(!player.is_playing());
}

fn wait_only_flow(duration_ms: u64) -> Flow {
    Flow {
        version: 1,
        name: "wait-only".to_string(),
        display_name: "Wait Only".to_string(),
        target_window: target_window(),
        steps: vec![FlowStep::Wait {
            id: 1,
            action: "等待".to_string(),
            duration_ms,
            delay_ms: duration_ms,
            note: "safe wait".to_string(),
        }],
    }
}

fn mixed_flow() -> Flow {
    Flow {
        version: 1,
        name: "mixed".to_string(),
        display_name: "Mixed".to_string(),
        target_window: target_window(),
        steps: vec![
            FlowStep::Click {
                id: 1,
                action: "左键单击".to_string(),
                target: "(100, 200) [屏幕绝对]".to_string(),
                x: 100,
                y: 200,
                delay_ms: 10,
                note: "not replayed yet".to_string(),
            },
            FlowStep::Type {
                id: 2,
                action: "文本输入".to_string(),
                text: "safe".to_string(),
                delay_ms: 10,
                note: "not replayed yet".to_string(),
            },
            FlowStep::Hotkey {
                id: 3,
                action: "快捷键".to_string(),
                keys: vec!["Ctrl".to_string(), "S".to_string()],
                delay_ms: 10,
                note: "not replayed yet".to_string(),
            },
            FlowStep::Scroll {
                id: 4,
                action: "滚动".to_string(),
                delta_x: 0,
                delta_y: -120,
                delay_ms: 10,
                note: "safe scroll".to_string(),
            },
            FlowStep::Wait {
                id: 5,
                action: "等待".to_string(),
                duration_ms: 20,
                delay_ms: 20,
                note: "safe wait".to_string(),
            },
        ],
    }
}

fn click_only_flow(target_window: TargetWindow) -> Flow {
    Flow {
        version: 1,
        name: "click-only".to_string(),
        display_name: "Click Only".to_string(),
        target_window,
        steps: vec![FlowStep::Click {
            id: 1,
            action: "左键单击".to_string(),
            target: "(100, 200) [屏幕绝对]".to_string(),
            x: 100,
            y: 200,
            delay_ms: 10,
            note: "safe click".to_string(),
        }],
    }
}

fn type_only_flow(target_window: TargetWindow) -> Flow {
    Flow {
        version: 1,
        name: "type-only".to_string(),
        display_name: "Type Only".to_string(),
        target_window,
        steps: vec![FlowStep::Type {
            id: 1,
            action: "文本输入".to_string(),
            text: "safe".to_string(),
            delay_ms: 10,
            note: "safe type".to_string(),
        }],
    }
}

fn hotkey_only_flow(target_window: TargetWindow, delay_ms: u64) -> Flow {
    Flow {
        version: 1,
        name: "hotkey-only".to_string(),
        display_name: "Hotkey Only".to_string(),
        target_window,
        steps: vec![FlowStep::Hotkey {
            id: 1,
            action: "快捷键".to_string(),
            keys: vec!["Ctrl".to_string(), "S".to_string()],
            delay_ms,
            note: "safe hotkey".to_string(),
        }],
    }
}

fn scroll_only_flow(target_window: TargetWindow, delay_ms: u64) -> Flow {
    Flow {
        version: 1,
        name: "scroll-only".to_string(),
        display_name: "Scroll Only".to_string(),
        target_window,
        steps: vec![FlowStep::Scroll {
            id: 1,
            action: "滚动".to_string(),
            delta_x: 0,
            delta_y: -120,
            delay_ms,
            note: "safe scroll".to_string(),
        }],
    }
}

fn target_window() -> TargetWindow {
    TargetWindow {
        title: "Test".to_string(),
        process: "test.exe".to_string(),
        size: "800 x 600".to_string(),
        matched: true,
    }
}

fn unmatched_target_window() -> TargetWindow {
    TargetWindow {
        title: String::new(),
        process: "N/A".to_string(),
        size: "N/A".to_string(),
        matched: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordedClick {
    button: PlaybackMouseButton,
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordedScroll {
    delta_x: i32,
    delta_y: i32,
}

struct FakePlaybackInput {
    active_window: TargetWindow,
    clicks: Arc<Mutex<Vec<RecordedClick>>>,
    typed_texts: Arc<Mutex<Vec<String>>>,
    hotkeys: Arc<Mutex<Vec<Vec<String>>>>,
    scrolls: Arc<Mutex<Vec<RecordedScroll>>>,
}

impl PlaybackInput for FakePlaybackInput {
    fn active_window_target(&self) -> TargetWindow {
        self.active_window.clone()
    }

    fn click(&self, button: PlaybackMouseButton, x: i32, y: i32) -> Result<(), String> {
        self.clicks
            .lock()
            .expect("clicks should lock")
            .push(RecordedClick { button, x, y });
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<(), String> {
        self.typed_texts
            .lock()
            .expect("typed texts should lock")
            .push(text.to_string());
        Ok(())
    }

    fn press_hotkey(&self, keys: &[String]) -> Result<(), String> {
        self.hotkeys
            .lock()
            .expect("hotkeys should lock")
            .push(keys.to_vec());
        Ok(())
    }

    fn scroll(&self, delta_x: i32, delta_y: i32) -> Result<(), String> {
        self.scrolls
            .lock()
            .expect("scrolls should lock")
            .push(RecordedScroll { delta_x, delta_y });
        Ok(())
    }
}

struct SwitchingPlaybackInput {
    active_window: Arc<Mutex<TargetWindow>>,
    clicks: Arc<Mutex<Vec<RecordedClick>>>,
    typed_texts: Arc<Mutex<Vec<String>>>,
    hotkeys: Arc<Mutex<Vec<Vec<String>>>>,
    scrolls: Arc<Mutex<Vec<RecordedScroll>>>,
}

impl PlaybackInput for SwitchingPlaybackInput {
    fn active_window_target(&self) -> TargetWindow {
        self.active_window
            .lock()
            .expect("active window should lock")
            .clone()
    }

    fn click(&self, button: PlaybackMouseButton, x: i32, y: i32) -> Result<(), String> {
        self.clicks
            .lock()
            .expect("clicks should lock")
            .push(RecordedClick { button, x, y });
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<(), String> {
        self.typed_texts
            .lock()
            .expect("typed texts should lock")
            .push(text.to_string());
        Ok(())
    }

    fn press_hotkey(&self, keys: &[String]) -> Result<(), String> {
        self.hotkeys
            .lock()
            .expect("hotkeys should lock")
            .push(keys.to_vec());
        Ok(())
    }

    fn scroll(&self, delta_x: i32, delta_y: i32) -> Result<(), String> {
        self.scrolls
            .lock()
            .expect("scrolls should lock")
            .push(RecordedScroll { delta_x, delta_y });
        Ok(())
    }
}
