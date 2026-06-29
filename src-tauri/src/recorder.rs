use serde::Serialize;
use std::{
    fmt,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::keyboard;
use crate::mouse;
use crate::storage::{hotkey_is_allowed, Flow, FlowStep, TargetWindow};

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
    mouse_drags: Arc<Mutex<Vec<RecordedMouseDrag>>>,
    mouse_scrolls: Arc<Mutex<Vec<RecordedMouseScroll>>>,
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
pub(crate) struct RecordedMouseScroll {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) delta_x: i32,
    pub(crate) delta_y: i32,
    pub(crate) captured_at_ms: u64,
    pub(crate) target_window: Option<TargetWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordedMouseDrag {
    pub(crate) start_x: i32,
    pub(crate) start_y: i32,
    pub(crate) end_x: i32,
    pub(crate) end_y: i32,
    pub(crate) button: RecordedMouseButton,
    pub(crate) started_at_ms: u64,
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
    Key {
        key: String,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecordedAction {
    MouseClick(RecordedMouseClick),
    MouseDrag(RecordedMouseDrag),
    MouseScroll(RecordedMouseScroll),
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
    Key {
        key: String,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    },
}

impl RecordedAction {
    fn captured_at_ms(&self) -> u64 {
        match self {
            Self::MouseClick(click) => click.captured_at_ms,
            Self::MouseDrag(drag) => drag.started_at_ms,
            Self::MouseScroll(scroll) => scroll.captured_at_ms,
            Self::Text { captured_at_ms, .. } => *captured_at_ms,
            Self::Hotkey { captured_at_ms, .. } => *captured_at_ms,
            Self::Key { captured_at_ms, .. } => *captured_at_ms,
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
            mouse_drags: Arc::new(Mutex::new(Vec::new())),
            mouse_scrolls: Arc::new(Mutex::new(Vec::new())),
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

    pub fn record_mouse_scroll_at(
        &mut self,
        x: i32,
        y: i32,
        delta_x: i32,
        delta_y: i32,
        captured_at_ms: u64,
    ) -> Result<(), RecorderError> {
        self.record_mouse_scroll_at_maybe_target(x, y, delta_x, delta_y, captured_at_ms, None)
    }

    fn record_mouse_scroll_at_maybe_target(
        &mut self,
        x: i32,
        y: i32,
        delta_x: i32,
        delta_y: i32,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    ) -> Result<(), RecorderError> {
        let Some(session) = self.active_session.as_mut() else {
            return Err(RecorderError::NotRecording);
        };
        if delta_x == 0 && delta_y == 0 {
            return Ok(());
        }
        let mut mouse_scrolls = session
            .mouse_scrolls
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?;
        mouse_scrolls.push(RecordedMouseScroll {
            x,
            y,
            delta_x,
            delta_y,
            captured_at_ms,
            target_window,
        });
        Ok(())
    }

    pub fn record_mouse_drag_at(
        &mut self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        button: RecordedMouseButton,
        started_at_ms: u64,
        captured_at_ms: u64,
    ) -> Result<(), RecorderError> {
        self.record_mouse_drag_at_maybe_target(
            start_x,
            start_y,
            end_x,
            end_y,
            button,
            started_at_ms,
            captured_at_ms,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record_mouse_drag_at_maybe_target(
        &mut self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        button: RecordedMouseButton,
        started_at_ms: u64,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
    ) -> Result<(), RecorderError> {
        let Some(session) = self.active_session.as_mut() else {
            return Err(RecorderError::NotRecording);
        };
        let mut mouse_drags = session
            .mouse_drags
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?;
        mouse_drags.push(RecordedMouseDrag {
            start_x,
            start_y,
            end_x,
            end_y,
            button,
            started_at_ms,
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
        self.record_text_input_at_maybe_target(text, captured_at_ms, None)
    }

    pub fn record_text_input_at_target(
        &mut self,
        text: &str,
        captured_at_ms: u64,
        target_window: TargetWindow,
    ) -> Result<(), RecorderError> {
        self.record_text_input_at_maybe_target(text, captured_at_ms, Some(target_window))
    }

    fn record_text_input_at_maybe_target(
        &mut self,
        text: &str,
        captured_at_ms: u64,
        target_window: Option<TargetWindow>,
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
            target_window,
        });
        Ok(())
    }

    pub fn record_key_press_at(
        &mut self,
        key: &str,
        captured_at_ms: u64,
    ) -> Result<(), RecorderError> {
        let Some(session) = self.active_session.as_mut() else {
            return Err(RecorderError::NotRecording);
        };
        let key = key.trim();
        if key.is_empty() {
            return Ok(());
        }
        let mut keyboard_inputs = session
            .keyboard_inputs
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?;
        keyboard_inputs.push(RecordedKeyboardInput::Key {
            key: key.to_string(),
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
        let mouse_capture = mouse::start_mouse_capture(
            Arc::clone(&session.mouse_clicks),
            Arc::clone(&session.mouse_drags),
            Arc::clone(&session.mouse_scrolls),
        )
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
                !click.target_window.as_ref().is_some_and(is_remember_window)
                    && !excluded_regions
                        .iter()
                        .any(|region| region.contains(click.x, click.y))
            })
            .cloned()
            .collect::<Vec<_>>();
        let mouse_drags = session
            .mouse_drags
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?
            .iter()
            .filter(|drag| {
                !drag.target_window.as_ref().is_some_and(is_remember_window)
                    && !excluded_regions.iter().any(|region| {
                        region.contains(drag.start_x, drag.start_y)
                            || region.contains(drag.end_x, drag.end_y)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        let mouse_scrolls = session
            .mouse_scrolls
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?
            .iter()
            .filter(|scroll| {
                !scroll
                    .target_window
                    .as_ref()
                    .is_some_and(is_remember_window)
                    && !excluded_regions
                        .iter()
                        .any(|region| region.contains(scroll.x, scroll.y))
            })
            .cloned()
            .collect::<Vec<_>>();
        let keyboard_inputs = session
            .keyboard_inputs
            .lock()
            .map_err(|_| RecorderError::CaptureUnavailable)?
            .iter()
            .filter(|input| keyboard_input_is_safe(input))
            .cloned()
            .collect::<Vec<_>>();
        let flow = recorded_flow(
            session.started_at,
            session.started_at_ms,
            session.target_window,
            &mouse_clicks,
            &mouse_drags,
            &mouse_scrolls,
            &keyboard_inputs,
        );
        let has_mouse_input =
            !mouse_clicks.is_empty() || !mouse_drags.is_empty() || !mouse_scrolls.is_empty();
        let message = if !has_mouse_input && keyboard_inputs.is_empty() {
            "已停止录制会话；尚未捕获真实输入。"
        } else if keyboard_inputs.is_empty() {
            "已停止录制会话；已捕获鼠标步骤，本次未捕获键盘输入。"
        } else if !has_mouse_input {
            "已停止录制会话；已捕获键盘输入和热键步骤。"
        } else {
            "已停止录制会话；已捕获鼠标、键盘输入和热键步骤。"
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

fn keyboard_input_is_safe(input: &RecordedKeyboardInput) -> bool {
    let target_window = match input {
        RecordedKeyboardInput::Text { target_window, .. }
        | RecordedKeyboardInput::Hotkey { target_window, .. }
        | RecordedKeyboardInput::Key { target_window, .. } => target_window,
    };

    if target_window.as_ref().is_some_and(|target_window| {
        is_remember_window(target_window) || is_sensitive_window(target_window)
    }) {
        return false;
    }

    match input {
        RecordedKeyboardInput::Hotkey { keys, .. } => hotkey_is_allowed(keys),
        RecordedKeyboardInput::Key { key, .. } => !key.trim().is_empty(),
        RecordedKeyboardInput::Text { text, .. } => !text.is_empty(),
    }
}

fn is_remember_window(target_window: &TargetWindow) -> bool {
    target_window
        .process
        .trim()
        .eq_ignore_ascii_case("remember.exe")
}

fn is_sensitive_window(target_window: &TargetWindow) -> bool {
    let haystack = format!(
        "{} {}",
        target_window.title.to_ascii_lowercase(),
        target_window.process.to_ascii_lowercase()
    );
    [
        "password",
        "passcode",
        "login",
        "sign in",
        "signin",
        "otp",
        "2fa",
        "verification",
        "credit card",
        "payment",
        "密码",
        "登录",
        "登入",
        "验证码",
        "支付",
        "银行卡",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn recorded_flow(
    started_at: u64,
    started_at_ms: u64,
    target_window: TargetWindow,
    mouse_clicks: &[RecordedMouseClick],
    mouse_drags: &[RecordedMouseDrag],
    mouse_scrolls: &[RecordedMouseScroll],
    keyboard_inputs: &[RecordedKeyboardInput],
) -> Flow {
    let target_window =
        first_recorded_target_window(mouse_clicks, mouse_drags, mouse_scrolls, keyboard_inputs)
            .unwrap_or(target_window);
    let steps = recorded_steps(
        started_at_ms,
        mouse_clicks,
        mouse_drags,
        mouse_scrolls,
        keyboard_inputs,
    );
    Flow {
        version: 1,
        name: format!("recording-{started_at}"),
        display_name: if steps.is_empty() {
            format!("空录制会话 {started_at}")
        } else {
            format!("录制会话 {started_at}")
        },
        target_window: if steps.is_empty() {
            unknown_target_window()
        } else {
            target_window
        },
        steps,
    }
}

fn first_recorded_target_window(
    mouse_clicks: &[RecordedMouseClick],
    mouse_drags: &[RecordedMouseDrag],
    mouse_scrolls: &[RecordedMouseScroll],
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

    targets.extend(mouse_drags.iter().filter_map(|drag| {
        drag.target_window
            .as_ref()
            .map(|target_window| (drag.started_at_ms, target_window))
    }));

    targets.extend(mouse_scrolls.iter().filter_map(|scroll| {
        scroll
            .target_window
            .as_ref()
            .map(|target_window| (scroll.captured_at_ms, target_window))
    }));

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
            }
            | RecordedKeyboardInput::Key {
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

fn recorded_steps(
    started_at_ms: u64,
    mouse_clicks: &[RecordedMouseClick],
    mouse_drags: &[RecordedMouseDrag],
    mouse_scrolls: &[RecordedMouseScroll],
    keyboard_inputs: &[RecordedKeyboardInput],
) -> Vec<FlowStep> {
    let mut actions = mouse_clicks
        .iter()
        .cloned()
        .map(RecordedAction::MouseClick)
        .collect::<Vec<_>>();

    actions.extend(mouse_drags.iter().cloned().map(RecordedAction::MouseDrag));

    actions.extend(
        mouse_scrolls
            .iter()
            .cloned()
            .map(RecordedAction::MouseScroll),
    );

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
        RecordedKeyboardInput::Key {
            key,
            captured_at_ms,
            target_window,
        } => RecordedAction::Key {
            key,
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
                if let Some(next_click) =
                    actions
                        .get(action_index + 1)
                        .and_then(|action| match action {
                            RecordedAction::MouseClick(next_click)
                                if is_double_click_pair(click, next_click) =>
                            {
                                Some(next_click)
                            }
                            _ => None,
                        })
                {
                    previous_at_ms = next_click.captured_at_ms;
                    steps.push(FlowStep::Click {
                        id: (steps.len() + 1) as u32,
                        action: "双击".to_string(),
                        target: format!("({}, {}) [屏幕绝对]", click.x, click.y),
                        x: click.x,
                        y: click.y,
                        delay_ms,
                        note: "录制捕获：鼠标双击".to_string(),
                    });
                    action_index += 2;
                    continue;
                }

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
            RecordedAction::MouseScroll(scroll) => {
                let delay_ms = scroll.captured_at_ms.saturating_sub(previous_at_ms);
                previous_at_ms = scroll.captured_at_ms;
                steps.push(FlowStep::Scroll {
                    id: (steps.len() + 1) as u32,
                    action: "滚动".to_string(),
                    x: Some(scroll.x),
                    y: Some(scroll.y),
                    delta_x: scroll.delta_x,
                    delta_y: scroll.delta_y,
                    delay_ms,
                    note: "录制捕获：鼠标滚轮".to_string(),
                });
                action_index += 1;
            }
            RecordedAction::MouseDrag(drag) => {
                let delay_ms = drag.started_at_ms.saturating_sub(previous_at_ms);
                previous_at_ms = drag.captured_at_ms;
                steps.push(FlowStep::Drag {
                    id: (steps.len() + 1) as u32,
                    action: mouse_drag_action(drag.button).to_string(),
                    target: format!(
                        "({}, {}) -> ({}, {}) [屏幕绝对]",
                        drag.start_x, drag.start_y, drag.end_x, drag.end_y
                    ),
                    start_x: drag.start_x,
                    start_y: drag.start_y,
                    end_x: drag.end_x,
                    end_y: drag.end_y,
                    duration_ms: drag.captured_at_ms.saturating_sub(drag.started_at_ms),
                    delay_ms,
                    note: "录制捕获：鼠标拖拽".to_string(),
                });
                action_index += 1;
            }
            RecordedAction::Text {
                captured_at_ms,
                target_window,
                ..
            } => {
                let first_at_ms = *captured_at_ms;
                let first_target_window = target_window.clone();
                let delay_ms = first_at_ms.saturating_sub(previous_at_ms);
                let mut text = String::new();
                let mut last_at_ms = first_at_ms;

                while let Some(RecordedAction::Text {
                    text: next_text,
                    captured_at_ms,
                    target_window,
                }) = actions.get(action_index)
                {
                    if target_window != &first_target_window {
                        break;
                    }
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
            RecordedAction::Key {
                key,
                captured_at_ms,
                ..
            } => {
                let delay_ms = captured_at_ms.saturating_sub(previous_at_ms);
                previous_at_ms = *captured_at_ms;
                steps.push(FlowStep::Key {
                    id: (steps.len() + 1) as u32,
                    action: "按键".to_string(),
                    key: key.clone(),
                    delay_ms,
                    note: "录制捕获：键盘按键".to_string(),
                });
                action_index += 1;
            }
        }
    }

    steps
}

fn is_double_click_pair(first: &RecordedMouseClick, second: &RecordedMouseClick) -> bool {
    const DOUBLE_CLICK_MAX_INTERVAL_MS: u64 = 500;
    const DOUBLE_CLICK_MAX_DISTANCE: i32 = 4;

    first.button == RecordedMouseButton::Left
        && second.button == RecordedMouseButton::Left
        && second.captured_at_ms.saturating_sub(first.captured_at_ms)
            <= DOUBLE_CLICK_MAX_INTERVAL_MS
        && (second.x - first.x).abs() <= DOUBLE_CLICK_MAX_DISTANCE
        && (second.y - first.y).abs() <= DOUBLE_CLICK_MAX_DISTANCE
}

fn mouse_button_action(button: RecordedMouseButton) -> &'static str {
    match button {
        RecordedMouseButton::Left => "左键单击",
        RecordedMouseButton::Right => "右键单击",
    }
}

fn mouse_drag_action(button: RecordedMouseButton) -> &'static str {
    match button {
        RecordedMouseButton::Left => "左键拖拽",
        RecordedMouseButton::Right => "右键拖拽",
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
