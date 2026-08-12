use crate::model::{
    ButtonState, KeyState, MacroStep, MouseButton, PointerPosition, Recording, TargetWindowId,
    WindowPointerIntent, WindowTarget,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};

const MIN_REPEATED_LOOP_DURATION: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackSettings {
    pub loop_count: Option<u32>,
    pub speed_multiplier: f64,
}

impl PlaybackSettings {
    pub fn new(loop_count: Option<u32>, speed_multiplier: f64) -> Result<Self, String> {
        if loop_count == Some(0) {
            return Err("循环次数必须至少为 1。".to_string());
        }
        if !speed_multiplier.is_finite() || speed_multiplier <= 0.0 {
            return Err("回放速度必须为正数。".to_string());
        }
        Ok(Self {
            loop_count,
            speed_multiplier,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackAction {
    pub loop_index: u32,
    pub step_index: usize,
    pub delay_ms: u64,
    pub step: MacroStep,
}

pub trait StepExecutor {
    fn prepare_window_relative_playback(&self) -> Result<(), String> {
        Ok(())
    }

    fn begin_window_relative_loop(&self, _targets: &[WindowTarget]) -> Result<(), String> {
        Ok(())
    }

    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String>;

    fn mouse_button(
        &self,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> Result<(), String>;

    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String>;

    fn key(
        &self,
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
    ) -> Result<(), String>;

    fn window_mouse_move(
        &self,
        _target: &WindowTarget,
        _x: i32,
        _y: i32,
        _intent: WindowPointerIntent,
        _input_held: bool,
    ) -> Result<u64, String> {
        Err("当前输入执行器不支持窗口相对回放。".to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn window_mouse_button(
        &self,
        _target: &WindowTarget,
        _x: i32,
        _y: i32,
        _intent: WindowPointerIntent,
        _button: MouseButton,
        _state: ButtonState,
        _input_held: bool,
    ) -> Result<u64, String> {
        Err("当前输入执行器不支持窗口相对回放。".to_string())
    }

    fn window_mouse_wheel(
        &self,
        _target: &WindowTarget,
        _x: i32,
        _y: i32,
        _intent: WindowPointerIntent,
        _delta: i32,
        _input_held: bool,
    ) -> Result<u64, String> {
        Err("当前输入执行器不支持窗口相对回放。".to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn targeted_key(
        &self,
        _target: &WindowTarget,
        _vk_code: u16,
        _scan_code: u16,
        _extended: bool,
        _state: KeyState,
        _sequence_start: bool,
        _input_held: bool,
    ) -> Result<u64, String> {
        Err("当前输入执行器不支持窗口相对回放。".to_string())
    }

    fn release_mouse_button(&self, button: MouseButton) -> Result<(), String>;
}

#[derive(Default)]
struct StopState {
    stopped: AtomicBool,
    wait_lock: Mutex<()>,
    wake: Condvar,
}

#[derive(Clone, Default)]
pub struct StopToken {
    state: Arc<StopState>,
}

impl StopToken {
    pub fn request_stop(&self) {
        let _guard = self
            .state
            .wait_lock
            .lock()
            .expect("stop token wait lock poisoned");
        self.state.stopped.store(true, Ordering::SeqCst);
        self.state.wake.notify_all();
    }

    pub fn is_stopped(&self) -> bool {
        self.state.stopped.load(Ordering::SeqCst)
    }

    fn wait_for_stop(&self, timeout: Duration) -> bool {
        if self.is_stopped() {
            return true;
        }

        let guard = self
            .state
            .wait_lock
            .lock()
            .expect("stop token wait lock poisoned");
        if self.is_stopped() {
            return true;
        }

        let (_guard, _timeout) = self
            .state
            .wake
            .wait_timeout_while(guard, timeout, |_| !self.is_stopped())
            .expect("stop token wait lock poisoned");
        self.is_stopped()
    }

    fn wait_until_stopped(&self) {
        if self.is_stopped() {
            return;
        }

        let guard = self
            .state
            .wait_lock
            .lock()
            .expect("stop token wait lock poisoned");
        let _guard = self
            .state
            .wake
            .wait_while(guard, |_| !self.is_stopped())
            .expect("stop token wait lock poisoned");
    }
}

pub fn scaled_delay_ms(delay_ms: u64, speed_multiplier: f64) -> u64 {
    ((delay_ms as f64) / speed_multiplier).round().max(0.0) as u64
}

pub fn play_recording<E: StepExecutor + ?Sized>(
    recording: &Recording,
    settings: PlaybackSettings,
    executor: &E,
    stop_token: &StopToken,
) -> Result<(), String> {
    recording.validate()?;

    if recording.is_window_relative() {
        executor.prepare_window_relative_playback()?;
    }

    if recording.steps.is_empty() && recording.duration_ms == 0 {
        return match settings.loop_count {
            Some(_) => Ok(()),
            None => {
                stop_token.wait_until_stopped();
                Err("playback stopped".to_string())
            }
        };
    }

    let mut completed_loops = 0_u32;

    loop {
        if settings
            .loop_count
            .is_some_and(|loop_count| completed_loops >= loop_count)
        {
            return Ok(());
        }
        if stop_token.is_stopped() {
            return Err("playback stopped".to_string());
        }

        if recording.is_window_relative() {
            executor.begin_window_relative_loop(&recording.targets)?;
        }

        let mut pressed_inputs = PressedInputs::default();
        let loop_started = Instant::now();
        let mut timeline_pause_ms = 0_u64;
        for step in &recording.steps {
            let target_ms = scaled_delay_ms(step.elapsed_ms(), settings.speed_multiplier)
                .saturating_add(timeline_pause_ms);
            if let Err(error) = sleep_until(loop_started, target_ms, stop_token) {
                return cleanup_and_return(&mut pressed_inputs, executor, error);
            }
            match execute_step(step, &recording.targets, executor, &mut pressed_inputs) {
                Ok(pause_ms) => {
                    timeline_pause_ms = timeline_pause_ms.saturating_add(pause_ms);
                }
                Err(error) => {
                    return cleanup_and_return(&mut pressed_inputs, executor, error);
                }
            }
        }

        let duration_ms = scaled_delay_ms(recording.duration_ms, settings.speed_multiplier);
        let next_completed_loops = completed_loops.saturating_add(1);
        let repeats_after_this_loop = settings
            .loop_count
            .is_none_or(|loop_count| next_completed_loops < loop_count);
        let minimum_repeat_ms = MIN_REPEATED_LOOP_DURATION.as_millis() as u64;
        let loop_duration_ms = if duration_ms == 0 && repeats_after_this_loop {
            minimum_repeat_ms
        } else {
            duration_ms
        }
        .saturating_add(timeline_pause_ms);
        if let Err(error) = sleep_until(loop_started, loop_duration_ms, stop_token) {
            return cleanup_and_return(&mut pressed_inputs, executor, error);
        }
        if let Err(error) = pressed_inputs.release_all(executor) {
            return Err(format!("回放循环结束时释放残留输入失败：{error}"));
        }
        completed_loops = next_completed_loops;
    }
}

pub fn play_actions<E: StepExecutor + ?Sized>(
    actions: &[PlaybackAction],
    executor: &E,
    stop_token: &StopToken,
) -> Result<(), String> {
    let mut pressed_inputs = PressedInputs::default();

    for action in actions {
        if stop_token.is_stopped() {
            return cleanup_and_return(
                &mut pressed_inputs,
                executor,
                "playback stopped".to_string(),
            );
        }

        if let Err(err) = sleep_with_stop(action.delay_ms, stop_token) {
            return cleanup_and_return(&mut pressed_inputs, executor, err);
        }

        if stop_token.is_stopped() {
            return cleanup_and_return(
                &mut pressed_inputs,
                executor,
                "playback stopped".to_string(),
            );
        }

        if let Err(err) = execute_step(&action.step, &[], executor, &mut pressed_inputs) {
            return cleanup_and_return(&mut pressed_inputs, executor, err);
        }
    }

    pressed_inputs.release_all(executor)
}

fn sleep_with_stop(delay_ms: u64, stop_token: &StopToken) -> Result<(), String> {
    if delay_ms == 0 {
        return Ok(());
    }

    let delay = Duration::from_millis(delay_ms);
    let started = Instant::now();
    loop {
        if stop_token.is_stopped() {
            return Err("playback stopped".to_string());
        }

        let elapsed = started.elapsed();
        if elapsed >= delay {
            return Ok(());
        }

        if stop_token.wait_for_stop(delay.saturating_sub(elapsed)) {
            return Err("playback stopped".to_string());
        }
    }
}

fn sleep_until(started: Instant, target_ms: u64, stop_token: &StopToken) -> Result<(), String> {
    let target = Duration::from_millis(target_ms);
    loop {
        if stop_token.is_stopped() {
            return Err("playback stopped".to_string());
        }

        let elapsed = started.elapsed();
        if elapsed >= target {
            return Ok(());
        }

        if stop_token.wait_for_stop(target.saturating_sub(elapsed)) {
            return Err("playback stopped".to_string());
        }
    }
}

fn execute_step<E: StepExecutor + ?Sized>(
    step: &MacroStep,
    targets: &[WindowTarget],
    executor: &E,
    pressed_inputs: &mut PressedInputs,
) -> Result<u64, String> {
    match step {
        MacroStep::MouseMove { x, y, .. } => {
            executor.mouse_move(*x, *y)?;
            Ok(0)
        }
        MacroStep::MouseButton {
            x,
            y,
            button,
            state,
            ..
        } => {
            executor.mouse_button(*x, *y, *button, *state)?;
            match state {
                ButtonState::Pressed => pressed_inputs.add_mouse_button(*button, *x, *y),
                ButtonState::Released => pressed_inputs.remove_mouse_button(*button),
            }
            Ok(0)
        }
        MacroStep::MouseWheel { x, y, delta, .. } => {
            executor.mouse_wheel(*x, *y, *delta)?;
            Ok(0)
        }
        MacroStep::Key {
            vk_code,
            scan_code,
            extended,
            state,
            ..
        } => {
            executor.key(*vk_code, *scan_code, *extended, *state)?;
            match state {
                KeyState::Pressed => pressed_inputs.add_key(*vk_code, *scan_code, *extended),
                KeyState::Released => pressed_inputs.remove_key(*vk_code, *scan_code, *extended),
            }
            Ok(0)
        }
        MacroStep::PointerMove { position, .. } => match *position {
            PointerPosition::ScreenRelative { x, y } => {
                executor.mouse_move(x, y)?;
                Ok(0)
            }
            PointerPosition::WindowRelative {
                target_id,
                x,
                y,
                intent,
            } => executor.window_mouse_move(
                find_target(targets, target_id)?,
                x,
                y,
                intent,
                pressed_inputs.any_held(),
            ),
        },
        MacroStep::PointerButton {
            position,
            button,
            state,
            ..
        } => {
            let pause_ms = match *position {
                PointerPosition::ScreenRelative { x, y } => {
                    executor.mouse_button(x, y, *button, *state)?;
                    0
                }
                PointerPosition::WindowRelative {
                    target_id,
                    x,
                    y,
                    intent,
                } => executor.window_mouse_button(
                    find_target(targets, target_id)?,
                    x,
                    y,
                    intent,
                    *button,
                    *state,
                    pressed_inputs.any_held(),
                )?,
            };
            match state {
                ButtonState::Pressed => pressed_inputs.add_mouse_button(*button, 0, 0),
                ButtonState::Released => pressed_inputs.remove_mouse_button(*button),
            }
            Ok(pause_ms)
        }
        MacroStep::PointerWheel {
            position, delta, ..
        } => match *position {
            PointerPosition::ScreenRelative { x, y } => {
                executor.mouse_wheel(x, y, *delta)?;
                Ok(0)
            }
            PointerPosition::WindowRelative {
                target_id,
                x,
                y,
                intent,
            } => executor.window_mouse_wheel(
                find_target(targets, target_id)?,
                x,
                y,
                intent,
                *delta,
                pressed_inputs.any_held(),
            ),
        },
        MacroStep::TargetedKey {
            target_id,
            vk_code,
            scan_code,
            extended,
            state,
            ..
        } => {
            let sequence_start = *state == KeyState::Pressed && pressed_inputs.keys.is_empty();
            let target = target_id
                .map(|target_id| find_target(targets, target_id))
                .transpose()?;
            let pause_ms = match target {
                Some(target) => executor.targeted_key(
                    target,
                    *vk_code,
                    *scan_code,
                    *extended,
                    *state,
                    sequence_start,
                    pressed_inputs.any_held(),
                )?,
                None => {
                    executor.key(*vk_code, *scan_code, *extended, *state)?;
                    0
                }
            };
            match state {
                KeyState::Pressed => pressed_inputs.add_key(*vk_code, *scan_code, *extended),
                KeyState::Released => pressed_inputs.remove_key(*vk_code, *scan_code, *extended),
            }
            Ok(pause_ms)
        }
        MacroStep::Wait { .. } => Ok(0),
    }
}

fn find_target(
    targets: &[WindowTarget],
    target_id: TargetWindowId,
) -> Result<&WindowTarget, String> {
    targets
        .iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| format!("目标窗口 {} 不可用。", target_id.0))
}

fn cleanup_and_return<E: StepExecutor + ?Sized>(
    pressed_inputs: &mut PressedInputs,
    executor: &E,
    err: String,
) -> Result<(), String> {
    match pressed_inputs.release_all(executor) {
        Ok(()) => Err(err),
        Err(cleanup_error) => Err(format!("{err}；释放残留输入失败：{cleanup_error}")),
    }
}

#[derive(Default)]
struct PressedInputs {
    keys: Vec<(u16, u16, bool)>,
    mouse_buttons: Vec<MouseButton>,
}

impl PressedInputs {
    fn any_held(&self) -> bool {
        !self.keys.is_empty() || !self.mouse_buttons.is_empty()
    }

    fn add_key(&mut self, vk_code: u16, scan_code: u16, extended: bool) {
        if !self.keys.contains(&(vk_code, scan_code, extended)) {
            self.keys.push((vk_code, scan_code, extended));
        }
    }

    fn remove_key(&mut self, vk_code: u16, scan_code: u16, extended: bool) {
        if let Some(index) = self
            .keys
            .iter()
            .position(|key| *key == (vk_code, scan_code, extended))
        {
            self.keys.remove(index);
        }
    }

    fn add_mouse_button(&mut self, button: MouseButton, _x: i32, _y: i32) {
        if !self.mouse_buttons.contains(&button) {
            self.mouse_buttons.push(button);
        }
    }

    fn remove_mouse_button(&mut self, button: MouseButton) {
        if let Some(index) = self
            .mouse_buttons
            .iter()
            .position(|pressed_button| *pressed_button == button)
        {
            self.mouse_buttons.remove(index);
        }
    }

    fn release_all<E: StepExecutor + ?Sized>(&mut self, executor: &E) -> Result<(), String> {
        let mut first_error = None;

        for (vk_code, scan_code, extended) in self.keys.drain(..).rev() {
            if let Err(error) = executor.key(vk_code, scan_code, extended, KeyState::Released) {
                first_error.get_or_insert(error);
            }
        }

        for button in self.mouse_buttons.drain(..).rev() {
            if let Err(error) = executor.release_mouse_button(button) {
                first_error.get_or_insert(error);
            }
        }

        first_error.map_or(Ok(()), Err)
    }
}
