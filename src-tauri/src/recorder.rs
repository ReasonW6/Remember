use serde::Serialize;
use std::{
    fmt,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::keyboard;
use crate::mouse;
use crate::storage::{Flow, FlowStep, TargetWindow};

pub const RECORDING_SAFETY_WARNING: &str =
    "录制会记录鼠标和键盘操作，请勿在录制期间输入密码、验证码或其他敏感信息。";

#[derive(Debug)]
pub enum RecorderError {
    AlreadyRecording,
    CaptureUnavailable,
    NotRecording,
}

impl fmt::Display for RecorderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRecording => write!(formatter, "recording is already active"),
            Self::CaptureUnavailable => write!(formatter, "recording capture is unavailable"),
            Self::NotRecording => write!(formatter, "recording is not active"),
        }
    }
}

impl std::error::Error for RecorderError {}

#[derive(Debug)]
struct RecordingSession {
    started_at: u64,
    started_at_ms: u64,
    target_window: TargetWindow,
    mouse_clicks: Arc<Mutex<Vec<RecordedMouseClick>>>,
    keyboard_inputs: Arc<Mutex<Vec<RecordedKeyboardInput>>>,
    mouse_capture: Option<mouse::MouseCaptureGuard>,
    keyboard_capture: Option<keyboard::KeyboardCaptureGuard>,
}

#[derive(Debug, Default)]
pub struct RecorderState {
    active_session: Option<RecordingSession>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordedMouseButton {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ScreenRect {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordedMouseClick {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) button: RecordedMouseButton,
    pub(crate) captured_at_ms: u64,
    pub(crate) target_window: Option<TargetWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecordedKeyboardInput {
    Text {
        text: String,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    },
    Hotkey {
        keys: Vec<String>,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecordedAction {
    MouseClick(RecordedMouseClick),
    Text {
        text: String,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    },
    Hotkey {
        keys: Vec<String>,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    },
}

impl RecordedAction {
    fn captured_at_ms(&self) -> u64 {
        match self {
            Self::MouseClick(click) => click.captured_at_ms,
            Self::Text { captured_at_ms, .. } => *captured_at_ms,
            Self::Hotkey { captured_at_ms, .. } => *captured_at_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStartPayload {
    pub status: &'static str,
    pub label: &'static str,
    pub started_at: u64,
    pub warning: &'static str,
    pub safety_warning: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStopPayload {
    pub status: &'static str,
    pub label: &'static str,
    pub started_at: u64,
    pub stopped_at: u64,
    pub flow: Flow,
    pub message: &'static str,
}

impl RecorderState {
    pub fn is_recording(&self) -> bool {
        self.active_session.is_some()
    }

    pub fn start(&mut self) -> Result<RecordingStartPayload, RecorderError> {
        self.start_with_target_window(unknown_target_window())
    }

    pub fn start_with_target_window(
        &mut self,
        target_window: TargetWindow,
    ) -> Result<RecordingStartPayload, RecorderError> {
        if self.active_session.is_some() {
            return Err(RecorderError::AlreadyRecording);
        }

        let started_at_ms = unix_millis();
        let started_at = started_at_ms / 1000;
        self.active_session = Some(RecordingSession {
            started_at,
            started_at_ms,
            target_window,
            mouse_clicks: Arc::new(Mutex::new(Vec::new())),
            keyboard_inputs: Arc::new(Mutex::new(Vec::new())),
            mouse_capture: None,
            keyboard_capture: None,
        });

        Ok(RecordingStartPayload {
            status: "recording",
            label: "录制中",
            started_at,
            warning: "已启动录制会话；当前会捕获鼠标点击、键盘输入和热键。",
            safety_warning: RECORDING_SAFETY_WARNING,
        })
    }

    pub fn active_started_at_ms(&self) -> Option<u64> {
        self.active_session
            .as_ref()
            .map(|session| session.started_at_ms)
    }

    pub fn record_mouse_click_at(
        &mut self,
        x: i32,
        y: i32,
        button: RecordedMouseButton,
        captured_at_ms: u64,
    ) -> Result<(), RecorderError> {
        self.record_mouse_click_at_maybe_target(x, y, button, captured_at_ms, None)
    }

    pub fn record_mouse_click_at_target(
        &mut self,
        x: i32,
        y: i32,
        button: RecordedMouseButton,
        captured_at_ms: u64,
        target_window: TargetWindow,
    ) -> Result<(), RecorderError> {
        self.record_mouse_click_at_maybe_target(x, y, button, captured_at_ms, Some(target_window))
    }

    fn record_mouse_click_at_maybe_target(
        &mut self,
        x: i32,
        y: i32,
        button: RecordedMouseButton,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    ) -> Result<(), RecorderError> {
        let Some(session) = self.active_session.as_mut() else {
            return Err(RecorderError::NotRecording);
        };
        let mut mouse_clicks = session
            .mouse_clicks
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?;
        mouse_clicks.push(RecordedMouseClick {
            x,
            y,
            button,
            captured_at_ms,
            target_window,
        });
        Ok(())
    }

    pub fn record_text_input_at(
        &mut self,
        text: &str,
        captured_at_ms: u64,
    ) -> Result<(), RecorderError> {
        let Some(session) = self.active_session.as_mut() else {
            return Err(RecorderError::NotRecording);
        };
        if text.is_empty() {
            return Ok(());
        }
        let mut keyboard_inputs = session
            .keyboard_inputs
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?;
        keyboard_inputs.push(RecordedKeyboardInput::Text {
            text: text.to_string(),
            captured_at_ms,
            target_window: None,
        });
        Ok(())
    }

    pub fn record_hotkey_at(
        &mut self,
        keys: Vec<String>,
        captured_at_ms: u64,
    ) -> Result<(), RecorderError> {
        let Some(session) = self.active_session.as_mut() else {
            return Err(RecorderError::NotRecording);
        };
        if keys.is_empty() {
            return Ok(());
        }
        let mut keyboard_inputs = session
            .keyboard_inputs
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?;
        keyboard_inputs.push(RecordedKeyboardInput::Hotkey {
            keys,
            captured_at_ms,
            target_window: None,
        });
        Ok(())
    }

    pub fn enable_mouse_capture(&mut self) -> Result<(), RecorderError> {
        let Some(session) = self.active_session.as_mut() else {
            return Err(RecorderError::NotRecording);
        };
        if session.mouse_capture.is_some() {
            return Ok(());
        }
        let mouse_capture = mouse::start_click_capture(Arc::clone(&session.mouse_clicks))
            .map_err(|_| RecorderError::CaptureUnavailable)?;
        session.mouse_capture = Some(mouse_capture);
        Ok(())
    }

    pub fn enable_keyboard_capture(&mut self) -> Result<(), RecorderError> {
        let Some(session) = self.active_session.as_mut() else {
            return Err(RecorderError::NotRecording);
        };
        if session.keyboard_capture.is_some() {
            return Ok(());
        }
        let keyboard_capture = keyboard::start_key_capture(Arc::clone(&session.keyboard_inputs))
            .map_err(|_| RecorderError::CaptureUnavailable)?;
        session.keyboard_capture = Some(keyboard_capture);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<RecordingStopPayload, RecorderError> {
        self.stop_excluding_regions(&[])
    }

    pub fn stop_excluding_regions(
        &mut self,
        excluded_regions: &[ScreenRect],
    ) -> Result<RecordingStopPayload, RecorderError> {
        let Some(mut session) = self.active_session.take() else {
            return Err(RecorderError::NotRecording);
        };

        if let Some(mouse_capture) = session.mouse_capture.take() {
            mouse_capture.stop();
        }
        if let Some(keyboard_capture) = session.keyboard_capture.take() {
            keyboard_capture.stop();
        }

        let stopped_at = unix_seconds();
        let mouse_clicks = session
            .mouse_clicks
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?
            .iter()
            .filter(|click| {
                !excluded_regions
                    .iter()
                    .any(|region| region.contains(click.x, click.y))
            })
            .cloned()
            .collect::<Vec<_>>();
        let keyboard_inputs = session
            .keyboard_inputs
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?
            .clone();
        let flow = recorded_flow(
            session.started_at,
            session.started_at_ms,
            session.target_window,
            &mouse_clicks,
            &keyboard_inputs,
        );
        let message = if mouse_clicks.is_empty() && keyboard_inputs.is_empty() {
            "已停止录制会话；生成的是安全占位流程，尚未捕获真实输入。"
        } else if keyboard_inputs.is_empty() {
            "已停止录制会话；已捕获鼠标点击步骤，本次未捕获键盘输入。"
        } else if mouse_clicks.is_empty() {
            "已停止录制会话；已捕获键盘输入和热键步骤。"
        } else {
            "已停止录制会话；已捕获鼠标点击、键盘输入和热键步骤。"
        };

        Ok(RecordingStopPayload {
            status: "stopped",
            label: "已停止",
            started_at: session.started_at,
            stopped_at,
            flow,
            message,
        })
    }
}

fn recorded_flow(
    started_at: u64,
    started_at_ms: u64,
    target_window: TargetWindow,
    mouse_clicks: &[RecordedMouseClick],
    keyboard_inputs: &[RecordedKeyboardInput],
) -> Flow {
    let target_window =
        first_recorded_target_window(mouse_clicks, keyboard_inputs).unwrap_or(target_window);
    let steps = recorded_steps(started_at_ms, mouse_clicks, keyboard_inputs);
    Flow {
        version: 1,
        name: format!("recording-{started_at}"),
        display_name: if steps.is_empty() {
            format!("录制会话安全占位 {started_at}")
        } else {
            format!("录制会话 {started_at}")
        },
        target_window,
        steps: if steps.is_empty() {
            safe_placeholder_steps()
        } else {
            steps
        },
    }
}

fn first_recorded_target_window(
    mouse_clicks: &[RecordedMouseClick],
    keyboard_inputs: &[RecordedKeyboardInput],
) -> Option<TargetWindow> {
    let mut targets = mouse_clicks
        .iter()
        .filter_map(|click| {
            click
                .target_window
                .as_ref()
                .map(|target_window| (click.captured_at_ms, target_window))
        })
        .collect::<Vec<_>>();

    targets.extend(keyboard_inputs.iter().filter_map(|input| {
        match input {
            RecordedKeyboardInput::Text {
                captured_at_ms,
                target_window,
                ..
            }
            | RecordedKeyboardInput::Hotkey {
                captured_at_ms,
                target_window,
                ..
            } => target_window
                .as_ref()
                .map(|target_window| (*captured_at_ms, target_window)),
        }
    }));

    targets.sort_by_key(|(captured_at_ms, _)| *captured_at_ms);
    targets
        .into_iter()
        .map(|(_, target_window)| target_window)
        .find(|target_window| target_window.matched && target_window.process != "N/A")
        .cloned()
}

fn safe_placeholder_steps() -> Vec<FlowStep> {
    vec![FlowStep::Wait {
        id: 1,
        action: "等待".to_string(),
        duration_ms: 500,
        delay_ms: 500,
        note: "安全占位步骤：尚未捕获真实输入".to_string(),
    }]
}

fn recorded_steps(
    started_at_ms: u64,
    mouse_clicks: &[RecordedMouseClick],
    keyboard_inputs: &[RecordedKeyboardInput],
) -> Vec<FlowStep> {
    let mut actions = mouse_clicks
        .iter()
        .cloned()
        .map(RecordedAction::MouseClick)
        .collect::<Vec<_>>();

    actions.extend(keyboard_inputs.iter().cloned().map(|input| match input {
        RecordedKeyboardInput::Text {
            text,
            captured_at_ms,
            target_window,
        } => RecordedAction::Text {
            text,
            captured_at_ms,
            target_window,
        },
        RecordedKeyboardInput::Hotkey {
            keys,
            captured_at_ms,
            target_window,
        } => RecordedAction::Hotkey {
            keys,
            captured_at_ms,
            target_window,
        },
    }));
    actions.sort_by_key(RecordedAction::captured_at_ms);

    let mut previous_at_ms = started_at_ms;
    let mut steps = Vec::new();
    let mut action_index = 0;

    while action_index < actions.len() {
        match &actions[action_index] {
            RecordedAction::MouseClick(click) => {
                let delay_ms = click.captured_at_ms.saturating_sub(previous_at_ms);
                previous_at_ms = click.captured_at_ms;
                steps.push(FlowStep::Click {
                    id: (steps.len() + 1) as u32,
                    action: mouse_button_action(click.button).to_string(),
                    target: format!("({}, {}) [屏幕绝对]", click.x, click.y),
                    x: click.x,
                    y: click.y,
                    delay_ms,
                    note: "录制捕获：鼠标点击".to_string(),
                });
                action_index += 1;
            }
            RecordedAction::Text { captured_at_ms, .. } => {
                let first_at_ms = *captured_at_ms;
                let delay_ms = first_at_ms.saturating_sub(previous_at_ms);
                let mut text = String::new();
                let mut last_at_ms = first_at_ms;

                while let Some(RecordedAction::Text {
                    text: next_text,
                    captured_at_ms,
                    ..
                }) = actions.get(action_index)
                {
                    text.push_str(next_text);
                    last_at_ms = *captured_at_ms;
                    action_index += 1;
                }

                previous_at_ms = last_at_ms;
                steps.push(FlowStep::Type {
                    id: (steps.len() + 1) as u32,
                    action: "文本输入".to_string(),
                    text,
                    delay_ms,
                    note: "录制捕获：键盘输入".to_string(),
                });
            }
            RecordedAction::Hotkey {
                keys,
                captured_at_ms,
                ..
            } => {
                let delay_ms = captured_at_ms.saturating_sub(previous_at_ms);
                previous_at_ms = *captured_at_ms;
                steps.push(FlowStep::Hotkey {
                    id: (steps.len() + 1) as u32,
                    action: "快捷键".to_string(),
                    keys: keys.clone(),
                    delay_ms,
                    note: "录制捕获：快捷键".to_string(),
                });
                action_index += 1;
            }
        }
    }

    steps
}

fn mouse_button_action(button: RecordedMouseButton) -> &'static str {
    match button {
        RecordedMouseButton::Left => "左键单击",
        RecordedMouseButton::Right => "右键单击",
    }
}

fn unknown_target_window() -> TargetWindow {
    TargetWindow {
        title: "尚未捕获活动窗口".to_string(),
        process: "N/A".to_string(),
        size: "N/A".to_string(),
        matched: false,
    }
}

fn unix_seconds() -> u64 {
    unix_millis() / 1000
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
