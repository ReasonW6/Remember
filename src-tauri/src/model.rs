use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const RECORDING_VERSION_V1: u32 = 1;
pub const RECORDING_VERSION_V2: u32 = 2;
// Keep the existing constant as the screen-only version so callers that have not opted in
// continue to create the exact legacy format.
pub const RECORDING_VERSION: u32 = RECORDING_VERSION_V1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct TargetWindowId(pub u32);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetWindowAvailability {
    Initial,
    Deferred,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowTarget {
    pub id: TargetWindowId,
    pub executable_path: String,
    pub window_class: String,
    pub title: String,
    pub client_size: ClientSize,
    pub dpi: u32,
    pub availability: TargetWindowAvailability,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowPointerIntent {
    Foreground,
    ActivationClick,
    DropRelease,
    BackgroundWheel,
    WindowAdjustment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "coordinate_space", rename_all = "snake_case")]
pub enum PointerPosition {
    ScreenRelative {
        x: i32,
        y: i32,
    },
    WindowRelative {
        target_id: TargetWindowId,
        x: i32,
        y: i32,
        intent: WindowPointerIntent,
    },
}

impl PointerPosition {
    pub fn coordinates(&self) -> (i32, i32) {
        match *self {
            Self::ScreenRelative { x, y } | Self::WindowRelative { x, y, .. } => (x, y),
        }
    }

    pub fn target_id(&self) -> Option<TargetWindowId> {
        match *self {
            Self::ScreenRelative { .. } => None,
            Self::WindowRelative { target_id, .. } => Some(target_id),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ButtonState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MacroStep {
    MouseMove {
        elapsed_ms: u64,
        x: i32,
        y: i32,
    },
    MouseButton {
        elapsed_ms: u64,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    },
    MouseWheel {
        elapsed_ms: u64,
        x: i32,
        y: i32,
        delta: i32,
    },
    Key {
        elapsed_ms: u64,
        vk_code: u16,
        scan_code: u16,
        #[serde(default)]
        extended: bool,
        state: KeyState,
    },
    PointerMove {
        elapsed_ms: u64,
        position: PointerPosition,
    },
    PointerButton {
        elapsed_ms: u64,
        position: PointerPosition,
        button: MouseButton,
        state: ButtonState,
    },
    PointerWheel {
        elapsed_ms: u64,
        position: PointerPosition,
        delta: i32,
    },
    TargetedKey {
        elapsed_ms: u64,
        target_id: Option<TargetWindowId>,
        vk_code: u16,
        scan_code: u16,
        #[serde(default)]
        extended: bool,
        state: KeyState,
    },
    Wait {
        elapsed_ms: u64,
    },
}

impl MacroStep {
    pub fn elapsed_ms(&self) -> u64 {
        match self {
            MacroStep::MouseMove { elapsed_ms, .. }
            | MacroStep::MouseButton { elapsed_ms, .. }
            | MacroStep::MouseWheel { elapsed_ms, .. }
            | MacroStep::Key { elapsed_ms, .. }
            | MacroStep::PointerMove { elapsed_ms, .. }
            | MacroStep::PointerButton { elapsed_ms, .. }
            | MacroStep::PointerWheel { elapsed_ms, .. }
            | MacroStep::TargetedKey { elapsed_ms, .. }
            | MacroStep::Wait { elapsed_ms } => *elapsed_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recording {
    pub version: u32,
    pub name: String,
    pub created_at: String,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<WindowTarget>,
    pub steps: Vec<MacroStep>,
}

impl Recording {
    pub fn new(
        name: impl Into<String>,
        created_at: impl Into<String>,
        steps: Vec<MacroStep>,
    ) -> Self {
        let duration_ms = steps.last().map(MacroStep::elapsed_ms).unwrap_or(0);
        Self {
            version: RECORDING_VERSION,
            name: name.into(),
            created_at: created_at.into(),
            duration_ms,
            targets: Vec::new(),
            steps,
        }
    }

    pub fn new_window_relative(
        name: impl Into<String>,
        created_at: impl Into<String>,
        targets: Vec<WindowTarget>,
        steps: Vec<MacroStep>,
    ) -> Self {
        let duration_ms = steps.last().map(MacroStep::elapsed_ms).unwrap_or(0);
        Self {
            version: RECORDING_VERSION_V2,
            name: name.into(),
            created_at: created_at.into(),
            duration_ms,
            targets,
            steps,
        }
    }

    pub fn is_window_relative(&self) -> bool {
        self.version == RECORDING_VERSION_V2
    }

    pub fn validate(&self) -> Result<(), String> {
        match self.version {
            RECORDING_VERSION_V1 => self.validate_v1_contents()?,
            RECORDING_VERSION_V2 => self.validate_v2_contents()?,
            version => return Err(format!("unsupported recording version {version}")),
        }
        if self.name.trim().is_empty() {
            return Err("recording name cannot be empty".to_string());
        }
        if self.created_at.trim().is_empty() {
            return Err("created_at cannot be empty".to_string());
        }
        if self
            .steps
            .windows(2)
            .any(|pair| pair[1].elapsed_ms() < pair[0].elapsed_ms())
        {
            return Err("step timestamps must be monotonic".to_string());
        }
        if self.duration_ms < self.steps.last().map(MacroStep::elapsed_ms).unwrap_or(0) {
            return Err("duration_ms cannot be shorter than the final step".to_string());
        }
        Ok(())
    }

    fn validate_v1_contents(&self) -> Result<(), String> {
        if !self.targets.is_empty() {
            return Err("version 1 recordings cannot contain window targets".to_string());
        }
        if self.steps.iter().any(MacroStep::is_v2_only) {
            return Err("version 1 recordings cannot contain version 2 steps".to_string());
        }
        Ok(())
    }

    fn validate_v2_contents(&self) -> Result<(), String> {
        let mut target_ids = HashSet::with_capacity(self.targets.len());
        for target in &self.targets {
            if !target_ids.insert(target.id) {
                return Err(format!("duplicate target window id {}", target.id.0));
            }
            if target.executable_path.trim().is_empty() {
                return Err(format!(
                    "target window {} executable_path cannot be empty",
                    target.id.0
                ));
            }
            if target.window_class.trim().is_empty() {
                return Err(format!(
                    "target window {} window_class cannot be empty",
                    target.id.0
                ));
            }
            if target.dpi == 0 {
                return Err(format!(
                    "target window {} dpi must be greater than zero",
                    target.id.0
                ));
            }
            if target.client_size.width == 0 || target.client_size.height == 0 {
                return Err(format!(
                    "target window {} client size must be greater than zero",
                    target.id.0
                ));
            }
        }

        let mut pressed_keys = HashSet::new();
        let mut keyboard_sequence_target = None;
        for step in &self.steps {
            match step {
                MacroStep::MouseMove { .. }
                | MacroStep::MouseButton { .. }
                | MacroStep::MouseWheel { .. }
                | MacroStep::Key { .. } => {
                    return Err(
                        "version 2 recordings must use explicit version 2 input steps".to_string(),
                    );
                }
                MacroStep::PointerMove { position, .. }
                | MacroStep::PointerButton { position, .. }
                | MacroStep::PointerWheel { position, .. } => {
                    if let Some(target_id) = position.target_id() {
                        validate_target_reference(target_id, &target_ids)?;
                    }
                }
                MacroStep::TargetedKey {
                    target_id,
                    vk_code,
                    scan_code,
                    extended,
                    state,
                    ..
                } => {
                    if let Some(target_id) = target_id {
                        validate_target_reference(*target_id, &target_ids)?;
                    }
                    validate_targeted_key_sequence(
                        *target_id,
                        (*vk_code, *scan_code, *extended),
                        *state,
                        &mut pressed_keys,
                        &mut keyboard_sequence_target,
                    )?;
                }
                MacroStep::Wait { .. } => {}
            }
            validate_pointer_intent(step)?;
        }
        Ok(())
    }
}

fn validate_pointer_intent(step: &MacroStep) -> Result<(), String> {
    match step {
        MacroStep::PointerMove {
            position: PointerPosition::WindowRelative { intent, .. },
            ..
        } => {
            if !matches!(
                intent,
                WindowPointerIntent::Foreground | WindowPointerIntent::WindowAdjustment
            ) {
                return Err(
                    "window-relative pointer movement must use foreground or window-adjustment intent"
                        .to_string(),
                );
            }
            Ok(())
        }
        MacroStep::PointerButton {
            position: PointerPosition::WindowRelative { intent, .. },
            state,
            ..
        } => match (*intent, *state) {
            (WindowPointerIntent::Foreground | WindowPointerIntent::WindowAdjustment, _) => Ok(()),
            (WindowPointerIntent::ActivationClick, ButtonState::Pressed) => Ok(()),
            (WindowPointerIntent::DropRelease, ButtonState::Released) => Ok(()),
            (WindowPointerIntent::ActivationClick, ButtonState::Released) => {
                Err("window-relative activation-click intent requires a pressed button".to_string())
            }
            (WindowPointerIntent::DropRelease, ButtonState::Pressed) => {
                Err("window-relative drop-release intent requires a released button".to_string())
            }
            (WindowPointerIntent::BackgroundWheel, _) => {
                Err("window-relative pointer button cannot use background-wheel intent".to_string())
            }
        },
        MacroStep::PointerWheel {
            position: PointerPosition::WindowRelative { intent, .. },
            ..
        } => {
            if matches!(
                intent,
                WindowPointerIntent::Foreground | WindowPointerIntent::BackgroundWheel
            ) {
                return Ok(());
            }
            Err(
                "window-relative pointer wheel must use foreground or background-wheel intent"
                    .to_string(),
            )
        }
        _ => Ok(()),
    }
}

fn validate_targeted_key_sequence(
    target_id: Option<TargetWindowId>,
    physical_key: (u16, u16, bool),
    state: KeyState,
    pressed_keys: &mut HashSet<(u16, u16, bool)>,
    sequence_target: &mut Option<Option<TargetWindowId>>,
) -> Result<(), String> {
    if pressed_keys.is_empty() {
        if state == KeyState::Released {
            return Err("targeted keyboard sequence must start with a pressed key".to_string());
        }
        *sequence_target = Some(target_id);
    } else if *sequence_target != Some(target_id) {
        return Err(
            "targeted keyboard sequence must keep the same target until all keys are released"
                .to_string(),
        );
    }

    match state {
        // Match player::PressedInputs: key-repeat presses do not add a second held instance.
        KeyState::Pressed => {
            pressed_keys.insert(physical_key);
        }
        // Likewise, an unmatched release inside an active sequence is a no-op.
        KeyState::Released => {
            pressed_keys.remove(&physical_key);
        }
    }
    if pressed_keys.is_empty() {
        *sequence_target = None;
    }
    Ok(())
}

impl MacroStep {
    fn is_v2_only(&self) -> bool {
        matches!(
            self,
            Self::PointerMove { .. }
                | Self::PointerButton { .. }
                | Self::PointerWheel { .. }
                | Self::TargetedKey { .. }
        )
    }
}

fn validate_target_reference(
    target_id: TargetWindowId,
    target_ids: &HashSet<TargetWindowId>,
) -> Result<(), String> {
    if target_ids.contains(&target_id) {
        Ok(())
    } else {
        Err(format!(
            "step references unknown target window id {}",
            target_id.0
        ))
    }
}
