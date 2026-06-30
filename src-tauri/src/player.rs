use crate::model::{MacroStep, Recording};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackSettings {
    pub loop_count: u32,
    pub speed_multiplier: f64,
}

impl PlaybackSettings {
    pub fn new(loop_count: u32, speed_multiplier: f64) -> Result<Self, String> {
        if loop_count == 0 {
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

pub fn build_playback_plan(
    recording: &Recording,
    settings: PlaybackSettings,
) -> Vec<PlaybackAction> {
    let mut actions = Vec::new();
    for loop_index in 0..settings.loop_count {
        let mut previous_elapsed = 0;
        for (step_index, step) in recording.steps.iter().cloned().enumerate() {
            let elapsed = step.elapsed_ms();
            let raw_delay = elapsed.saturating_sub(previous_elapsed);
            previous_elapsed = elapsed;
            actions.push(PlaybackAction {
                loop_index,
                step_index,
                delay_ms: scaled_delay_ms(raw_delay, settings.speed_multiplier),
                step,
            });
        }
    }
    actions
}
