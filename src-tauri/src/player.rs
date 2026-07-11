use crate::model::{ButtonState, KeyState, MacroStep, MouseButton, Recording};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

const STOP_SLEEP_CHUNK: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackSettings {
    pub loop_count: Option<u32>,
    pub speed_multiplier: f64,
}

impl PlaybackSettings {
    pub fn new(loop_count: Option<u32>, speed_multiplier: f64) -> Result<Self, String> {
        if loop_count == Some(0) {
            return Err("loop count must be at least 1".to_string());
        }
        if !speed_multiplier.is_finite() || speed_multiplier <= 0.0 {
            return Err("speed multiplier must be positive".to_string());
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

    fn release_mouse_button(&self, button: MouseButton) -> Result<(), String>;
}

#[derive(Clone, Default)]
pub struct StopToken {
    stopped: Arc<AtomicBool>,
}

impl StopToken {
    pub fn request_stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
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

    if recording.steps.is_empty() && recording.duration_ms == 0 {
        return match settings.loop_count {
            Some(_) => Ok(()),
            None => {
                while !stop_token.is_stopped() {
                    thread::sleep(STOP_SLEEP_CHUNK);
                }
                Err("playback stopped".to_string())
            }
        };
    }

    let mut pressed_inputs = PressedInputs::default();
    let mut completed_loops = 0_u32;

    loop {
        if settings
            .loop_count
            .is_some_and(|loop_count| completed_loops >= loop_count)
        {
            return pressed_inputs.release_all(executor);
        }
        if stop_token.is_stopped() {
            return cleanup_and_return(
                &mut pressed_inputs,
                executor,
                "playback stopped".to_string(),
            );
        }

        let loop_started = Instant::now();
        for step in &recording.steps {
            let target_ms = scaled_delay_ms(step.elapsed_ms(), settings.speed_multiplier);
            if let Err(error) = sleep_until(loop_started, target_ms, stop_token) {
                return cleanup_and_return(&mut pressed_inputs, executor, error);
            }
            if let Err(error) = execute_step(step, executor, &mut pressed_inputs) {
                return cleanup_and_return(&mut pressed_inputs, executor, error);
            }
        }

        let duration_ms = scaled_delay_ms(recording.duration_ms, settings.speed_multiplier);
        if let Err(error) = sleep_until(loop_started, duration_ms, stop_token) {
            return cleanup_and_return(&mut pressed_inputs, executor, error);
        }
        completed_loops = completed_loops.saturating_add(1);
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

        if let Err(err) = execute_step(&action.step, executor, &mut pressed_inputs) {
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

        let remaining = delay.saturating_sub(elapsed);
        let chunk = if remaining > STOP_SLEEP_CHUNK {
            STOP_SLEEP_CHUNK
        } else {
            remaining
        };
        thread::sleep(chunk);
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

        thread::sleep(target.saturating_sub(elapsed).min(STOP_SLEEP_CHUNK));
    }
}

fn execute_step<E: StepExecutor + ?Sized>(
    step: &MacroStep,
    executor: &E,
    pressed_inputs: &mut PressedInputs,
) -> Result<(), String> {
    match step {
        MacroStep::MouseMove { x, y, .. } => executor.mouse_move(*x, *y),
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
            Ok(())
        }
        MacroStep::MouseWheel { x, y, delta, .. } => executor.mouse_wheel(*x, *y, *delta),
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
            Ok(())
        }
        MacroStep::Wait { .. } => Ok(()),
    }
}

fn cleanup_and_return<E: StepExecutor + ?Sized>(
    pressed_inputs: &mut PressedInputs,
    executor: &E,
    err: String,
) -> Result<(), String> {
    match pressed_inputs.release_all(executor) {
        Ok(()) => Err(err),
        Err(cleanup_error) => Err(format!("{err}; input cleanup failed: {cleanup_error}")),
    }
}

#[derive(Default)]
struct PressedInputs {
    keys: Vec<(u16, u16, bool)>,
    mouse_buttons: Vec<MouseButton>,
}

impl PressedInputs {
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
