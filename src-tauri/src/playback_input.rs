use crate::{storage::TargetWindow, windows};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackMouseButton {
    Left,
    Right,
}

pub trait PlaybackInput: Send + Sync + 'static {
    fn active_window_target(&self) -> TargetWindow;

    fn click(&self, button: PlaybackMouseButton, x: i32, y: i32) -> Result<(), String>;

    fn type_text(&self, text: &str) -> Result<(), String>;

    fn press_hotkey(&self, keys: &[String]) -> Result<(), String>;

    fn press_key(&self, key: &str) -> Result<(), String> {
        self.press_hotkey(&[key.to_string()])
    }

    fn drag(
        &self,
        button: PlaybackMouseButton,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        duration_ms: u64,
    ) -> Result<(), String> {
        let _ = (button, start_x, start_y, end_x, end_y, duration_ms);
        Err("drag playback is unavailable for this input backend".to_string())
    }

    fn scroll(&self, delta_x: i32, delta_y: i32) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct SystemPlaybackInput;

impl PlaybackInput for SystemPlaybackInput {
    fn active_window_target(&self) -> TargetWindow {
        windows::active_window_target()
    }

    fn click(&self, button: PlaybackMouseButton, x: i32, y: i32) -> Result<(), String> {
        platform::click(button, x, y)
    }

    fn type_text(&self, text: &str) -> Result<(), String> {
        platform::type_text(text)
    }

    fn press_hotkey(&self, keys: &[String]) -> Result<(), String> {
        platform::press_hotkey(keys)
    }

    fn press_key(&self, key: &str) -> Result<(), String> {
        platform::press_key(key)
    }

    fn drag(
        &self,
        button: PlaybackMouseButton,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        duration_ms: u64,
    ) -> Result<(), String> {
        platform::drag(button, start_x, start_y, end_x, end_y, duration_ms)
    }

    fn scroll(&self, delta_x: i32, delta_y: i32) -> Result<(), String> {
        platform::scroll(delta_x, delta_y)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::PlaybackMouseButton;
    use std::mem::size_of;
    use std::{thread, time::Duration};

    const INPUT_KEYBOARD: u32 = 1;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const KEYEVENTF_UNICODE: u32 = 0x0004;
    const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
    const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
    const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
    const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
    const MOUSEEVENTF_WHEEL: u32 = 0x0800;
    const MOUSEEVENTF_HWHEEL: u32 = 0x01000;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct KeyboardInput {
        w_vk: u16,
        w_scan: u16,
        dw_flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MouseInput {
        dx: i32,
        dy: i32,
        mouse_data: u32,
        dw_flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    union InputData {
        ki: KeyboardInput,
        mi: MouseInput,
    }

    #[repr(C)]
    struct Input {
        input_type: u32,
        data: InputData,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetCursorPos(x: i32, y: i32) -> i32;
        fn mouse_event(dw_flags: u32, dx: u32, dy: u32, dw_data: u32, dw_extra_info: usize);
        fn SendInput(input_count: u32, inputs: *const Input, input_size: i32) -> u32;
    }

    pub fn click(button: PlaybackMouseButton, x: i32, y: i32) -> Result<(), String> {
        let positioned = unsafe { SetCursorPos(x, y) };
        if positioned == 0 {
            return Err(format!("failed to move cursor to ({x}, {y})"));
        }

        let (down, up) = match button {
            PlaybackMouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            PlaybackMouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        };

        unsafe {
            mouse_event(down, 0, 0, 0, 0);
            mouse_event(up, 0, 0, 0, 0);
        }

        Ok(())
    }

    pub fn drag(
        button: PlaybackMouseButton,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        duration_ms: u64,
    ) -> Result<(), String> {
        let positioned = unsafe { SetCursorPos(start_x, start_y) };
        if positioned == 0 {
            return Err(format!("failed to move cursor to ({start_x}, {start_y})"));
        }

        let (down, up) = match button {
            PlaybackMouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            PlaybackMouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        };

        unsafe {
            mouse_event(down, 0, 0, 0, 0);
        }

        let step_count = 12u64;
        let sleep_ms = duration_ms / step_count;
        for index in 1..=step_count {
            let progress = index as f64 / step_count as f64;
            let x = start_x + ((end_x - start_x) as f64 * progress).round() as i32;
            let y = start_y + ((end_y - start_y) as f64 * progress).round() as i32;
            unsafe {
                SetCursorPos(x, y);
            }
            if sleep_ms > 0 {
                thread::sleep(Duration::from_millis(sleep_ms));
            }
        }

        unsafe {
            mouse_event(up, 0, 0, 0, 0);
        }

        Ok(())
    }

    pub fn type_text(text: &str) -> Result<(), String> {
        let mut inputs = Vec::new();
        for code_unit in text.encode_utf16() {
            inputs.push(keyboard_input(code_unit, KEYEVENTF_UNICODE));
            inputs.push(keyboard_input(
                code_unit,
                KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            ));
        }

        if inputs.is_empty() {
            return Ok(());
        }

        send_keyboard_inputs(&inputs, "text input")
    }

    pub fn press_hotkey(keys: &[String]) -> Result<(), String> {
        let virtual_keys = keys
            .iter()
            .map(|key| hotkey_virtual_key(key))
            .collect::<Result<Vec<_>, _>>()?;
        if virtual_keys.is_empty() {
            return Err("hotkey has no keys".to_string());
        }

        let mut inputs = Vec::with_capacity(virtual_keys.len() * 2);
        for virtual_key in &virtual_keys {
            inputs.push(virtual_key_input(*virtual_key, 0));
        }
        for virtual_key in virtual_keys.iter().rev() {
            inputs.push(virtual_key_input(*virtual_key, KEYEVENTF_KEYUP));
        }

        send_keyboard_inputs(&inputs, "hotkey")
    }

    pub fn press_key(key: &str) -> Result<(), String> {
        let virtual_key = hotkey_virtual_key(key)?;
        let inputs = [
            virtual_key_input(virtual_key, 0),
            virtual_key_input(virtual_key, KEYEVENTF_KEYUP),
        ];
        send_keyboard_inputs(&inputs, "key")
    }

    pub fn scroll(delta_x: i32, delta_y: i32) -> Result<(), String> {
        if delta_x == 0 && delta_y == 0 {
            return Ok(());
        }

        unsafe {
            if delta_y != 0 {
                mouse_event(MOUSEEVENTF_WHEEL, 0, 0, delta_y as u32, 0);
            }
            if delta_x != 0 {
                mouse_event(MOUSEEVENTF_HWHEEL, 0, 0, delta_x as u32, 0);
            }
        }

        Ok(())
    }

    fn send_keyboard_inputs(inputs: &[Input], label: &str) -> Result<(), String> {
        if inputs.is_empty() {
            return Ok(());
        }

        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<Input>() as i32,
            )
        };

        if sent != inputs.len() as u32 {
            return Err(format!(
                "failed to send {label}: sent {sent} of {} events",
                inputs.len()
            ));
        }

        Ok(())
    }

    fn hotkey_virtual_key(key: &str) -> Result<u16, String> {
        let normalized = key.trim().to_ascii_uppercase();
        match normalized.as_str() {
            "CTRL" | "CONTROL" => Ok(0x11),
            "ALT" => Ok(0x12),
            "SHIFT" => Ok(0x10),
            "WIN" | "WINDOWS" => Ok(0x5b),
            "BACKSPACE" => Ok(0x08),
            "TAB" => Ok(0x09),
            "ENTER" | "RETURN" => Ok(0x0d),
            "ESC" | "ESCAPE" => Ok(0x1b),
            "SPACE" => Ok(0x20),
            "LEFT" => Ok(0x25),
            "UP" => Ok(0x26),
            "RIGHT" => Ok(0x27),
            "DOWN" => Ok(0x28),
            "DELETE" | "DEL" => Ok(0x2e),
            "F1" => Ok(0x70),
            "F2" => Ok(0x71),
            "F3" => Ok(0x72),
            "F4" => Ok(0x73),
            "F5" => Ok(0x74),
            "F6" => Ok(0x75),
            "F7" => Ok(0x76),
            "F8" => Ok(0x77),
            "F9" => Ok(0x78),
            "F10" => Ok(0x79),
            "F11" => Ok(0x7a),
            "F12" => Ok(0x7b),
            _ => {
                if normalized.len() == 1 {
                    let character = normalized
                        .chars()
                        .next()
                        .expect("single-character key should exist");
                    if character.is_ascii_alphanumeric() {
                        return Ok(character as u16);
                    }
                }
                Err(format!("unsupported hotkey key: {key}"))
            }
        }
    }

    fn keyboard_input(code_unit: u16, flags: u32) -> Input {
        Input {
            input_type: INPUT_KEYBOARD,
            data: InputData {
                ki: KeyboardInput {
                    w_vk: 0,
                    w_scan: code_unit,
                    dw_flags: flags,
                    time: 0,
                    dw_extra_info: 0,
                },
            },
        }
    }

    fn virtual_key_input(virtual_key: u16, flags: u32) -> Input {
        Input {
            input_type: INPUT_KEYBOARD,
            data: InputData {
                ki: KeyboardInput {
                    w_vk: virtual_key,
                    w_scan: 0,
                    dw_flags: flags,
                    time: 0,
                    dw_extra_info: 0,
                },
            },
        }
    }

    #[cfg(test)]
    pub fn input_size_for_test() -> usize {
        size_of::<Input>()
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::PlaybackMouseButton;

    pub fn click(_button: PlaybackMouseButton, _x: i32, _y: i32) -> Result<(), String> {
        Err("mouse playback is only available on Windows".to_string())
    }

    pub fn type_text(_text: &str) -> Result<(), String> {
        Err("text playback is only available on Windows".to_string())
    }

    pub fn press_hotkey(_keys: &[String]) -> Result<(), String> {
        Err("hotkey playback is only available on Windows".to_string())
    }

    pub fn press_key(_key: &str) -> Result<(), String> {
        Err("key playback is only available on Windows".to_string())
    }

    pub fn drag(
        _button: PlaybackMouseButton,
        _start_x: i32,
        _start_y: i32,
        _end_x: i32,
        _end_y: i32,
        _duration_ms: u64,
    ) -> Result<(), String> {
        Err("drag playback is only available on Windows".to_string())
    }

    pub fn scroll(_delta_x: i32, _delta_y: i32) -> Result<(), String> {
        Err("scroll playback is only available on Windows".to_string())
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    #[test]
    fn send_input_uses_native_windows_input_struct_size() {
        assert_eq!(super::platform::input_size_for_test(), 40);
    }
}
