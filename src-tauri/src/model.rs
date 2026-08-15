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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PointerSemanticAction {
    SelectComboOption {
        control_id: i32,
        control_bounds: ControlBounds,
        option_name: String,
    },
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        semantic_action: Option<PointerSemanticAction>,
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
            version => return Err(format!("不支持录制文件版本 {version}。")),
        }
        if self.name.trim().is_empty() {
            return Err("录制名称不能为空。".to_string());
        }
        if self.created_at.trim().is_empty() {
            return Err("录制创建时间不能为空。".to_string());
        }
        if self
            .steps
            .windows(2)
            .any(|pair| pair[1].elapsed_ms() < pair[0].elapsed_ms())
        {
            return Err("步骤时间戳不得倒退。".to_string());
        }
        if self.duration_ms < self.steps.last().map(MacroStep::elapsed_ms).unwrap_or(0) {
            return Err("录制总时长不能短于最后一个步骤的时间。".to_string());
        }
        Ok(())
    }

    fn validate_v1_contents(&self) -> Result<(), String> {
        if !self.targets.is_empty() {
            return Err("版本 1 录制文件不能包含目标窗口。".to_string());
        }
        if self.steps.iter().any(MacroStep::is_v2_only) {
            return Err("版本 1 录制文件不能包含版本 2 步骤。".to_string());
        }
        Ok(())
    }

    fn validate_v2_contents(&self) -> Result<(), String> {
        let mut target_ids = HashSet::with_capacity(self.targets.len());
        for target in &self.targets {
            if !target_ids.insert(target.id) {
                return Err(format!("目标窗口 ID {} 重复。", target.id.0));
            }
            if target.executable_path.trim().is_empty() {
                return Err(format!(
                    "目标窗口 {} 的可执行文件路径不能为空。",
                    target.id.0
                ));
            }
            if target.window_class.trim().is_empty() {
                return Err(format!("目标窗口 {} 的窗口类名不能为空。", target.id.0));
            }
            if target.dpi == 0 {
                return Err(format!("目标窗口 {} 的 DPI 必须大于零。", target.id.0));
            }
            if target.client_size.width == 0 || target.client_size.height == 0 {
                return Err(format!(
                    "目标窗口 {} 的客户区宽度和高度必须大于零。",
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
                    return Err("版本 2 录制文件必须使用明确的版本 2 输入步骤。".to_string());
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
            validate_pointer_semantic_action(step)?;
        }
        Ok(())
    }
}

fn validate_pointer_semantic_action(step: &MacroStep) -> Result<(), String> {
    let MacroStep::PointerButton {
        position,
        button,
        semantic_action: Some(action),
        ..
    } = step
    else {
        return Ok(());
    };
    if !matches!(position, PointerPosition::WindowRelative { .. }) {
        return Err("语义鼠标操作必须引用目标窗口。".to_string());
    }
    if *button != MouseButton::Left {
        return Err("下拉选项语义操作只能使用鼠标左键。".to_string());
    }
    match action {
        PointerSemanticAction::SelectComboOption {
            control_bounds,
            option_name,
            ..
        } => {
            if option_name.trim().is_empty() {
                return Err("下拉选项名称不能为空。".to_string());
            }
            if control_bounds.width == 0 || control_bounds.height == 0 {
                return Err("下拉控件的录制尺寸必须大于零。".to_string());
            }
        }
    }
    Ok(())
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
                return Err("窗口相对鼠标移动必须使用前台或窗口调整意图。".to_string());
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
                Err("窗口相对激活点击意图只能用于鼠标按钮按下。".to_string())
            }
            (WindowPointerIntent::DropRelease, ButtonState::Pressed) => {
                Err("窗口相对拖放释放意图只能用于鼠标按钮松开。".to_string())
            }
            (WindowPointerIntent::BackgroundWheel, _) => {
                Err("窗口相对鼠标按钮不能使用后台滚轮意图。".to_string())
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
            Err("窗口相对鼠标滚轮必须使用前台或后台滚轮意图。".to_string())
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
            return Err("目标键盘序列必须从按键按下开始。".to_string());
        }
        *sequence_target = Some(target_id);
    } else if *sequence_target != Some(target_id) {
        return Err("目标键盘序列在所有按键松开前必须保持同一目标窗口。".to_string());
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
        Err(format!("步骤引用了未知的目标窗口 ID {}。", target_id.0))
    }
}
