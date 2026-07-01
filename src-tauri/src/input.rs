#[cfg(not(target_os = "windows"))]
use crate::app_state::AppController;
use crate::model::{ButtonState, KeyState, MouseButton};
use crate::player::StepExecutor;
#[cfg(not(target_os = "windows"))]
use std::sync::{Arc, Mutex};

pub const REMEMBER_INPUT_EXTRA_INFO: usize = 0x524d_4d42_5249_4e50;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemInputExecutor;

impl StepExecutor for SystemInputExecutor {
    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        platform::mouse_move(x, y)
    }

    fn mouse_button(
        &self,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> Result<(), String> {
        platform::mouse_button(x, y, button, state)
    }

    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String> {
        platform::mouse_wheel(x, y, delta)
    }

    fn key(&self, vk_code: u16, scan_code: u16, state: KeyState) -> Result<(), String> {
        platform::key(vk_code, scan_code, state)
    }
}

#[cfg(target_os = "windows")]
pub use capture::{start_capture, InputCaptureRuntime};

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Default)]
pub struct InputCaptureRuntime;

#[cfg(not(target_os = "windows"))]
pub fn start_capture(
    _shared: Arc<Mutex<AppController>>,
    _main_window_hwnd: Option<usize>,
) -> Result<InputCaptureRuntime, String> {
    Err("Remember input capture is Windows-only".to_string())
}

#[cfg(target_os = "windows")]
mod capture {
    use crate::{
        app_state::AppController,
        input::REMEMBER_INPUT_EXTRA_INFO,
        model::{ButtonState, KeyState, MouseButton},
        recorder::RawInputEvent,
    };
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc, Mutex,
        },
        thread::{self, JoinHandle},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use windows::Win32::{
        Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetAncestor, PeekMessageW, SetWindowsHookExW,
            TranslateMessage, UnhookWindowsHookEx, WindowFromPoint, GA_ROOT, HC_ACTION, HHOOK,
            KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PM_REMOVE, WH_KEYBOARD_LL, WH_MOUSE_LL,
            WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
            WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
            WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
        },
    };

    static CAPTURE_CONTROLLER: Mutex<Option<Arc<Mutex<AppController>>>> = Mutex::new(None);
    static MAIN_WINDOW_HWND: Mutex<Option<usize>> = Mutex::new(None);

    pub struct InputCaptureRuntime {
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }

    impl Drop for InputCaptureRuntime {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);

            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }

            clear_capture_controller();
            clear_main_window_hwnd();
        }
    }

    pub fn start_capture(
        shared: Arc<Mutex<AppController>>,
        main_window_hwnd: Option<usize>,
    ) -> Result<InputCaptureRuntime, String> {
        set_capture_controller(shared)?;
        set_main_window_hwnd(main_window_hwnd);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop.clone();
        let (installed_tx, installed_rx) = mpsc::channel();

        let worker = thread::spawn(move || {
            run_capture_thread(stop_for_thread, installed_tx);
        });

        match installed_rx.recv() {
            Ok(Ok(())) => Ok(InputCaptureRuntime {
                stop,
                worker: Some(worker),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                clear_capture_controller();
                clear_main_window_hwnd();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                clear_capture_controller();
                clear_main_window_hwnd();
                Err("input capture thread stopped before installing hooks".to_string())
            }
        }
    }

    fn set_capture_controller(shared: Arc<Mutex<AppController>>) -> Result<(), String> {
        let mut controller = CAPTURE_CONTROLLER
            .lock()
            .map_err(|_| "input capture lock poisoned".to_string())?;
        if controller.is_some() {
            return Err("input capture already started".to_string());
        }

        *controller = Some(shared);
        Ok(())
    }

    fn clear_capture_controller() {
        if let Ok(mut controller) = CAPTURE_CONTROLLER.lock() {
            *controller = None;
        }
    }

    fn set_main_window_hwnd(hwnd: Option<usize>) {
        if let Ok(mut main_window_hwnd) = MAIN_WINDOW_HWND.lock() {
            *main_window_hwnd = hwnd;
        }
    }

    fn clear_main_window_hwnd() {
        if let Ok(mut main_window_hwnd) = MAIN_WINDOW_HWND.lock() {
            *main_window_hwnd = None;
        }
    }

    fn current_main_window_hwnd() -> Option<usize> {
        MAIN_WINDOW_HWND.lock().ok().and_then(|hwnd| *hwnd)
    }

    fn run_capture_thread(stop: Arc<AtomicBool>, installed_tx: mpsc::Sender<Result<(), String>>) {
        let hooks = match HookHandles::install() {
            Ok(hooks) => {
                let _ = installed_tx.send(Ok(()));
                hooks
            }
            Err(error) => {
                let _ = installed_tx.send(Err(error));
                return;
            }
        };

        let mut message = MSG::default();
        while !stop.load(Ordering::SeqCst) {
            unsafe {
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }

            thread::sleep(Duration::from_millis(10));
        }

        hooks.unhook();
    }

    struct HookHandles {
        mouse: HHOOK,
        keyboard: HHOOK,
    }

    impl HookHandles {
        fn install() -> Result<Self, String> {
            let module = unsafe { GetModuleHandleW(None) }
                .map_err(|error| format!("GetModuleHandleW failed: {error}"))?;
            let instance = HINSTANCE(module.0);

            let mouse =
                unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), instance, 0) }
                    .map_err(|error| format!("SetWindowsHookExW mouse hook failed: {error}"))?;

            let keyboard = match unsafe {
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), instance, 0)
            } {
                Ok(keyboard) => keyboard,
                Err(error) => {
                    unsafe {
                        let _ = UnhookWindowsHookEx(mouse);
                    }
                    return Err(format!("SetWindowsHookExW keyboard hook failed: {error}"));
                }
            };

            Ok(Self { mouse, keyboard })
        }

        fn unhook(self) {
            unsafe {
                let _ = UnhookWindowsHookEx(self.mouse);
                let _ = UnhookWindowsHookEx(self.keyboard);
            }
        }
    }

    unsafe extern "system" fn mouse_hook_proc(
        code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if code == HC_ACTION as i32 {
            if let Some(event) = mouse_event(w_param, l_param) {
                capture(event);
            }
        }

        CallNextHookEx(HHOOK::default(), code, w_param, l_param)
    }

    unsafe extern "system" fn keyboard_hook_proc(
        code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if code == HC_ACTION as i32 {
            if let Some(event) = key_event(w_param, l_param) {
                capture(event);
            }
        }

        CallNextHookEx(HHOOK::default(), code, w_param, l_param)
    }

    fn capture(event: RawInputEvent) {
        let shared = match CAPTURE_CONTROLLER.lock() {
            Ok(controller) => controller.clone(),
            Err(_) => None,
        };

        if let Some(shared) = shared {
            if let Ok(mut controller) = shared.lock() {
                controller.capture_input(event);
            }
        }
    }

    fn mouse_event(w_param: WPARAM, l_param: LPARAM) -> Option<RawInputEvent> {
        let info = unsafe { (l_param.0 as *const MSLLHOOKSTRUCT).as_ref()? };
        if info.dwExtraInfo == REMEMBER_INPUT_EXTRA_INFO {
            return None;
        }

        let at_ms = now_ms();
        let x = info.pt.x;
        let y = info.pt.y;
        if same_root_window(root_window_from_point(x, y), current_main_window_hwnd()) {
            return None;
        }

        match w_param.0 as u32 {
            WM_MOUSEMOVE => Some(RawInputEvent::MouseMove { at_ms, x, y }),
            WM_LBUTTONDOWN => Some(mouse_button(
                at_ms,
                x,
                y,
                MouseButton::Left,
                ButtonState::Pressed,
            )),
            WM_LBUTTONUP => Some(mouse_button(
                at_ms,
                x,
                y,
                MouseButton::Left,
                ButtonState::Released,
            )),
            WM_RBUTTONDOWN => Some(mouse_button(
                at_ms,
                x,
                y,
                MouseButton::Right,
                ButtonState::Pressed,
            )),
            WM_RBUTTONUP => Some(mouse_button(
                at_ms,
                x,
                y,
                MouseButton::Right,
                ButtonState::Released,
            )),
            WM_MBUTTONDOWN => Some(mouse_button(
                at_ms,
                x,
                y,
                MouseButton::Middle,
                ButtonState::Pressed,
            )),
            WM_MBUTTONUP => Some(mouse_button(
                at_ms,
                x,
                y,
                MouseButton::Middle,
                ButtonState::Released,
            )),
            WM_XBUTTONDOWN => x_button(info.mouseData)
                .map(|button| mouse_button(at_ms, x, y, button, ButtonState::Pressed)),
            WM_XBUTTONUP => x_button(info.mouseData)
                .map(|button| mouse_button(at_ms, x, y, button, ButtonState::Released)),
            WM_MOUSEWHEEL => Some(RawInputEvent::MouseWheel {
                at_ms,
                x,
                y,
                delta: signed_high_word(info.mouseData) as i32,
            }),
            _ => None,
        }
    }

    fn mouse_button(
        at_ms: u64,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> RawInputEvent {
        RawInputEvent::MouseButton {
            at_ms,
            x,
            y,
            button,
            state,
        }
    }

    fn x_button(mouse_data: u32) -> Option<MouseButton> {
        match u32::from(high_word(mouse_data)) {
            value if value == u32::from(XBUTTON1) => Some(MouseButton::X1),
            value if value == u32::from(XBUTTON2) => Some(MouseButton::X2),
            _ => None,
        }
    }

    fn key_event(w_param: WPARAM, l_param: LPARAM) -> Option<RawInputEvent> {
        let info = unsafe { (l_param.0 as *const KBDLLHOOKSTRUCT).as_ref()? };
        if info.dwExtraInfo == REMEMBER_INPUT_EXTRA_INFO {
            return None;
        }

        let state = match w_param.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => KeyState::Pressed,
            WM_KEYUP | WM_SYSKEYUP => KeyState::Released,
            _ => return None,
        };

        Some(RawInputEvent::Key {
            at_ms: now_ms(),
            vk_code: info.vkCode.try_into().ok()?,
            scan_code: info.scanCode.try_into().ok()?,
            state,
        })
    }

    fn root_window_from_point(x: i32, y: i32) -> Option<usize> {
        let hwnd = unsafe { WindowFromPoint(POINT { x, y }) };
        if hwnd.is_invalid() {
            return None;
        }

        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        if root.is_invalid() {
            None
        } else {
            Some(root.0 as usize)
        }
    }

    fn same_root_window(event_root_hwnd: Option<usize>, main_window_hwnd: Option<usize>) -> bool {
        matches!(
            (event_root_hwnd, main_window_hwnd),
            (Some(event_root_hwnd), Some(main_window_hwnd))
                if event_root_hwnd != 0 && event_root_hwnd == main_window_hwnd
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::Foundation::POINT;

        #[test]
        fn mouse_event_ignores_remember_playback_sentinel() {
            let info = MSLLHOOKSTRUCT {
                pt: POINT { x: 10, y: 20 },
                mouseData: 0,
                flags: 0,
                time: 0,
                dwExtraInfo: REMEMBER_INPUT_EXTRA_INFO,
            };

            let event = mouse_event(
                WPARAM(WM_LBUTTONDOWN as usize),
                LPARAM((&info as *const MSLLHOOKSTRUCT) as isize),
            );

            assert_eq!(event, None);
        }

        #[test]
        fn key_event_ignores_remember_playback_sentinel() {
            let info = KBDLLHOOKSTRUCT {
                vkCode: 0x41,
                scanCode: 0x1E,
                flags: Default::default(),
                time: 0,
                dwExtraInfo: REMEMBER_INPUT_EXTRA_INFO,
            };

            let event = key_event(
                WPARAM(WM_KEYDOWN as usize),
                LPARAM((&info as *const KBDLLHOOKSTRUCT) as isize),
            );

            assert_eq!(event, None);
        }

        #[test]
        fn root_window_match_filters_main_window_input_only() {
            assert!(same_root_window(Some(0x55), Some(0x55)));
            assert!(!same_root_window(Some(0x55), Some(0x66)));
            assert!(!same_root_window(Some(0x55), None));
            assert!(!same_root_window(None, Some(0x55)));
        }
    }

    fn high_word(value: u32) -> u16 {
        ((value >> 16) & 0xffff) as u16
    }

    fn signed_high_word(value: u32) -> i16 {
        high_word(value) as i16
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use crate::{
        input::REMEMBER_INPUT_EXTRA_INFO,
        model::{ButtonState, KeyState, MouseButton},
    };
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
        MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
        MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{SetCursorPos, XBUTTON1, XBUTTON2};

    pub fn mouse_move(x: i32, y: i32) -> Result<(), String> {
        unsafe { SetCursorPos(x, y) }.map_err(|error| format!("SetCursorPos failed: {error}"))
    }

    pub fn mouse_button(
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> Result<(), String> {
        mouse_move(x, y)?;
        let (flags, mouse_data) = mouse_button_input(button, state);
        send_mouse_input(flags, mouse_data)
    }

    pub fn mouse_wheel(x: i32, y: i32, delta: i32) -> Result<(), String> {
        mouse_move(x, y)?;
        send_mouse_input(MOUSEEVENTF_WHEEL, delta as u32)
    }

    pub fn key(vk_code: u16, scan_code: u16, state: KeyState) -> Result<(), String> {
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if state == KeyState::Released {
            flags |= KEYEVENTF_KEYUP;
        }

        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk_code),
                    wScan: scan_code,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: REMEMBER_INPUT_EXTRA_INFO,
                },
            },
        };

        send_input(input)
    }

    fn mouse_button_input(button: MouseButton, state: ButtonState) -> (MOUSE_EVENT_FLAGS, u32) {
        match (button, state) {
            (MouseButton::Left, ButtonState::Pressed) => (MOUSEEVENTF_LEFTDOWN, 0),
            (MouseButton::Left, ButtonState::Released) => (MOUSEEVENTF_LEFTUP, 0),
            (MouseButton::Right, ButtonState::Pressed) => (MOUSEEVENTF_RIGHTDOWN, 0),
            (MouseButton::Right, ButtonState::Released) => (MOUSEEVENTF_RIGHTUP, 0),
            (MouseButton::Middle, ButtonState::Pressed) => (MOUSEEVENTF_MIDDLEDOWN, 0),
            (MouseButton::Middle, ButtonState::Released) => (MOUSEEVENTF_MIDDLEUP, 0),
            (MouseButton::X1, ButtonState::Pressed) => (MOUSEEVENTF_XDOWN, u32::from(XBUTTON1)),
            (MouseButton::X1, ButtonState::Released) => (MOUSEEVENTF_XUP, u32::from(XBUTTON1)),
            (MouseButton::X2, ButtonState::Pressed) => (MOUSEEVENTF_XDOWN, u32::from(XBUTTON2)),
            (MouseButton::X2, ButtonState::Released) => (MOUSEEVENTF_XUP, u32::from(XBUTTON2)),
        }
    }

    fn send_mouse_input(flags: MOUSE_EVENT_FLAGS, mouse_data: u32) -> Result<(), String> {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: mouse_data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: REMEMBER_INPUT_EXTRA_INFO,
                },
            },
        };

        send_input(input)
    }

    fn send_input(input: INPUT) -> Result<(), String> {
        let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
        if sent == 1 {
            Ok(())
        } else {
            Err("SendInput failed".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use crate::model::{ButtonState, KeyState, MouseButton};

    const WINDOWS_ONLY_MESSAGE: &str = "Remember input playback is Windows-only";

    pub fn mouse_move(_x: i32, _y: i32) -> Result<(), String> {
        Err(WINDOWS_ONLY_MESSAGE.to_string())
    }

    pub fn mouse_button(
        _x: i32,
        _y: i32,
        _button: MouseButton,
        _state: ButtonState,
    ) -> Result<(), String> {
        Err(WINDOWS_ONLY_MESSAGE.to_string())
    }

    pub fn mouse_wheel(_x: i32, _y: i32, _delta: i32) -> Result<(), String> {
        Err(WINDOWS_ONLY_MESSAGE.to_string())
    }

    pub fn key(_vk_code: u16, _scan_code: u16, _state: KeyState) -> Result<(), String> {
        Err(WINDOWS_ONLY_MESSAGE.to_string())
    }
}
