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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlHotkeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlHotkey {
    pub vk_code: u16,
    pub modifiers: ControlHotkeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlHotkeyAction {
    Record,
    Playback,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlHotkeyDecision {
    pub suppress: bool,
    pub action: Option<ControlHotkeyAction>,
}

impl ControlHotkeyDecision {
    const PASS: Self = Self {
        suppress: false,
        action: None,
    };
}

pub struct AppController {
    mode: AppMode,
    recorder: Recorder,
    control_hotkeys: ControlHotkeySuppressor,
    playback_settings: PlaybackSettings,
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
            playback_settings: PlaybackSettings {
                loop_count: 1,
                speed_multiplier: 1.0,
            },
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
            self.control_hotkeys.suppress_record_hotkey_release_tail();
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
            return;
        }

        for event in self.control_hotkeys.filter(event) {
            self.recorder.capture(event);
        }
    }

    pub fn control_hotkey_action(&mut self, event: RawInputEvent) -> Option<ControlHotkeyAction> {
        self.control_hotkey_decision(event).action
    }

    pub fn control_hotkey_decision(&mut self, event: RawInputEvent) -> ControlHotkeyDecision {
        self.control_hotkeys.decide(event, self.mode)
    }

    pub fn reset_control_hotkey_state(&mut self) {
        self.control_hotkeys.reset();
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
        let settings = PlaybackSettings::new(loop_count, speed_multiplier)?;
        self.start_playback_with_settings(settings)
    }

    pub fn set_playback_settings(
        &mut self,
        loop_count: u32,
        speed_multiplier: f64,
    ) -> Result<(), String> {
        self.playback_settings = PlaybackSettings::new(loop_count, speed_multiplier)?;
        Ok(())
    }

    pub fn start_playback_with_current_settings(&mut self) -> Result<PlaybackRun, String> {
        self.start_playback_with_settings(self.playback_settings)
    }

    fn start_playback_with_settings(
        &mut self,
        settings: PlaybackSettings,
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

    pub fn set_control_hotkeys(
        &mut self,
        hotkeys: Vec<ControlHotkey>,
        record_hotkey: ControlHotkey,
        playback_hotkey: ControlHotkey,
        stop_hotkey: ControlHotkey,
    ) {
        self.control_hotkeys
            .set_hotkeys(hotkeys, record_hotkey, playback_hotkey, stop_hotkey);
    }
}

struct ControlHotkeySuppressor {
    pending_modifiers: Vec<RawInputEvent>,
    active_modifiers: Vec<u16>,
    // The keyboard hook calls decide() and filter() for the same event, so the
    // action path tracks its own modifier state to avoid stealing releases
    // from the recording filter.
    action_active_modifiers: Vec<u16>,
    action_suppressed_key: Option<u16>,
    suppressed_modifier_releases: Vec<u16>,
    suppress_unknown_modifier_release_tail: bool,
    suppressed_key: Option<u16>,
    hotkeys: Vec<ControlHotkey>,
    record_hotkey: ControlHotkey,
    playback_hotkey: ControlHotkey,
    stop_hotkey: ControlHotkey,
}

impl Default for ControlHotkeySuppressor {
    fn default() -> Self {
        let default_record = ControlHotkey {
            vk_code: 0x77,
            modifiers: ControlHotkeyModifiers::default(),
        };
        Self {
            pending_modifiers: Vec::new(),
            active_modifiers: Vec::new(),
            action_active_modifiers: Vec::new(),
            action_suppressed_key: None,
            suppressed_modifier_releases: Vec::new(),
            suppress_unknown_modifier_release_tail: false,
            suppressed_key: None,
            hotkeys: vec![
                default_record,
                ControlHotkey {
                    vk_code: 0x7B,
                    modifiers: ControlHotkeyModifiers::default(),
                },
            ],
            record_hotkey: default_record,
            playback_hotkey: ControlHotkey {
                vk_code: 0x7B,
                modifiers: ControlHotkeyModifiers::default(),
            },
            stop_hotkey: default_record,
        }
    }
}

impl ControlHotkeySuppressor {
    fn set_hotkeys(
        &mut self,
        hotkeys: Vec<ControlHotkey>,
        record_hotkey: ControlHotkey,
        playback_hotkey: ControlHotkey,
        stop_hotkey: ControlHotkey,
    ) {
        self.hotkeys = hotkeys;
        self.record_hotkey = record_hotkey;
        self.playback_hotkey = playback_hotkey;
        self.stop_hotkey = stop_hotkey;
        self.reset();
    }

    fn decide(&mut self, event: RawInputEvent, mode: AppMode) -> ControlHotkeyDecision {
        let RawInputEvent::Key { vk_code, state, .. } = event else {
            return ControlHotkeyDecision::PASS;
        };

        match state {
            KeyState::Pressed if is_modifier(vk_code) => {
                add_modifier(&mut self.action_active_modifiers, vk_code);
                ControlHotkeyDecision::PASS
            }
            KeyState::Released if is_modifier(vk_code) => {
                remove_first(&mut self.action_active_modifiers, vk_code);
                ControlHotkeyDecision::PASS
            }
            KeyState::Pressed => {
                if !self.is_action_hotkey(vk_code) {
                    return ControlHotkeyDecision::PASS;
                }
                self.action_suppressed_key = Some(vk_code);
                ControlHotkeyDecision {
                    suppress: true,
                    action: self.action_for_key(vk_code, mode),
                }
            }
            KeyState::Released => {
                if self.action_suppressed_key == Some(vk_code) {
                    self.action_suppressed_key = None;
                    return ControlHotkeyDecision {
                        suppress: true,
                        action: None,
                    };
                }
                ControlHotkeyDecision::PASS
            }
        }
    }

    fn is_action_hotkey(&self, vk_code: u16) -> bool {
        self.hotkeys
            .iter()
            .any(|hotkey| matches_hotkey(&self.action_active_modifiers, *hotkey, vk_code))
    }

    fn action_for_key(&self, vk_code: u16, mode: AppMode) -> Option<ControlHotkeyAction> {
        if matches_hotkey(&self.action_active_modifiers, self.record_hotkey, vk_code) {
            return match mode {
                AppMode::Idle => Some(ControlHotkeyAction::Record),
                AppMode::Recording | AppMode::Playing if self.record_hotkey == self.stop_hotkey => {
                    Some(ControlHotkeyAction::Stop)
                }
                AppMode::Recording | AppMode::Playing => None,
            };
        }

        if matches_hotkey(&self.action_active_modifiers, self.playback_hotkey, vk_code) {
            return match mode {
                AppMode::Idle => Some(ControlHotkeyAction::Playback),
                AppMode::Recording | AppMode::Playing => None,
            };
        }

        if matches_hotkey(&self.action_active_modifiers, self.stop_hotkey, vk_code) {
            return match mode {
                AppMode::Recording | AppMode::Playing => Some(ControlHotkeyAction::Stop),
                AppMode::Idle => None,
            };
        }

        None
    }

    fn reset(&mut self) {
        self.pending_modifiers.clear();
        self.active_modifiers.clear();
        self.action_active_modifiers.clear();
        self.action_suppressed_key = None;
        self.suppressed_modifier_releases.clear();
        self.suppress_unknown_modifier_release_tail = false;
        self.suppressed_key = None;
    }

    fn suppress_record_hotkey_release_tail(&mut self) {
        self.pending_modifiers.clear();
        self.active_modifiers.clear();
        self.suppressed_modifier_releases.clear();
        self.suppressed_key = Some(self.record_hotkey.vk_code);
        self.action_suppressed_key = Some(self.record_hotkey.vk_code);
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

        if self.is_control_hotkey(vk_code) {
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
        add_modifier(&mut self.active_modifiers, vk_code);
    }

    fn remove_active_modifier(&mut self, vk_code: u16) -> bool {
        remove_first(&mut self.active_modifiers, vk_code)
    }

    fn remove_suppressed_modifier(&mut self, vk_code: u16) -> bool {
        remove_first(&mut self.suppressed_modifier_releases, vk_code)
    }

    fn is_control_hotkey(&self, vk_code: u16) -> bool {
        self.hotkeys
            .iter()
            .any(|hotkey| matches_hotkey(&self.active_modifiers, *hotkey, vk_code))
    }
}

fn matches_hotkey(active_modifiers: &[u16], hotkey: ControlHotkey, vk_code: u16) -> bool {
    hotkey.vk_code == vk_code && modifiers_match(active_modifiers, hotkey.modifiers)
}

fn modifiers_match(active_modifiers: &[u16], modifiers: ControlHotkeyModifiers) -> bool {
    modifiers.ctrl == modifier_down(active_modifiers, is_ctrl)
        && modifiers.alt == modifier_down(active_modifiers, is_alt)
        && modifiers.shift == modifier_down(active_modifiers, is_shift)
        && modifiers.meta == modifier_down(active_modifiers, is_meta)
}

fn modifier_down(active_modifiers: &[u16], predicate: fn(u16) -> bool) -> bool {
    active_modifiers.iter().any(|vk_code| predicate(*vk_code))
}

fn add_modifier(modifiers: &mut Vec<u16>, vk_code: u16) {
    if !modifiers.contains(&vk_code) {
        modifiers.push(vk_code);
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
    is_ctrl(vk_code) || is_alt(vk_code) || is_shift(vk_code) || is_meta(vk_code)
}

fn is_ctrl(vk_code: u16) -> bool {
    matches!(vk_code, 0x11 | 0xA2 | 0xA3)
}

fn is_alt(vk_code: u16) -> bool {
    matches!(vk_code, 0x12 | 0xA4 | 0xA5)
}

fn is_shift(vk_code: u16) -> bool {
    matches!(vk_code, 0x10 | 0xA0 | 0xA1)
}

fn is_meta(vk_code: u16) -> bool {
    matches!(vk_code, 0x5B | 0x5C)
}
