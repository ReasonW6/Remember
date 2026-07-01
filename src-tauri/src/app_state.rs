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
        self.start_recording_inner(name, started_at_ms, created_at, false)
    }

    pub fn start_recording_from_hotkey(
        &mut self,
        name: impl Into<String>,
        started_at_ms: u64,
        created_at: impl Into<String>,
    ) -> Result<(), String> {
        self.start_recording_inner(name, started_at_ms, created_at, true)
    }

    fn start_recording_inner(
        &mut self,
        name: impl Into<String>,
        started_at_ms: u64,
        created_at: impl Into<String>,
        suppress_record_hotkey_release_tail: bool,
    ) -> Result<(), String> {
        match self.mode {
            AppMode::Idle => {}
            AppMode::Recording => return Err("cannot record while recording".to_string()),
            AppMode::Playing => return Err("cannot record while playing".to_string()),
        }
        self.recorder.start(name, started_at_ms, created_at)?;
        self.control_hotkeys.reset();
        if suppress_record_hotkey_release_tail {
            self.control_hotkeys.suppress_release_tail(0x52);
        }
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

    pub fn stop_active(&mut self, stopped_at_ms: u64) -> Result<(), String> {
        match self.mode {
            AppMode::Recording => {
                self.stop_recording(stopped_at_ms)?;
            }
            AppMode::Playing => self.stop_playback(),
            AppMode::Idle => {}
        }
        Ok(())
    }

    pub fn stop_token(&self) -> StopToken {
        self.stop_token.clone()
    }
}

#[derive(Default)]
struct ControlHotkeySuppressor {
    pending_modifiers: Vec<RawInputEvent>,
    active_modifiers: Vec<u16>,
    suppressed_modifier_releases: Vec<u16>,
    suppress_unknown_modifier_release_tail: bool,
    suppressed_key: Option<u16>,
}

impl ControlHotkeySuppressor {
    fn reset(&mut self) {
        self.pending_modifiers.clear();
        self.active_modifiers.clear();
        self.suppressed_modifier_releases.clear();
        self.suppress_unknown_modifier_release_tail = false;
        self.suppressed_key = None;
    }

    fn suppress_release_tail(&mut self, vk_code: u16) {
        self.pending_modifiers.clear();
        self.active_modifiers.clear();
        self.suppressed_modifier_releases.clear();
        self.suppressed_key = Some(vk_code);
        self.suppress_unknown_modifier_release_tail = true;
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
        if is_modifier(vk_code) {
            self.add_active_modifier(vk_code);
            self.pending_modifiers.push(event);
            return Vec::new();
        }

        if self.ctrl_down() && self.alt_down() && is_control_hotkey_key(vk_code) {
            self.pending_modifiers.clear();
            self.suppressed_key = Some(vk_code);
            self.suppressed_modifier_releases = self.active_modifiers.clone();
            self.suppress_unknown_modifier_release_tail = false;
            return Vec::new();
        }

        self.flush_with(event)
    }

    fn filter_key_released(&mut self, event: RawInputEvent, vk_code: u16) -> Vec<RawInputEvent> {
        if self.suppressed_key == Some(vk_code) {
            self.suppressed_key = None;
            return Vec::new();
        }

        if is_modifier(vk_code) {
            let was_active = self.remove_active_modifier(vk_code);
            if self.remove_suppressed_modifier(vk_code)
                || (!was_active && self.suppress_unknown_modifier_release_tail)
            {
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

    fn add_active_modifier(&mut self, vk_code: u16) {
        if !self.active_modifiers.contains(&vk_code) {
            self.active_modifiers.push(vk_code);
        }
    }

    fn remove_active_modifier(&mut self, vk_code: u16) -> bool {
        remove_first(&mut self.active_modifiers, vk_code)
    }

    fn remove_suppressed_modifier(&mut self, vk_code: u16) -> bool {
        remove_first(&mut self.suppressed_modifier_releases, vk_code)
    }

    fn ctrl_down(&self) -> bool {
        self.active_modifiers
            .iter()
            .any(|vk_code| is_ctrl(*vk_code))
    }

    fn alt_down(&self) -> bool {
        self.active_modifiers.iter().any(|vk_code| is_alt(*vk_code))
    }
}

fn remove_first(values: &mut Vec<u16>, value: u16) -> bool {
    if let Some(index) = values.iter().position(|existing| *existing == value) {
        values.remove(index);
        true
    } else {
        false
    }
}

fn is_modifier(vk_code: u16) -> bool {
    is_ctrl(vk_code) || is_alt(vk_code)
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
