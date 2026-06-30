use serde::{Deserialize, Serialize};

pub const RECORDING_VERSION: u32 = 1;

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
            steps,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != RECORDING_VERSION {
            return Err(format!("unsupported recording version {}", self.version));
        }
        if self.name.trim().is_empty() {
            return Err("recording name cannot be empty".to_string());
        }
        if self.created_at.trim().is_empty() {
            return Err("created_at cannot be empty".to_string());
        }
        if self.duration_ms < self.steps.last().map(MacroStep::elapsed_ms).unwrap_or(0) {
            return Err("duration_ms cannot be shorter than the final step".to_string());
        }
        Ok(())
    }
}
