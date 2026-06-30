use crate::model::{ButtonState, KeyState, MacroStep, MouseButton, Recording, RECORDING_VERSION};

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
        state: KeyState,
    },
}

#[derive(Debug, Clone)]
pub struct Recorder {
    move_sample_interval_ms: u64,
    active: Option<ActiveRecording>,
}

impl Recorder {
    pub fn new(move_sample_interval_ms: u64) -> Self {
        Self {
            move_sample_interval_ms,
            active: None,
        }
    }

    pub fn start(
        &mut self,
        name: impl Into<String>,
        start_ms: u64,
        created_at: impl Into<String>,
    ) -> Result<(), String> {
        if self.active.is_some() {
            return Err("already recording".to_string());
        }

        self.active = Some(ActiveRecording {
            name: name.into(),
            created_at: created_at.into(),
            start_ms,
            steps: Vec::new(),
            last_mouse_move_elapsed_ms: None,
        });
        Ok(())
    }

    pub fn capture(&mut self, event: RawInputEvent) {
        let Some(active) = &mut self.active else {
            return;
        };

        match event {
            RawInputEvent::MouseMove { at_ms, x, y } => {
                active.capture_mouse_move(at_ms, x, y, self.move_sample_interval_ms);
            }
            RawInputEvent::MouseButton {
                at_ms,
                x,
                y,
                button,
                state,
            } => {
                let elapsed_ms = active.elapsed_ms(at_ms);
                if active.is_stale_elapsed(elapsed_ms) {
                    return;
                }
                active.steps.push(MacroStep::MouseButton {
                    elapsed_ms,
                    x,
                    y,
                    button,
                    state,
                });
            }
            RawInputEvent::MouseWheel { at_ms, x, y, delta } => {
                let elapsed_ms = active.elapsed_ms(at_ms);
                if active.is_stale_elapsed(elapsed_ms) {
                    return;
                }
                active.steps.push(MacroStep::MouseWheel {
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
                state,
            } => {
                let elapsed_ms = active.elapsed_ms(at_ms);
                if active.is_stale_elapsed(elapsed_ms) {
                    return;
                }
                active.steps.push(MacroStep::Key {
                    elapsed_ms,
                    vk_code,
                    scan_code,
                    state,
                });
            }
        }
    }

    pub fn stop(&mut self, stop_ms: u64) -> Result<Recording, String> {
        let Some(active) = self.active.take() else {
            return Err("not recording".to_string());
        };

        let duration_ms = stop_ms
            .saturating_sub(active.start_ms)
            .max(active.last_emitted_elapsed_ms().unwrap_or(0));

        Ok(Recording {
            version: RECORDING_VERSION,
            name: active.name,
            created_at: active.created_at,
            duration_ms,
            steps: active.steps,
        })
    }

    pub fn is_recording(&self) -> bool {
        self.active.is_some()
    }
}

#[derive(Debug, Clone)]
struct ActiveRecording {
    name: String,
    created_at: String,
    start_ms: u64,
    steps: Vec<MacroStep>,
    last_mouse_move_elapsed_ms: Option<u64>,
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

    fn capture_mouse_move(&mut self, at_ms: u64, x: i32, y: i32, move_sample_interval_ms: u64) {
        let elapsed_ms = self.elapsed_ms(at_ms);
        if self.is_stale_elapsed(elapsed_ms) {
            return;
        }

        let should_sample = self
            .last_mouse_move_elapsed_ms
            .map(|last| elapsed_ms.saturating_sub(last) >= move_sample_interval_ms)
            .unwrap_or(true);

        if should_sample {
            self.steps.push(MacroStep::MouseMove { elapsed_ms, x, y });
            self.last_mouse_move_elapsed_ms = Some(elapsed_ms);
        }
    }
}
