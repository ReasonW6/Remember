use crate::{
    model::{KeyState, Recording},
    player::{build_playback_plan, PlaybackAction, PlaybackSettings, StopToken},
    recorder::{RawInputEvent, Recorder},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    Idle,
    Recording,
    Playing,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiState {
    pub mode: AppMode,
    pub recording_name: Option<String>,
    pub step_count: usize,
    pub duration_ms: u64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackRunId(u64);

#[derive(Debug, Clone)]
pub struct PlaybackRun {
    pub id: PlaybackRunId,
    pub actions: Vec<PlaybackAction>,
}

pub struct AppController {
    mode: AppMode,
    recorder: Recorder,
    control_hotkeys: ControlHotkeySuppressor,
    recording: Option<Recording>,
    stop_token: StopToken,
    next_playback_id: u64,
    active_playback_id: Option<PlaybackRunId>,
    message: String,
}

impl Default for AppController {
    fn default() -> Self {
        Self::new()
    }
}

impl AppController {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Idle,
            recorder: Recorder::new(50),
            control_hotkeys: ControlHotkeySuppressor::default(),
            recording: None,
            stop_token: StopToken::default(),
            next_playback_id: 0,
            active_playback_id: None,
            message: "Idle".to_string(),
        }
    }

    pub fn mode(&self) -> AppMode {
        self.mode
    }

    pub fn ui_state(&self) -> UiState {
        UiState {
            mode: self.mode,
            recording_name: self
                .recording
                .as_ref()
                .map(|recording| recording.name.clone()),
            step_count: self
                .recording
                .as_ref()
                .map(|recording| recording.steps.len())
                .unwrap_or(0),
            duration_ms: self
                .recording
                .as_ref()
                .map(|recording| recording.duration_ms)
                .unwrap_or(0),
            message: self.message.clone(),
        }
    }

    pub fn start_recording(
        &mut self,
        name: impl Into<String>,
        started_at_ms: u64,
        created_at: impl Into<String>,
    ) -> Result<(), String> {
        match self.mode {
            AppMode::Idle => {}
            AppMode::Recording => return Err("cannot record while recording".to_string()),
            AppMode::Playing => return Err("cannot record while playing".to_string()),
        }
        self.recorder.start(name, started_at_ms, created_at)?;
        self.control_hotkeys.reset();
        self.recording = None;
        self.mode = AppMode::Recording;
        self.message = "Recording".to_string();
        Ok(())
    }

    pub fn stop_recording(&mut self, stopped_at_ms: u64) -> Result<Recording, String> {
        let recording = self.recorder.stop(stopped_at_ms)?;
        self.control_hotkeys.reset();
        self.recording = Some(recording.clone());
        self.mode = AppMode::Idle;
        self.message = "Recording stopped".to_string();
        Ok(recording)
    }

    pub fn capture_input(&mut self, event: RawInputEvent) {
        if self.mode != AppMode::Recording {
            self.control_hotkeys.reset();
            return;
        }

        for event in self.control_hotkeys.filter(event) {
            self.recorder.capture(event);
        }
    }

    pub fn set_recording(&mut self, recording: Recording) -> Result<(), String> {
        match self.mode {
            AppMode::Idle => {}
            AppMode::Recording => return Err("cannot load recording while recording".to_string()),
            AppMode::Playing => return Err("cannot load recording while playing".to_string()),
        }
        recording.validate()?;
        self.recording = Some(recording);
        self.mode = AppMode::Idle;
        self.message = "Recording loaded".to_string();
        Ok(())
    }

    pub fn current_recording(&self) -> Option<&Recording> {
        self.recording.as_ref()
    }

    pub fn saveable_recording(&self) -> Result<Recording, String> {
        self.recording
            .clone()
            .ok_or_else(|| "no recording loaded".to_string())
    }

    pub fn mark_idle(&mut self, message: impl Into<String>) {
        self.mode = AppMode::Idle;
        self.active_playback_id = None;
        self.message = message.into();
    }

    pub fn finish_playback_if_current(
        &mut self,
        id: PlaybackRunId,
        message: impl Into<String>,
    ) -> bool {
        if self.mode != AppMode::Playing || self.active_playback_id != Some(id) {
            return false;
        }

        self.mode = AppMode::Idle;
        self.active_playback_id = None;
        self.message = message.into();
        true
    }

    pub fn start_playback(
        &mut self,
        loop_count: u32,
        speed_multiplier: f64,
    ) -> Result<PlaybackRun, String> {
        match self.mode {
            AppMode::Idle => {}
            AppMode::Recording => return Err("cannot play while recording".to_string()),
            AppMode::Playing => return Err("cannot play while playing".to_string()),
        }
        let recording = self
            .recording
            .as_ref()
            .ok_or_else(|| "no recording loaded".to_string())?;
        let settings = PlaybackSettings::new(loop_count, speed_multiplier)?;
        self.stop_token = StopToken::default();
        self.next_playback_id += 1;
        let id = PlaybackRunId(self.next_playback_id);
        self.active_playback_id = Some(id);
        self.mode = AppMode::Playing;
        self.message = "Playing".to_string();
        Ok(PlaybackRun {
            id,
            actions: build_playback_plan(recording, settings),
        })
    }

    pub fn stop_playback(&mut self) {
        if self.mode != AppMode::Playing {
            return;
        }
        self.stop_token.request_stop();
        self.mode = AppMode::Idle;
        self.active_playback_id = None;
        self.message = "Playback stopped".to_string();
    }

    pub fn stop_token(&self) -> StopToken {
        self.stop_token.clone()
    }
}

#[derive(Default)]
struct ControlHotkeySuppressor {
    pending_modifiers: Vec<RawInputEvent>,
    ctrl_down: bool,
    alt_down: bool,
    suppress_ctrl_release: bool,
    suppress_alt_release: bool,
    suppressed_key: Option<u16>,
}

impl ControlHotkeySuppressor {
    fn reset(&mut self) {
        self.pending_modifiers.clear();
        self.ctrl_down = false;
        self.alt_down = false;
        self.suppress_ctrl_release = false;
        self.suppress_alt_release = false;
        self.suppressed_key = None;
    }

    fn filter(&mut self, event: RawInputEvent) -> Vec<RawInputEvent> {
        let RawInputEvent::Key { vk_code, state, .. } = event else {
            return self.flush_with(event);
        };

        match state {
            KeyState::Pressed => self.filter_key_pressed(event, vk_code),
            KeyState::Released => self.filter_key_released(event, vk_code),
        }
    }

    fn filter_key_pressed(&mut self, event: RawInputEvent, vk_code: u16) -> Vec<RawInputEvent> {
        if is_ctrl(vk_code) {
            self.ctrl_down = true;
            self.pending_modifiers.push(event);
            return Vec::new();
        }

        if is_alt(vk_code) {
            self.alt_down = true;
            self.pending_modifiers.push(event);
            return Vec::new();
        }

        if self.ctrl_down && self.alt_down && is_control_hotkey_key(vk_code) {
            self.pending_modifiers.clear();
            self.suppressed_key = Some(vk_code);
            self.suppress_ctrl_release = true;
            self.suppress_alt_release = true;
            return Vec::new();
        }

        self.flush_with(event)
    }

    fn filter_key_released(&mut self, event: RawInputEvent, vk_code: u16) -> Vec<RawInputEvent> {
        if self.suppressed_key == Some(vk_code) {
            self.suppressed_key = None;
            return Vec::new();
        }

        if is_ctrl(vk_code) {
            self.ctrl_down = false;
            if self.suppress_ctrl_release {
                self.suppress_ctrl_release = false;
                return Vec::new();
            }
            return self.flush_with(event);
        }

        if is_alt(vk_code) {
            self.alt_down = false;
            if self.suppress_alt_release {
                self.suppress_alt_release = false;
                return Vec::new();
            }
            return self.flush_with(event);
        }

        self.flush_with(event)
    }

    fn flush_with(&mut self, event: RawInputEvent) -> Vec<RawInputEvent> {
        let mut events = std::mem::take(&mut self.pending_modifiers);
        events.push(event);
        events
    }
}

fn is_ctrl(vk_code: u16) -> bool {
    matches!(vk_code, 0x11 | 0xA2 | 0xA3)
}

fn is_alt(vk_code: u16) -> bool {
    matches!(vk_code, 0x12 | 0xA4 | 0xA5)
}

fn is_control_hotkey_key(vk_code: u16) -> bool {
    matches!(vk_code, 0x52 | 0x50 | 0x1B)
}
