use crate::recorder::RecordedMouseClick;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
mod platform {
    use super::RecordedMouseClick;
    use crate::{recorder::RecordedMouseButton, windows};
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

    const WH_MOUSE_LL: i32 = 14;
    const WM_LBUTTONDOWN: usize = 0x0201;
    const WM_RBUTTONDOWN: usize = 0x0204;
    const WM_QUIT: u32 = 0x0012;

    static CLICK_SINK: Mutex<Option<Arc<Mutex<Vec<RecordedMouseClick>>>>> = Mutex::new(None);

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
    struct MouseHookStruct {
        pt: Point,
        mouse_data: u32,
        flags: u32,
        time: u32,
        extra_info: usize,
    }

    #[link(name = "user32")]
    extern "system" {
        fn CallNextHookEx(hook: Handle, code: i32, w_param: usize, l_param: isize) -> isize;
        fn DispatchMessageW(message: *const Msg) -> isize;
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
    pub struct MouseCaptureGuard {
        thread_id: u32,
        join_handle: Option<JoinHandle<()>>,
    }

    impl MouseCaptureGuard {
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

    impl Drop for MouseCaptureGuard {
        fn drop(&mut self) {
            self.stop_inner();
        }
    }

    pub fn start_click_capture(
        click_sink: Arc<Mutex<Vec<RecordedMouseClick>>>,
    ) -> Result<MouseCaptureGuard, String> {
        let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
        let join_handle = thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            if let Ok(mut sink) = CLICK_SINK.lock() {
                *sink = Some(click_sink);
            }

            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let hook =
                unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), instance, 0) };

            if hook.is_null() {
                clear_click_sink();
                let _ = ready_sender.send(Err("failed to install mouse hook".to_string()));
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
            clear_click_sink();
        });

        match ready_receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(thread_id)) => Ok(MouseCaptureGuard {
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

    unsafe extern "system" fn mouse_hook_proc(code: i32, w_param: usize, l_param: isize) -> isize {
        if code >= 0 {
            let button = match w_param {
                WM_LBUTTONDOWN => Some(RecordedMouseButton::Left),
                WM_RBUTTONDOWN => Some(RecordedMouseButton::Right),
                _ => None,
            };

            if let Some(button) = button {
                let hook = &*(l_param as *const MouseHookStruct);
                push_click(RecordedMouseClick {
                    x: hook.pt.x,
                    y: hook.pt.y,
                    button,
                    captured_at_ms: unix_millis(),
                    target_window: Some(windows::target_window_at_point(hook.pt.x, hook.pt.y)),
                });
            }
        }

        CallNextHookEx(null_mut(), code, w_param, l_param)
    }

    fn push_click(click: RecordedMouseClick) {
        let click_sink = CLICK_SINK
            .lock()
            .ok()
            .and_then(|sink| sink.as_ref().cloned());
        let Some(click_sink) = click_sink else {
            return;
        };
        if let Ok(mut clicks) = click_sink.lock() {
            clicks.push(click);
        };
    }

    fn clear_click_sink() {
        if let Ok(mut sink) = CLICK_SINK.lock() {
            *sink = None;
        }
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
    use super::RecordedMouseClick;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    pub struct MouseCaptureGuard;

    impl MouseCaptureGuard {
        pub fn stop(self) {}
    }

    pub fn start_click_capture(
        _click_sink: Arc<Mutex<Vec<RecordedMouseClick>>>,
    ) -> Result<MouseCaptureGuard, String> {
        Err("mouse capture is only available on Windows".to_string())
    }
}

pub(crate) use platform::MouseCaptureGuard;

pub(crate) fn start_click_capture(
    click_sink: Arc<Mutex<Vec<RecordedMouseClick>>>,
) -> Result<MouseCaptureGuard, String> {
    platform::start_click_capture(click_sink)
}
