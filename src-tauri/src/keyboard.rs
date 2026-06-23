use crate::recorder::RecordedKeyboardInput;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
mod platform {
    use super::RecordedKeyboardInput;
    use crate::windows;
    use std::{
        ffi::c_void,
        mem,
        ptr::null_mut,
        sync::{Arc, Mutex},
        thread::{self, JoinHandle},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    type Handle = *mut c_void;
    type HookProc = Option<unsafe extern "system" fn(i32, usize, isize) -> isize>;

    const WH_KEYBOARD_LL: i32 = 13;
    const WM_KEYDOWN: usize = 0x0100;
    const WM_SYSKEYDOWN: usize = 0x0104;
    const WM_QUIT: u32 = 0x0012;

    const VK_BACK: u32 = 0x08;
    const VK_TAB: u32 = 0x09;
    const VK_RETURN: u32 = 0x0d;
    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const VK_ESCAPE: u32 = 0x1b;
    const VK_SPACE: u32 = 0x20;
    const VK_LEFT: u32 = 0x25;
    const VK_UP: u32 = 0x26;
    const VK_RIGHT: u32 = 0x27;
    const VK_DOWN: u32 = 0x28;
    const VK_DELETE: u32 = 0x2e;
    const VK_LWIN: i32 = 0x5b;
    const VK_RWIN: i32 = 0x5c;
    const VK_OEM_1: u32 = 0xba;
    const VK_OEM_PLUS: u32 = 0xbb;
    const VK_OEM_COMMA: u32 = 0xbc;
    const VK_OEM_MINUS: u32 = 0xbd;
    const VK_OEM_PERIOD: u32 = 0xbe;
    const VK_OEM_2: u32 = 0xbf;
    const VK_OEM_3: u32 = 0xc0;
    const VK_OEM_4: u32 = 0xdb;
    const VK_OEM_5: u32 = 0xdc;
    const VK_OEM_6: u32 = 0xdd;
    const VK_OEM_7: u32 = 0xde;

    static KEY_SINK: Mutex<Option<Arc<Mutex<Vec<RecordedKeyboardInput>>>>> = Mutex::new(None);

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: Handle,
        message: u32,
        w_param: usize,
        l_param: isize,
        time: u32,
        pt: Point,
        private: u32,
    }

    #[repr(C)]
    struct KeyboardHookStruct {
        vk_code: u32,
        scan_code: u32,
        flags: u32,
        time: u32,
        extra_info: usize,
    }

    #[link(name = "user32")]
    extern "system" {
        fn CallNextHookEx(hook: Handle, code: i32, w_param: usize, l_param: isize) -> isize;
        fn DispatchMessageW(message: *const Msg) -> isize;
        fn GetAsyncKeyState(virtual_key: i32) -> i16;
        fn GetMessageW(message: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
        fn PostThreadMessageW(thread_id: u32, message: u32, w_param: usize, l_param: isize) -> i32;
        fn SetWindowsHookExW(
            hook_id: i32,
            hook_proc: HookProc,
            instance: Handle,
            thread_id: u32,
        ) -> Handle;
        fn TranslateMessage(message: *const Msg) -> i32;
        fn UnhookWindowsHookEx(hook: Handle) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
        fn GetModuleHandleW(module_name: *const u16) -> Handle;
    }

    #[derive(Debug)]
    pub struct KeyboardCaptureGuard {
        thread_id: u32,
        join_handle: Option<JoinHandle<()>>,
    }

    impl KeyboardCaptureGuard {
        pub fn stop(mut self) {
            self.stop_inner();
        }

        fn stop_inner(&mut self) {
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
            if let Some(join_handle) = self.join_handle.take() {
                let _ = join_handle.join();
            }
        }
    }

    impl Drop for KeyboardCaptureGuard {
        fn drop(&mut self) {
            self.stop_inner();
        }
    }

    pub fn start_key_capture(
        key_sink: Arc<Mutex<Vec<RecordedKeyboardInput>>>,
    ) -> Result<KeyboardCaptureGuard, String> {
        let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
        let join_handle = thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            if let Ok(mut sink) = KEY_SINK.lock() {
                *sink = Some(key_sink);
            }

            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let hook =
                unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook_proc), instance, 0) };

            if hook.is_null() {
                clear_key_sink();
                let _ = ready_sender.send(Err("failed to install keyboard hook".to_string()));
                return;
            }

            let _ = ready_sender.send(Ok(thread_id));

            let mut message = unsafe { mem::zeroed::<Msg>() };
            loop {
                let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
                if result <= 0 {
                    break;
                }
                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }

            unsafe {
                UnhookWindowsHookEx(hook);
            }
            clear_key_sink();
        });

        match ready_receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(thread_id)) => Ok(KeyboardCaptureGuard {
                thread_id,
                join_handle: Some(join_handle),
            }),
            Ok(Err(error)) => {
                let _ = join_handle.join();
                Err(error)
            }
            Err(error) => {
                let _ = join_handle.join();
                Err(error.to_string())
            }
        }
    }

    unsafe extern "system" fn key_hook_proc(code: i32, w_param: usize, l_param: isize) -> isize {
        if code >= 0 && (w_param == WM_KEYDOWN || w_param == WM_SYSKEYDOWN) {
            let hook = &*(l_param as *const KeyboardHookStruct);
            if let Some(input) = recorded_input_from_key(hook.vk_code) {
                push_input(input);
            }
        }

        CallNextHookEx(null_mut(), code, w_param, l_param)
    }

    fn recorded_input_from_key(vk_code: u32) -> Option<RecordedKeyboardInput> {
        if is_modifier_key(vk_code) {
            return None;
        }

        let modifiers = active_modifiers();
        let captured_at_ms = unix_millis();
        if modifiers
            .iter()
            .any(|modifier| *modifier == "Ctrl" || *modifier == "Alt" || *modifier == "Win")
        {
            let mut keys = modifiers;
            let key_name = hotkey_key_name(vk_code)?;
            keys.push(key_name.to_string());
            return Some(RecordedKeyboardInput::Hotkey {
                keys,
                captured_at_ms,
                target_window: Some(windows::active_window_target()),
            });
        }

        printable_text(vk_code, key_is_pressed(VK_SHIFT))
            .map(|text| RecordedKeyboardInput::Text {
                text,
                captured_at_ms,
                target_window: Some(windows::active_window_target()),
            })
            .or_else(|| {
                plain_key_name(vk_code).map(|key| RecordedKeyboardInput::Key {
                    key: key.to_string(),
                    captured_at_ms,
                    target_window: Some(windows::active_window_target()),
                })
            })
    }

    fn push_input(input: RecordedKeyboardInput) {
        let key_sink = KEY_SINK.lock().ok().and_then(|sink| sink.as_ref().cloned());
        let Some(key_sink) = key_sink else {
            return;
        };
        if let Ok(mut inputs) = key_sink.lock() {
            inputs.push(input);
        };
    }

    fn clear_key_sink() {
        if let Ok(mut sink) = KEY_SINK.lock() {
            *sink = None;
        }
    }

    fn active_modifiers() -> Vec<String> {
        let mut modifiers = Vec::new();
        if key_is_pressed(VK_CONTROL) {
            modifiers.push("Ctrl".to_string());
        }
        if key_is_pressed(VK_MENU) {
            modifiers.push("Alt".to_string());
        }
        if key_is_pressed(VK_SHIFT) {
            modifiers.push("Shift".to_string());
        }
        if key_is_pressed(VK_LWIN) || key_is_pressed(VK_RWIN) {
            modifiers.push("Win".to_string());
        }
        modifiers
    }

    fn key_is_pressed(virtual_key: i32) -> bool {
        unsafe { GetAsyncKeyState(virtual_key) & 0x8000u16 as i16 != 0 }
    }

    fn is_modifier_key(vk_code: u32) -> bool {
        matches!(
            vk_code,
            0x10 | 0x11 | 0x12 | 0x5b | 0x5c | 0xa0 | 0xa1 | 0xa2 | 0xa3 | 0xa4 | 0xa5
        )
    }

    fn hotkey_key_name(vk_code: u32) -> Option<&'static str> {
        match vk_code {
            0x30..=0x39 | 0x41..=0x5a => key_char(vk_code),
            0x70 => Some("F1"),
            0x71 => Some("F2"),
            0x72 => Some("F3"),
            0x73 => Some("F4"),
            0x74 => Some("F5"),
            0x75 => Some("F6"),
            0x76 => Some("F7"),
            0x77 => Some("F8"),
            0x78 => Some("F9"),
            0x79 => Some("F10"),
            0x7a => Some("F11"),
            0x7b => Some("F12"),
            VK_BACK => Some("Backspace"),
            VK_DELETE => Some("Delete"),
            VK_DOWN => Some("Down"),
            VK_ESCAPE => Some("Esc"),
            VK_LEFT => Some("Left"),
            VK_RETURN => Some("Enter"),
            VK_RIGHT => Some("Right"),
            VK_SPACE => Some("Space"),
            VK_TAB => Some("Tab"),
            VK_UP => Some("Up"),
            _ => None,
        }
    }

    fn plain_key_name(vk_code: u32) -> Option<&'static str> {
        match vk_code {
            VK_BACK => Some("Backspace"),
            VK_DELETE => Some("Delete"),
            VK_DOWN => Some("Down"),
            VK_ESCAPE => Some("Esc"),
            VK_LEFT => Some("Left"),
            VK_RETURN => Some("Enter"),
            VK_RIGHT => Some("Right"),
            VK_TAB => Some("Tab"),
            VK_UP => Some("Up"),
            _ => None,
        }
    }

    fn key_char(vk_code: u32) -> Option<&'static str> {
        match vk_code {
            0x30 => Some("0"),
            0x31 => Some("1"),
            0x32 => Some("2"),
            0x33 => Some("3"),
            0x34 => Some("4"),
            0x35 => Some("5"),
            0x36 => Some("6"),
            0x37 => Some("7"),
            0x38 => Some("8"),
            0x39 => Some("9"),
            0x41 => Some("A"),
            0x42 => Some("B"),
            0x43 => Some("C"),
            0x44 => Some("D"),
            0x45 => Some("E"),
            0x46 => Some("F"),
            0x47 => Some("G"),
            0x48 => Some("H"),
            0x49 => Some("I"),
            0x4a => Some("J"),
            0x4b => Some("K"),
            0x4c => Some("L"),
            0x4d => Some("M"),
            0x4e => Some("N"),
            0x4f => Some("O"),
            0x50 => Some("P"),
            0x51 => Some("Q"),
            0x52 => Some("R"),
            0x53 => Some("S"),
            0x54 => Some("T"),
            0x55 => Some("U"),
            0x56 => Some("V"),
            0x57 => Some("W"),
            0x58 => Some("X"),
            0x59 => Some("Y"),
            0x5a => Some("Z"),
            _ => None,
        }
    }

    fn printable_text(vk_code: u32, shifted: bool) -> Option<String> {
        let character = match vk_code {
            0x41..=0x5a => {
                let base = char::from_u32(vk_code)?;
                if shifted {
                    base
                } else {
                    base.to_ascii_lowercase()
                }
            }
            0x30 => {
                if shifted {
                    ')'
                } else {
                    '0'
                }
            }
            0x31 => {
                if shifted {
                    '!'
                } else {
                    '1'
                }
            }
            0x32 => {
                if shifted {
                    '@'
                } else {
                    '2'
                }
            }
            0x33 => {
                if shifted {
                    '#'
                } else {
                    '3'
                }
            }
            0x34 => {
                if shifted {
                    '$'
                } else {
                    '4'
                }
            }
            0x35 => {
                if shifted {
                    '%'
                } else {
                    '5'
                }
            }
            0x36 => {
                if shifted {
                    '^'
                } else {
                    '6'
                }
            }
            0x37 => {
                if shifted {
                    '&'
                } else {
                    '7'
                }
            }
            0x38 => {
                if shifted {
                    '*'
                } else {
                    '8'
                }
            }
            0x39 => {
                if shifted {
                    '('
                } else {
                    '9'
                }
            }
            VK_SPACE => ' ',
            VK_OEM_1 => {
                if shifted {
                    ':'
                } else {
                    ';'
                }
            }
            VK_OEM_PLUS => {
                if shifted {
                    '+'
                } else {
                    '='
                }
            }
            VK_OEM_COMMA => {
                if shifted {
                    '<'
                } else {
                    ','
                }
            }
            VK_OEM_MINUS => {
                if shifted {
                    '_'
                } else {
                    '-'
                }
            }
            VK_OEM_PERIOD => {
                if shifted {
                    '>'
                } else {
                    '.'
                }
            }
            VK_OEM_2 => {
                if shifted {
                    '?'
                } else {
                    '/'
                }
            }
            VK_OEM_3 => {
                if shifted {
                    '~'
                } else {
                    '`'
                }
            }
            VK_OEM_4 => {
                if shifted {
                    '{'
                } else {
                    '['
                }
            }
            VK_OEM_5 => {
                if shifted {
                    '|'
                } else {
                    '\\'
                }
            }
            VK_OEM_6 => {
                if shifted {
                    '}'
                } else {
                    ']'
                }
            }
            VK_OEM_7 => {
                if shifted {
                    '"'
                } else {
                    '\''
                }
            }
            _ => return None,
        };

        Some(character.to_string())
    }

    fn unix_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::RecordedKeyboardInput;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    pub struct KeyboardCaptureGuard;

    impl KeyboardCaptureGuard {
        pub fn stop(self) {}
    }

    pub fn start_key_capture(
        _key_sink: Arc<Mutex<Vec<RecordedKeyboardInput>>>,
    ) -> Result<KeyboardCaptureGuard, String> {
        Err("keyboard capture is only available on Windows".to_string())
    }
}

pub(crate) use platform::KeyboardCaptureGuard;

pub(crate) fn start_key_capture(
    key_sink: Arc<Mutex<Vec<RecordedKeyboardInput>>>,
) -> Result<KeyboardCaptureGuard, String> {
    platform::start_key_capture(key_sink)
}
