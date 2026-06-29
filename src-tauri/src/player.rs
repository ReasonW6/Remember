use crate::{
    playback_input::{PlaybackInput, PlaybackMouseButton, PlaybackMousePoint, SystemPlaybackInput},
    storage::{validate_flow, DragPathPoint, Flow, FlowStep, TargetWindow},
};
use serde::Serialize;
use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

static NEXT_PLAYBACK_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub enum PlayerError {
    AlreadyPlaying,
    InvalidFlow(String),
    InvalidLoopCount,
    InvalidSpeed,
    NotPlaying,
}

impl fmt::Display for PlayerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyPlaying => write!(formatter, "playback is already active"),
            Self::InvalidFlow(message) => write!(formatter, "invalid playback flow: {message}"),
            Self::InvalidLoopCount => write!(
                formatter,
                "infinite playback requires explicit confirmation"
            ),
            Self::InvalidSpeed => write!(formatter, "playback speed must be greater than zero"),
            Self::NotPlaying => write!(formatter, "playback is not active"),
        }
    }
}

impl std::error::Error for PlayerError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackOptions {
    pub speed_multiplier: f64,
    pub loop_count: u32,
    pub infinite_loop_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackFinishReason {
    Completed,
    Stopped,
    EmergencyStopped,
    SafetyStopped,
}

impl PlaybackFinishReason {
    fn message_prefix(self) -> &'static str {
        match self {
            Self::Completed => "回放完成",
            Self::Stopped => "回放已停止",
            Self::EmergencyStopped => "回放已紧急停止",
            Self::SafetyStopped => "回放已安全停止",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStartPayload {
    pub run_id: u64,
    pub status: &'static str,
    pub label: &'static str,
    pub flow_name: String,
    pub loop_count: u32,
    pub speed_multiplier: f64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackControlPayload {
    pub status: &'static str,
    pub label: &'static str,
    pub reason: PlaybackFinishReason,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackFinishedPayload {
    pub run_id: u64,
    pub status: &'static str,
    pub label: &'static str,
    pub reason: PlaybackFinishReason,
    pub flow_name: String,
    pub completed_steps: u32,
    pub skipped_steps: u32,
    pub loop_count: u32,
    pub message: String,
}

#[derive(Debug)]
struct PlaybackSession {
    cancel_requested: Arc<AtomicBool>,
    cancel_reason: Arc<Mutex<PlaybackFinishReason>>,
    finished: Arc<AtomicBool>,
    _join_handle: JoinHandle<()>,
}

pub struct PlayerState {
    active_session: Option<PlaybackSession>,
    input: Arc<dyn PlaybackInput>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::with_input(Arc::new(SystemPlaybackInput))
    }
}

impl PlayerState {
    pub fn with_input(input: Arc<dyn PlaybackInput>) -> Self {
        Self {
            active_session: None,
            input,
        }
    }

    pub fn is_playing(&mut self) -> bool {
        self.clear_finished_session();
        self.active_session.is_some()
    }

    pub fn start<F>(
        &mut self,
        flow: Flow,
        options: PlaybackOptions,
        on_finished: F,
    ) -> Result<PlaybackStartPayload, PlayerError>
    where
        F: FnOnce(PlaybackFinishedPayload) + Send + 'static,
    {
        self.clear_finished_session();
        if self.active_session.is_some() {
            return Err(PlayerError::AlreadyPlaying);
        }
        validate_options(options)?;
        validate_flow(&flow).map_err(|error| PlayerError::InvalidFlow(error.to_string()))?;

        let cancel_requested = Arc::new(AtomicBool::new(false));
        let cancel_reason = Arc::new(Mutex::new(PlaybackFinishReason::Stopped));
        let finished = Arc::new(AtomicBool::new(false));

        let thread_cancel_requested = Arc::clone(&cancel_requested);
        let thread_cancel_reason = Arc::clone(&cancel_reason);
        let thread_finished = Arc::clone(&finished);
        let thread_input = Arc::clone(&self.input);
        let flow_name = flow.display_name.clone();
        let run_id = NEXT_PLAYBACK_RUN_ID.fetch_add(1, Ordering::Relaxed);
        let loop_description = loop_description(options.loop_count);
        let start_payload = PlaybackStartPayload {
            run_id,
            status: "playing",
            label: "回放中",
            flow_name: flow_name.clone(),
            loop_count: options.loop_count,
            speed_multiplier: options.speed_multiplier,
            message: format!(
                "开始回放 {flow_name}；{loop_description}；当前安全切片会执行等待、通过目标窗口检查的点击、拖拽、文本输入、按键、热键和滚轮。"
            ),
        };

        let join_handle = thread::spawn(move || {
            let payload = run_playback(
                flow,
                options,
                thread_cancel_requested,
                thread_cancel_reason,
                thread_input,
                run_id,
            );
            thread_finished.store(true, Ordering::SeqCst);
            on_finished(payload);
        });

        self.active_session = Some(PlaybackSession {
            cancel_requested,
            cancel_reason,
            finished,
            _join_handle: join_handle,
        });

        Ok(start_payload)
    }

    pub fn stop(&mut self) -> Result<PlaybackControlPayload, PlayerError> {
        self.request_stop(PlaybackFinishReason::Stopped)
    }

    pub fn emergency_stop(&mut self) -> Result<PlaybackControlPayload, PlayerError> {
        self.request_stop(PlaybackFinishReason::EmergencyStopped)
    }

    fn request_stop(
        &mut self,
        reason: PlaybackFinishReason,
    ) -> Result<PlaybackControlPayload, PlayerError> {
        self.clear_finished_session();
        let Some(session) = self.active_session.as_ref() else {
            return Err(PlayerError::NotPlaying);
        };

        if let Ok(mut cancel_reason) = session.cancel_reason.lock() {
            *cancel_reason = reason;
        }
        session.cancel_requested.store(true, Ordering::SeqCst);

        Ok(PlaybackControlPayload {
            status: "stopped",
            label: "已停止",
            reason,
            message: match reason {
                PlaybackFinishReason::Stopped => "已请求停止当前回放。".to_string(),
                PlaybackFinishReason::EmergencyStopped => "已触发紧急停止。".to_string(),
                PlaybackFinishReason::Completed => "回放已经完成。".to_string(),
                PlaybackFinishReason::SafetyStopped => "已触发安全停止。".to_string(),
            },
        })
    }

    fn clear_finished_session(&mut self) {
        let should_clear = self
            .active_session
            .as_ref()
            .is_some_and(|session| session.finished.load(Ordering::SeqCst));
        if should_clear {
            self.active_session = None;
        }
    }
}

fn validate_options(options: PlaybackOptions) -> Result<(), PlayerError> {
    if !options.speed_multiplier.is_finite() || options.speed_multiplier <= 0.0 {
        return Err(PlayerError::InvalidSpeed);
    }
    if options.loop_count == 0 && !options.infinite_loop_confirmed {
        return Err(PlayerError::InvalidLoopCount);
    }
    Ok(())
}

fn run_playback(
    flow: Flow,
    options: PlaybackOptions,
    cancel_requested: Arc<AtomicBool>,
    cancel_reason: Arc<Mutex<PlaybackFinishReason>>,
    input: Arc<dyn PlaybackInput>,
    run_id: u64,
) -> PlaybackFinishedPayload {
    let mut completed_steps: u32 = 0;
    let mut skipped_steps: u32 = 0;
    let mut reason = PlaybackFinishReason::Completed;
    let mut safety_message: Option<String> = None;

    let mut completed_loops: u32 = 0;

    'playback: loop {
        if options.loop_count != 0 && completed_loops >= options.loop_count {
            break;
        }

        if options.loop_count == 0 && flow.steps.is_empty() {
            break;
        }

        for step in &flow.steps {
            if cancel_requested.load(Ordering::SeqCst) {
                reason = current_cancel_reason(&cancel_reason);
                break 'playback;
            }

            match step {
                FlowStep::Click {
                    action,
                    x,
                    y,
                    delay_ms,
                    ..
                } => {
                    if let Err(cancelled_reason) =
                        wait_before_step(*delay_ms, options, &cancel_requested, &cancel_reason)
                    {
                        reason = cancelled_reason;
                        break 'playback;
                    }

                    if let Err(message) =
                        prepare_active_input_target(input.as_ref(), &flow.target_window)
                    {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(message);
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }

                    let (button, repeat_count) = click_plan(action);
                    for _ in 0..repeat_count {
                        if cancel_requested.load(Ordering::SeqCst) {
                            reason = current_cancel_reason(&cancel_reason);
                            break 'playback;
                        }
                        if let Err(error) = input.click(button, *x, *y) {
                            reason = PlaybackFinishReason::SafetyStopped;
                            safety_message =
                                Some(format!("点击执行失败：{error}，已拒绝继续回放。"));
                            skipped_steps = skipped_steps.saturating_add(1);
                            break 'playback;
                        }
                    }
                    completed_steps = completed_steps.saturating_add(1);
                }
                FlowStep::Drag {
                    action,
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    duration_ms,
                    delay_ms,
                    path,
                    ..
                } => {
                    if let Err(cancelled_reason) =
                        wait_before_step(*delay_ms, options, &cancel_requested, &cancel_reason)
                    {
                        reason = cancelled_reason;
                        break 'playback;
                    }

                    if let Err(message) =
                        prepare_active_input_target(input.as_ref(), &flow.target_window)
                    {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(message);
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }

                    let (button, _) = click_plan(action);
                    let adjusted_drag_duration =
                        adjusted_duration(*duration_ms, options.speed_multiplier);
                    let drag_result = if path.len() >= 2 {
                        let adjusted_path = adjusted_drag_path(path, options.speed_multiplier);
                        input.drag_path_cancelable(button, &adjusted_path, &cancel_requested)
                    } else {
                        input.drag_cancelable(
                            button,
                            *start_x,
                            *start_y,
                            *end_x,
                            *end_y,
                            adjusted_drag_duration.as_millis() as u64,
                            &cancel_requested,
                        )
                    };
                    match drag_result {
                        Ok(true) => {
                            reason = current_cancel_reason(&cancel_reason);
                            break 'playback;
                        }
                        Ok(false) => {}
                        Err(error) => {
                            reason = PlaybackFinishReason::SafetyStopped;
                            safety_message =
                                Some(format!("拖拽执行失败：{error}，已拒绝继续回放。"));
                            skipped_steps = skipped_steps.saturating_add(1);
                            break 'playback;
                        }
                    }
                    completed_steps = completed_steps.saturating_add(1);
                }
                FlowStep::Type { text, delay_ms, .. } => {
                    if let Err(cancelled_reason) =
                        wait_before_step(*delay_ms, options, &cancel_requested, &cancel_reason)
                    {
                        reason = cancelled_reason;
                        break 'playback;
                    }

                    if let Err(message) =
                        prepare_active_input_target(input.as_ref(), &flow.target_window)
                    {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(message);
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }

                    if let Err(error) = input.type_text(text) {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(format!("文本输入失败：{error}，已拒绝继续回放。"));
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }
                    completed_steps = completed_steps.saturating_add(1);
                }
                FlowStep::Key { key, delay_ms, .. } => {
                    if let Err(cancelled_reason) =
                        wait_before_step(*delay_ms, options, &cancel_requested, &cancel_reason)
                    {
                        reason = cancelled_reason;
                        break 'playback;
                    }

                    if let Err(message) =
                        prepare_active_input_target(input.as_ref(), &flow.target_window)
                    {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(message);
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }

                    if let Err(error) = input.press_key(key) {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(format!("按键输入失败：{error}，已拒绝继续回放。"));
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }
                    completed_steps = completed_steps.saturating_add(1);
                }
                FlowStep::Wait { duration_ms, .. } => {
                    if let Err(cancelled_reason) =
                        wait_before_step(*duration_ms, options, &cancel_requested, &cancel_reason)
                    {
                        reason = cancelled_reason;
                        break 'playback;
                    }
                    completed_steps = completed_steps.saturating_add(1);
                }
                FlowStep::Hotkey { keys, delay_ms, .. } => {
                    if let Err(cancelled_reason) =
                        wait_before_step(*delay_ms, options, &cancel_requested, &cancel_reason)
                    {
                        reason = cancelled_reason;
                        break 'playback;
                    }

                    if let Err(message) =
                        prepare_active_input_target(input.as_ref(), &flow.target_window)
                    {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(message);
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }

                    if let Err(error) = input.press_hotkey(keys) {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(format!("热键输入失败：{error}，已拒绝继续回放。"));
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }
                    completed_steps = completed_steps.saturating_add(1);
                }
                FlowStep::Scroll {
                    x,
                    y,
                    delta_x,
                    delta_y,
                    delay_ms,
                    ..
                } => {
                    if let Err(cancelled_reason) =
                        wait_before_step(*delay_ms, options, &cancel_requested, &cancel_reason)
                    {
                        reason = cancelled_reason;
                        break 'playback;
                    }

                    if let Err(message) =
                        prepare_active_input_target(input.as_ref(), &flow.target_window)
                    {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(message);
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }

                    let scroll_result = match (x, y) {
                        (Some(x), Some(y)) => input.scroll_at(*x, *y, *delta_x, *delta_y),
                        _ => input.scroll(*delta_x, *delta_y),
                    };
                    if let Err(error) = scroll_result {
                        reason = PlaybackFinishReason::SafetyStopped;
                        safety_message = Some(format!("滚轮输入失败：{error}，已拒绝继续回放。"));
                        skipped_steps = skipped_steps.saturating_add(1);
                        break 'playback;
                    }
                    completed_steps = completed_steps.saturating_add(1);
                }
            }
        }

        completed_loops = completed_loops.saturating_add(1);
    }

    let message = finish_message(reason, completed_steps, skipped_steps, safety_message);
    PlaybackFinishedPayload {
        run_id,
        status: "stopped",
        label: "已停止",
        reason,
        flow_name: flow.display_name,
        completed_steps,
        skipped_steps,
        loop_count: options.loop_count,
        message,
    }
}

fn validate_input_target(expected: &TargetWindow, active: &TargetWindow) -> Result<(), String> {
    if !is_known_target(expected) {
        return Err("目标窗口缺少可验证信息，已拒绝输入操作。".to_string());
    }

    if !is_known_target(active) {
        return Err("当前活动窗口不可验证，已拒绝输入操作。".to_string());
    }

    if !same_process(&expected.process, &active.process) {
        return Err(format!(
            "目标窗口不同：录制时为 {}，当前为 {}，未执行输入操作。",
            expected.process, active.process
        ));
    }

    if has_known_title(&expected.title)
        && has_known_title(&active.title)
        && !same_title(&expected.title, &active.title)
    {
        return Err(format!(
            "目标窗口标题不同：录制时为“{}”，当前为“{}”，未执行输入操作。",
            expected.title, active.title
        ));
    }

    Ok(())
}

fn validate_active_input_target(
    input: &dyn PlaybackInput,
    expected: &TargetWindow,
) -> Result<(), String> {
    let active_window = input.active_window_target();
    validate_input_target(expected, &active_window)
}

fn prepare_active_input_target(
    input: &dyn PlaybackInput,
    expected: &TargetWindow,
) -> Result<(), String> {
    if validate_active_input_target(input, expected).is_ok() {
        return Ok(());
    }

    input
        .focus_target_window(expected)
        .map_err(|error| format!("无法切换到录制目标窗口：{error}，已拒绝继续回放。"))?;
    validate_active_input_target(input, expected)
}

fn wait_before_step(
    delay_ms: u64,
    options: PlaybackOptions,
    cancel_requested: &AtomicBool,
    cancel_reason: &Mutex<PlaybackFinishReason>,
) -> Result<(), PlaybackFinishReason> {
    if sleep_cancelable(
        adjusted_duration(delay_ms, options.speed_multiplier),
        cancel_requested,
    ) {
        Err(current_cancel_reason(cancel_reason))
    } else {
        Ok(())
    }
}

fn is_known_target(target: &TargetWindow) -> bool {
    target.matched && has_known_process(&target.process)
}

fn has_known_process(process: &str) -> bool {
    let process = process.trim();
    !process.is_empty() && process != "N/A" && !process.starts_with("PID ")
}

fn has_known_title(title: &str) -> bool {
    let title = title.trim();
    !title.is_empty()
        && title != "N/A"
        && title != "未知活动窗口"
        && title != "尚未捕获活动窗口"
        && !is_unstable_child_window_title(title)
}

fn is_unstable_child_window_title(title: &str) -> bool {
    title.eq_ignore_ascii_case("Chrome Legacy Window")
}

fn same_process(expected: &str, active: &str) -> bool {
    expected.trim().eq_ignore_ascii_case(active.trim())
}

fn same_title(expected: &str, active: &str) -> bool {
    expected.trim().eq_ignore_ascii_case(active.trim())
}

fn click_plan(action: &str) -> (PlaybackMouseButton, u8) {
    let button = if action.contains("右键") {
        PlaybackMouseButton::Right
    } else {
        PlaybackMouseButton::Left
    };
    let repeat_count = if action.contains("双击") { 2 } else { 1 };
    (button, repeat_count)
}

fn current_cancel_reason(cancel_reason: &Mutex<PlaybackFinishReason>) -> PlaybackFinishReason {
    cancel_reason
        .lock()
        .map(|reason| *reason)
        .unwrap_or(PlaybackFinishReason::Stopped)
}

fn adjusted_duration(duration_ms: u64, speed_multiplier: f64) -> Duration {
    let adjusted_ms = (duration_ms as f64 / speed_multiplier).ceil().max(0.0) as u64;
    Duration::from_millis(adjusted_ms)
}

fn adjusted_drag_path(path: &[DragPathPoint], speed_multiplier: f64) -> Vec<PlaybackMousePoint> {
    path.iter()
        .map(|point| PlaybackMousePoint {
            x: point.x,
            y: point.y,
            elapsed_ms: (point.elapsed_ms as f64 / speed_multiplier).ceil().max(0.0) as u64,
        })
        .collect()
}

fn loop_description(loop_count: u32) -> String {
    if loop_count == 0 {
        "无限循环，直到手动停止或触发紧急停止".to_string()
    } else {
        format!("循环 {loop_count} 次")
    }
}

fn sleep_cancelable(duration: Duration, cancel_requested: &AtomicBool) -> bool {
    let interval = Duration::from_millis(10);
    let mut elapsed = Duration::ZERO;

    while elapsed < duration {
        if cancel_requested.load(Ordering::SeqCst) {
            return true;
        }
        let remaining = duration.saturating_sub(elapsed);
        let sleep_for = remaining.min(interval);
        thread::sleep(sleep_for);
        elapsed += sleep_for;
    }

    cancel_requested.load(Ordering::SeqCst)
}

fn finish_message(
    reason: PlaybackFinishReason,
    completed_steps: u32,
    skipped_steps: u32,
    safety_message: Option<String>,
) -> String {
    let prefix = reason.message_prefix();
    if let Some(safety_message) = safety_message {
        return format!(
            "{prefix}；{safety_message} 已执行 {completed_steps} 个步骤，跳过 {skipped_steps} 个步骤。"
        );
    }

    if skipped_steps == 0 {
        format!("{prefix}；已执行 {completed_steps} 个步骤。")
    } else {
        format!("{prefix}；已执行 {completed_steps} 个步骤，跳过 {skipped_steps} 个暂未支持或被拒绝步骤。")
    }
}
