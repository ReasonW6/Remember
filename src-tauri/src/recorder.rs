use crate::model::{
    ButtonState, ClientSize, KeyState, MacroStep, MouseButton, PointerPosition, Recording,
    TargetWindowAvailability, TargetWindowId, WindowPointerIntent, WindowTarget, RECORDING_VERSION,
    RECORDING_VERSION_V2,
};
use std::collections::HashMap;

pub const MAX_RECORDING_STEPS: usize = 250_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawInputEvent {
    MouseMove {
        at_ms: u64,
        x: i32,
        y: i32,
    },
    MouseButton {
        at_ms: u64,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    },
    MouseWheel {
        at_ms: u64,
        x: i32,
        y: i32,
        delta: i32,
    },
    Key {
        at_ms: u64,
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
    },
}

impl RawInputEvent {
    fn at_ms(self) -> u64 {
        match self {
            Self::MouseMove { at_ms, .. }
            | Self::MouseButton { at_ms, .. }
            | Self::MouseWheel { at_ms, .. }
            | Self::Key { at_ms, .. } => at_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowInstanceId {
    pub hwnd: usize,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedWindow {
    pub instance: WindowInstanceId,
    pub executable_path: String,
    pub window_class: String,
    pub title: String,
    pub client_origin_x: i32,
    pub client_origin_y: i32,
    pub client_size: ClientSize,
    pub dpi: u32,
    pub availability: TargetWindowAvailability,
    pub direct_pointer_target: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureSurface {
    Screen,
    Window {
        window: CapturedWindow,
        intent: WindowPointerIntent,
    },
    Unreadable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOutcome {
    Continue,
    UnreadableStarted(String),
    UnreadableEnded,
    StopRecording(String),
}

#[derive(Debug, Clone)]
pub struct Recorder {
    move_sample_interval_ms: u64,
    active: Option<ActiveRecording>,
    last_stop_was_truncated: bool,
}

impl Recorder {
    pub fn new(move_sample_interval_ms: u64) -> Self {
        Self {
            move_sample_interval_ms,
            active: None,
            last_stop_was_truncated: false,
        }
    }

    pub fn start(
        &mut self,
        name: impl Into<String>,
        start_ms: u64,
        created_at: impl Into<String>,
    ) -> Result<(), String> {
        self.start_version(name, start_ms, created_at, RECORDING_VERSION)
    }

    pub fn start_window_relative(
        &mut self,
        name: impl Into<String>,
        start_ms: u64,
        created_at: impl Into<String>,
    ) -> Result<(), String> {
        self.start_version(name, start_ms, created_at, RECORDING_VERSION_V2)
    }

    fn start_version(
        &mut self,
        name: impl Into<String>,
        start_ms: u64,
        created_at: impl Into<String>,
        version: u32,
    ) -> Result<(), String> {
        if self.active.is_some() {
            return Err("already recording".to_string());
        }

        self.last_stop_was_truncated = false;
        self.active = Some(ActiveRecording {
            version,
            name: name.into(),
            created_at: created_at.into(),
            start_ms,
            steps: Vec::new(),
            targets: Vec::new(),
            target_by_instance: HashMap::new(),
            target_geometry_by_instance: HashMap::new(),
            step_limit_exceeded: false,
            halt_reason: None,
            last_mouse_move_elapsed_ms: None,
            last_mouse_move_position: None,
            last_readable_pointer: None,
            pointer_unreadable_reason: None,
            physically_pressed_mouse_buttons: Vec::new(),
            recorded_pressed_mouse_buttons: Vec::new(),
            pointer_gesture_anchor: None,
            physically_pressed_keys: Vec::new(),
            keyboard_sequence: None,
        });
        Ok(())
    }

    pub fn capture(&mut self, event: RawInputEvent) {
        let Some(active) = &mut self.active else {
            return;
        };
        if active.version == RECORDING_VERSION {
            active.capture_v1(event, self.move_sample_interval_ms);
        } else {
            let _ = active.capture_v2(event, CaptureSurface::Screen, self.move_sample_interval_ms);
        }
    }

    pub fn capture_with_surface(
        &mut self,
        event: RawInputEvent,
        surface: CaptureSurface,
    ) -> CaptureOutcome {
        let Some(active) = &mut self.active else {
            return CaptureOutcome::Continue;
        };
        if active.version == RECORDING_VERSION {
            active.capture_v1(event, self.move_sample_interval_ms);
            CaptureOutcome::Continue
        } else {
            active.capture_v2(event, surface, self.move_sample_interval_ms)
        }
    }

    pub fn stop(&mut self, stop_ms: u64) -> Result<Recording, String> {
        let Some(active) = self.active.take() else {
            return Err("not recording".to_string());
        };
        self.last_stop_was_truncated = active.step_limit_exceeded;

        let duration_ms = stop_ms
            .saturating_sub(active.start_ms)
            .max(active.last_emitted_elapsed_ms().unwrap_or(0));

        Ok(Recording {
            version: active.version,
            name: active.name,
            created_at: active.created_at,
            duration_ms,
            targets: active.targets,
            steps: active.steps,
        })
    }

    pub fn is_recording(&self) -> bool {
        self.active.is_some()
    }

    pub fn last_stop_was_truncated(&self) -> bool {
        self.last_stop_was_truncated
    }
}

#[derive(Debug, Clone)]
struct ActiveRecording {
    version: u32,
    name: String,
    created_at: String,
    start_ms: u64,
    steps: Vec<MacroStep>,
    targets: Vec<WindowTarget>,
    target_by_instance: HashMap<WindowInstanceId, usize>,
    target_geometry_by_instance: HashMap<WindowInstanceId, ClientSize>,
    step_limit_exceeded: bool,
    halt_reason: Option<String>,
    last_mouse_move_elapsed_ms: Option<u64>,
    last_mouse_move_position: Option<PointerPosition>,
    last_readable_pointer: Option<PointerPosition>,
    pointer_unreadable_reason: Option<String>,
    physically_pressed_mouse_buttons: Vec<MouseButton>,
    recorded_pressed_mouse_buttons: Vec<MouseButton>,
    pointer_gesture_anchor: Option<PointerGestureAnchor>,
    physically_pressed_keys: Vec<PhysicalKey>,
    keyboard_sequence: Option<KeyboardSequence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PointerGestureAnchor {
    instance: WindowInstanceId,
    target_id: TargetWindowId,
    client_origin_x: i32,
    client_origin_y: i32,
    step_start_index: usize,
    adjusting_window: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhysicalKey {
    vk_code: u16,
    scan_code: u16,
    extended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KeyboardSequence {
    Recording(Option<TargetWindowId>),
    Skipping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerEventKind {
    Move,
    Button,
    Wheel,
}

impl ActiveRecording {
    fn elapsed_ms(&self, at_ms: u64) -> u64 {
        at_ms.saturating_sub(self.start_ms)
    }

    fn last_emitted_elapsed_ms(&self) -> Option<u64> {
        self.steps.last().map(MacroStep::elapsed_ms)
    }

    fn is_stale_elapsed(&self, elapsed_ms: u64) -> bool {
        self.last_emitted_elapsed_ms()
            .map(|last| elapsed_ms < last)
            .unwrap_or(false)
    }

    fn push_step(&mut self, step: MacroStep) -> bool {
        if self.steps.len() >= MAX_RECORDING_STEPS {
            self.step_limit_exceeded = true;
            return false;
        }

        self.steps.push(step);
        true
    }

    fn capture_v1(&mut self, event: RawInputEvent, move_sample_interval_ms: u64) {
        if self.step_limit_exceeded {
            return;
        }

        match event {
            RawInputEvent::MouseMove { at_ms, x, y } => {
                self.capture_mouse_move_v1(at_ms, x, y, move_sample_interval_ms);
            }
            RawInputEvent::MouseButton {
                at_ms,
                x,
                y,
                button,
                state,
            } => {
                let elapsed_ms = self.elapsed_ms(at_ms);
                if self.is_stale_elapsed(elapsed_ms) {
                    return;
                }
                update_button_state(&mut self.physically_pressed_mouse_buttons, button, state);
                self.push_step(MacroStep::MouseButton {
                    elapsed_ms,
                    x,
                    y,
                    button,
                    state,
                });
            }
            RawInputEvent::MouseWheel { at_ms, x, y, delta } => {
                let elapsed_ms = self.elapsed_ms(at_ms);
                if self.is_stale_elapsed(elapsed_ms) {
                    return;
                }
                self.push_step(MacroStep::MouseWheel {
                    elapsed_ms,
                    x,
                    y,
                    delta,
                });
            }
            RawInputEvent::Key {
                at_ms,
                vk_code,
                scan_code,
                extended,
                state,
            } => {
                let elapsed_ms = self.elapsed_ms(at_ms);
                if self.is_stale_elapsed(elapsed_ms) {
                    return;
                }
                self.push_step(MacroStep::Key {
                    elapsed_ms,
                    vk_code,
                    scan_code,
                    extended,
                    state,
                });
            }
        }
    }

    fn capture_mouse_move_v1(&mut self, at_ms: u64, x: i32, y: i32, move_sample_interval_ms: u64) {
        let elapsed_ms = self.elapsed_ms(at_ms);
        if self.is_stale_elapsed(elapsed_ms) {
            return;
        }

        let should_sample = !self.physically_pressed_mouse_buttons.is_empty()
            || self
                .last_mouse_move_elapsed_ms
                .map(|last| elapsed_ms.saturating_sub(last) >= move_sample_interval_ms)
                .unwrap_or(true);

        if should_sample && self.push_step(MacroStep::MouseMove { elapsed_ms, x, y }) {
            self.last_mouse_move_elapsed_ms = Some(elapsed_ms);
        }
    }

    fn capture_v2(
        &mut self,
        event: RawInputEvent,
        surface: CaptureSurface,
        move_sample_interval_ms: u64,
    ) -> CaptureOutcome {
        if self.step_limit_exceeded {
            return CaptureOutcome::Continue;
        }
        if let Some(reason) = &self.halt_reason {
            return CaptureOutcome::StopRecording(reason.clone());
        }

        let elapsed_ms = self.elapsed_ms(event.at_ms());
        if self.is_stale_elapsed(elapsed_ms) {
            return CaptureOutcome::Continue;
        }

        let unreadable_before = self.pointer_unreadable_reason.clone();
        let result = match event {
            RawInputEvent::MouseMove { x, y, .. } => self.capture_v2_pointer(
                event,
                x,
                y,
                PointerEventKind::Move,
                elapsed_ms,
                surface,
                move_sample_interval_ms,
            ),
            RawInputEvent::MouseButton { x, y, .. } => self.capture_v2_pointer(
                event,
                x,
                y,
                PointerEventKind::Button,
                elapsed_ms,
                surface,
                move_sample_interval_ms,
            ),
            RawInputEvent::MouseWheel { x, y, .. } => self.capture_v2_pointer(
                event,
                x,
                y,
                PointerEventKind::Wheel,
                elapsed_ms,
                surface,
                move_sample_interval_ms,
            ),
            RawInputEvent::Key { .. } => self.capture_v2_key(event, elapsed_ms, surface),
        };

        if let Err(reason) = result {
            self.halt_reason = Some(reason.clone());
            return CaptureOutcome::StopRecording(reason);
        }

        let unreadable_after = self.pointer_unreadable_reason.clone();
        unreadable_transition(unreadable_before, unreadable_after)
    }

    #[allow(clippy::too_many_arguments)]
    fn capture_v2_pointer(
        &mut self,
        event: RawInputEvent,
        x: i32,
        y: i32,
        event_kind: PointerEventKind,
        elapsed_ms: u64,
        surface: CaptureSurface,
        move_sample_interval_ms: u64,
    ) -> Result<(), String> {
        if let CaptureSurface::Unreadable(reason) = surface {
            let entering_unreadable = self.pointer_unreadable_reason.is_none();
            self.pointer_unreadable_reason = Some(reason);
            if entering_unreadable {
                self.last_mouse_move_elapsed_ms = None;
                self.last_mouse_move_position = None;
                self.pointer_gesture_anchor = None;
                self.release_recorded_mouse_buttons(elapsed_ms);
            }
            if let RawInputEvent::MouseButton { button, state, .. } = event {
                update_button_state(&mut self.physically_pressed_mouse_buttons, button, state);
            }
            return Ok(());
        }

        self.detect_pointer_adjustment(&surface);
        let mut position = self.resolve_pointer_position(x, y, event_kind, &surface)?;
        if self.pointer_gesture_anchor.is_none()
            && self.physically_pressed_mouse_buttons.is_empty()
            && matches!(
                event,
                RawInputEvent::MouseButton {
                    state: ButtonState::Pressed,
                    ..
                }
            )
        {
            if let (
                CaptureSurface::Window { window, .. },
                PointerPosition::WindowRelative {
                    target_id, x, y, ..
                },
            ) = (&surface, position)
            {
                if window.direct_pointer_target {
                    let adjusting_window = is_non_client_position(x, y, window.client_size);
                    self.pointer_gesture_anchor = Some(PointerGestureAnchor {
                        instance: window.instance,
                        target_id,
                        client_origin_x: window.client_origin_x,
                        client_origin_y: window.client_origin_y,
                        step_start_index: self.steps.len(),
                        adjusting_window,
                    });
                    if adjusting_window {
                        position = PointerPosition::WindowRelative {
                            target_id,
                            x,
                            y,
                            intent: WindowPointerIntent::WindowAdjustment,
                        };
                    }
                }
            }
        }
        let resumed_from_unreadable = self.pointer_unreadable_reason.take().is_some();
        if resumed_from_unreadable {
            self.last_mouse_move_elapsed_ms = None;
            self.last_mouse_move_position = None;
            self.resume_physically_pressed_mouse_buttons(elapsed_ms, position);
        }
        self.last_readable_pointer = Some(position);
        if self.step_limit_exceeded {
            return Ok(());
        }

        match event {
            RawInputEvent::MouseMove { .. } => {
                let coordinate_space_changed = self
                    .last_mouse_move_position
                    .map(|last| !same_pointer_space(last, position))
                    .unwrap_or(false);
                let should_sample = resumed_from_unreadable
                    || coordinate_space_changed
                    || !self.physically_pressed_mouse_buttons.is_empty()
                    || self
                        .last_mouse_move_elapsed_ms
                        .map(|last| elapsed_ms.saturating_sub(last) >= move_sample_interval_ms)
                        .unwrap_or(true);
                if should_sample
                    && self.push_step(MacroStep::PointerMove {
                        elapsed_ms,
                        position,
                    })
                {
                    self.last_mouse_move_elapsed_ms = Some(elapsed_ms);
                    self.last_mouse_move_position = Some(position);
                }
            }
            RawInputEvent::MouseButton { button, state, .. } => {
                update_button_state(&mut self.physically_pressed_mouse_buttons, button, state);
                if self.push_step(MacroStep::PointerButton {
                    elapsed_ms,
                    position,
                    button,
                    state,
                }) {
                    update_button_state(&mut self.recorded_pressed_mouse_buttons, button, state);
                }
                if self.physically_pressed_mouse_buttons.is_empty() {
                    self.pointer_gesture_anchor = None;
                }
            }
            RawInputEvent::MouseWheel { delta, .. } => {
                self.push_step(MacroStep::PointerWheel {
                    elapsed_ms,
                    position,
                    delta,
                });
            }
            RawInputEvent::Key { .. } => unreachable!("pointer capture received a key event"),
        }
        Ok(())
    }

    fn capture_v2_key(
        &mut self,
        event: RawInputEvent,
        elapsed_ms: u64,
        surface: CaptureSurface,
    ) -> Result<(), String> {
        let RawInputEvent::Key {
            vk_code,
            scan_code,
            extended,
            state,
            ..
        } = event
        else {
            unreachable!("key capture received a pointer event");
        };

        if self.keyboard_sequence.is_none()
            && self.physically_pressed_keys.is_empty()
            && state == KeyState::Released
        {
            return Ok(());
        }

        if self.keyboard_sequence.is_none() && self.physically_pressed_keys.is_empty() {
            self.keyboard_sequence = Some(match surface {
                CaptureSurface::Screen => KeyboardSequence::Recording(None),
                CaptureSurface::Window { window, .. } => {
                    KeyboardSequence::Recording(Some(self.resolve_window_target(&window)?))
                }
                CaptureSurface::Unreadable(_) => KeyboardSequence::Skipping,
            });
        }

        if let Some(KeyboardSequence::Recording(target_id)) = self.keyboard_sequence {
            self.push_step(MacroStep::TargetedKey {
                elapsed_ms,
                target_id,
                vk_code,
                scan_code,
                extended,
                state,
            });
        }

        let key = PhysicalKey {
            vk_code,
            scan_code,
            extended,
        };
        match state {
            KeyState::Pressed => {
                if !self.physically_pressed_keys.contains(&key) {
                    self.physically_pressed_keys.push(key);
                }
            }
            KeyState::Released => self
                .physically_pressed_keys
                .retain(|pressed| *pressed != key),
        }
        if self.physically_pressed_keys.is_empty() {
            self.keyboard_sequence = None;
        }
        Ok(())
    }

    fn resolve_pointer_position(
        &mut self,
        x: i32,
        y: i32,
        event_kind: PointerEventKind,
        surface: &CaptureSurface,
    ) -> Result<PointerPosition, String> {
        if let Some(anchor) = self
            .pointer_gesture_anchor
            .filter(|anchor| anchor.adjusting_window)
        {
            if let CaptureSurface::Window { window, .. } = surface {
                if window.instance == anchor.instance {
                    let target_id = self.resolve_window_target(window)?;
                    debug_assert_eq!(target_id, anchor.target_id);
                }
            }
            return Ok(PointerPosition::WindowRelative {
                target_id: anchor.target_id,
                x: checked_relative_coordinate(x, anchor.client_origin_x, "x")?,
                y: checked_relative_coordinate(y, anchor.client_origin_y, "y")?,
                intent: WindowPointerIntent::WindowAdjustment,
            });
        }
        match surface {
            CaptureSurface::Screen => Ok(PointerPosition::ScreenRelative { x, y }),
            CaptureSurface::Window { window, intent } => {
                validate_pointer_intent(event_kind, *intent)?;
                let relative_x = checked_relative_coordinate(x, window.client_origin_x, "x")?;
                let relative_y = checked_relative_coordinate(y, window.client_origin_y, "y")?;
                let target_id = self.resolve_window_target(window)?;
                Ok(PointerPosition::WindowRelative {
                    target_id,
                    x: relative_x,
                    y: relative_y,
                    intent: *intent,
                })
            }
            CaptureSurface::Unreadable(_) => {
                unreachable!("unreadable pointer surfaces are handled before position resolution")
            }
        }
    }

    fn detect_pointer_adjustment(&mut self, surface: &CaptureSurface) {
        let Some(anchor) = self.pointer_gesture_anchor else {
            return;
        };
        if anchor.adjusting_window {
            return;
        }
        let CaptureSurface::Window { window, .. } = surface else {
            return;
        };
        if window.instance != anchor.instance || window.dpi == 0 {
            return;
        }
        let recorded_size = self
            .target_geometry_by_instance
            .get(&window.instance)
            .copied()
            .unwrap_or(window.client_size);
        let geometry_changed = window.client_origin_x != anchor.client_origin_x
            || window.client_origin_y != anchor.client_origin_y
            || window.client_size != recorded_size;
        if !geometry_changed {
            return;
        }

        if let Some(active) = self.pointer_gesture_anchor.as_mut() {
            active.adjusting_window = true;
        }
        for step in &mut self.steps[anchor.step_start_index..] {
            let position = match step {
                MacroStep::PointerMove { position, .. }
                | MacroStep::PointerButton { position, .. } => position,
                _ => continue,
            };
            if let PointerPosition::WindowRelative {
                target_id, intent, ..
            } = position
            {
                if *target_id == anchor.target_id {
                    *intent = WindowPointerIntent::WindowAdjustment;
                }
            }
        }
    }

    fn resolve_window_target(
        &mut self,
        captured: &CapturedWindow,
    ) -> Result<TargetWindowId, String> {
        if captured.executable_path.trim().is_empty() {
            return Err("无法读取目标窗口的程序路径。".to_string());
        }
        if captured.window_class.trim().is_empty() {
            return Err("无法读取目标窗口的窗口类。".to_string());
        }
        if captured.client_size.width == 0 || captured.client_size.height == 0 {
            return Err("目标窗口的客户区尺寸无效。".to_string());
        }
        if captured.dpi == 0 {
            return Err("无法读取目标窗口的 DPI。".to_string());
        }

        if let Some(index) = self.target_by_instance.get(&captured.instance).copied() {
            let target = &self.targets[index];
            if target.executable_path != captured.executable_path {
                return Err(format!(
                    "目标窗口“{}”的程序路径在录制期间发生了变化。",
                    target.title
                ));
            }
            if target.window_class != captured.window_class {
                return Err(format!(
                    "目标窗口“{}”的窗口类在录制期间发生了变化。",
                    target.title
                ));
            }
            let expected_size = self
                .target_geometry_by_instance
                .get(&captured.instance)
                .copied()
                .unwrap_or(target.client_size);
            let adjusting_this_window = self.pointer_gesture_anchor.is_some_and(|anchor| {
                anchor.instance == captured.instance && anchor.adjusting_window
            });
            if target.dpi != captured.dpi {
                return Err(format!(
                    "目标窗口“{}”的 DPI 从 {} 变为 {}，无法安全继续窗口相对录制。",
                    target.title, target.dpi, captured.dpi
                ));
            }
            if expected_size != captured.client_size || adjusting_this_window {
                self.target_geometry_by_instance
                    .insert(captured.instance, captured.client_size);
            }
            return Ok(target.id);
        }

        let ordinal = self
            .targets
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| "录制包含的目标窗口数量过多。".to_string())?;
        let id = TargetWindowId(ordinal);
        let index = self.targets.len();
        self.targets.push(WindowTarget {
            id,
            executable_path: captured.executable_path.clone(),
            window_class: captured.window_class.clone(),
            title: captured.title.clone(),
            client_size: captured.client_size,
            dpi: captured.dpi,
            availability: captured.availability,
        });
        self.target_by_instance.insert(captured.instance, index);
        self.target_geometry_by_instance
            .insert(captured.instance, captured.client_size);
        Ok(id)
    }

    fn release_recorded_mouse_buttons(&mut self, elapsed_ms: u64) {
        let Some(position) = self.last_readable_pointer.map(synthetic_button_position) else {
            self.recorded_pressed_mouse_buttons.clear();
            return;
        };
        let buttons = std::mem::take(&mut self.recorded_pressed_mouse_buttons);
        for button in buttons {
            if !self.push_step(MacroStep::PointerButton {
                elapsed_ms,
                position,
                button,
                state: ButtonState::Released,
            }) {
                break;
            }
        }
    }

    fn resume_physically_pressed_mouse_buttons(
        &mut self,
        elapsed_ms: u64,
        position: PointerPosition,
    ) {
        let position = synthetic_button_position(position);
        for button in self.physically_pressed_mouse_buttons.clone() {
            if self.recorded_pressed_mouse_buttons.contains(&button) {
                continue;
            }
            if !self.push_step(MacroStep::PointerButton {
                elapsed_ms,
                position,
                button,
                state: ButtonState::Pressed,
            }) {
                break;
            }
            self.recorded_pressed_mouse_buttons.push(button);
        }
    }
}

fn validate_pointer_intent(
    event_kind: PointerEventKind,
    intent: WindowPointerIntent,
) -> Result<(), String> {
    let valid = match event_kind {
        PointerEventKind::Move => matches!(
            intent,
            WindowPointerIntent::Foreground | WindowPointerIntent::WindowAdjustment
        ),
        PointerEventKind::Button => intent != WindowPointerIntent::BackgroundWheel,
        PointerEventKind::Wheel => matches!(
            intent,
            WindowPointerIntent::Foreground | WindowPointerIntent::BackgroundWheel
        ),
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "鼠标输入类型 {event_kind:?} 与窗口操作意图 {intent:?} 不兼容。"
        ))
    }
}

fn is_non_client_position(x: i32, y: i32, client_size: ClientSize) -> bool {
    x < 0
        || y < 0
        || u32::try_from(x).map_or(true, |x| x >= client_size.width)
        || u32::try_from(y).map_or(true, |y| y >= client_size.height)
}

fn checked_relative_coordinate(
    screen_coordinate: i32,
    client_origin: i32,
    axis: &str,
) -> Result<i32, String> {
    let relative = i64::from(screen_coordinate) - i64::from(client_origin);
    i32::try_from(relative).map_err(|_| format!("窗口相对 {axis} 坐标 {relative} 超出支持范围。"))
}

fn update_button_state(
    pressed_buttons: &mut Vec<MouseButton>,
    button: MouseButton,
    state: ButtonState,
) {
    match state {
        ButtonState::Pressed => {
            if !pressed_buttons.contains(&button) {
                pressed_buttons.push(button);
            }
        }
        ButtonState::Released => pressed_buttons.retain(|pressed| *pressed != button),
    }
}

fn same_pointer_space(left: PointerPosition, right: PointerPosition) -> bool {
    match (left, right) {
        (PointerPosition::ScreenRelative { .. }, PointerPosition::ScreenRelative { .. }) => true,
        (
            PointerPosition::WindowRelative {
                target_id: left, ..
            },
            PointerPosition::WindowRelative {
                target_id: right, ..
            },
        ) => left == right,
        _ => false,
    }
}

fn synthetic_button_position(position: PointerPosition) -> PointerPosition {
    match position {
        PointerPosition::ScreenRelative { .. } => position,
        PointerPosition::WindowRelative {
            target_id, x, y, ..
        } => PointerPosition::WindowRelative {
            target_id,
            x,
            y,
            intent: WindowPointerIntent::Foreground,
        },
    }
}

fn unreadable_transition(before: Option<String>, after: Option<String>) -> CaptureOutcome {
    match (before, after) {
        (None, Some(reason)) => CaptureOutcome::UnreadableStarted(reason),
        (Some(_), None) => CaptureOutcome::UnreadableEnded,
        (Some(before), Some(after)) if before != after => CaptureOutcome::UnreadableStarted(after),
        _ => CaptureOutcome::Continue,
    }
}
