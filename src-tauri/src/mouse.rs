use crate::recorder::{RecordedMouseClick, RecordedMouseDrag, RecordedMouseScroll};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
mod platform {
    use super::{RecordedMouseClick, RecordedMouseDrag, RecordedMouseScroll};
    use crate::{recorder::RecordedMouseButton, storage::TargetWindow, windows};
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
    const WM_LBUTTONUP: usize = 0x0202;
    const WM_RBUTTONDOWN: usize = 0x0204;
    const WM_RBUTTONUP: usize = 0x0205;
    const WM_MOUSEWHEEL: usize = 0x020A;
    const WM_MOUSEHWHEEL: usize = 0x020E;
    const WM_QUIT: u32 = 0x0012;
    const DRAG_THRESHOLD_PX: i32 = 5;

    static CLICK_SINK: Mutex<Option<Arc<Mutex<Vec<RecordedMouseClick>>>>> = Mutex::new(None);
    static DRAG_SINK: Mutex<Option<Arc<Mutex<Vec<RecordedMouseDrag>>>>> = Mutex::new(None);
    static SCROLL_SINK: Mutex<Option<Arc<Mutex<Vec<RecordedMouseScroll>>>>> = Mutex::new(None);
    static DOWN_EVENT: Mutex<Option<MouseDownEvent>> = Mutex::new(None);

    #[derive(Debug, Clone)]
    struct MouseDownEvent {
        x: i32,
        y: i32,
        button: RecordedMouseButton,
        captured_at_ms: u64,
        target_window: TargetWindow,
    }

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

    pub fn start_mouse_capture(
        click_sink: Arc<Mutex<Vec<RecordedMouseClick>>>,
        drag_sink: Arc<Mutex<Vec<RecordedMouseDrag>>>,
        scroll_sink: Arc<Mutex<Vec<RecordedMouseScroll>>>,
    ) -> Result<MouseCaptureGuard, String> {
        let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
        let join_handle = thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            if let Ok(mut sink) = CLICK_SINK.lock() {
                *sink = Some(click_sink);
            }
            if let Ok(mut sink) = DRAG_SINK.lock() {
                *sink = Some(drag_sink);
            }
            if let Ok(mut sink) = SCROLL_SINK.lock() {
                *sink = Some(scroll_sink);
            }

            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let hook =
                unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), instance, 0) };

            if hook.is_null() {
                clear_sinks();
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
            clear_sinks();
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
            if let Some(button) = mouse_down_button(w_param) {
                let hook = &*(l_param as *const MouseHookStruct);
                remember_down_event(MouseDownEvent {
                    x: hook.pt.x,
                    y: hook.pt.y,
                    button,
                    captured_at_ms: unix_millis(),
                    target_window: windows::target_window_at_point(hook.pt.x, hook.pt.y),
                });
            } else if let Some(button) = mouse_up_button(w_param) {
                let hook = &*(l_param as *const MouseHookStruct);
                finish_down_event(button, hook.pt.x, hook.pt.y, unix_millis());
            } else if w_param == WM_MOUSEWHEEL || w_param == WM_MOUSEHWHEEL {
                let hook = &*(l_param as *const MouseHookStruct);
                let delta = wheel_delta(hook.mouse_data);
                let (delta_x, delta_y) = if w_param == WM_MOUSEHWHEEL {
                    (delta, 0)
                } else {
                    (0, delta)
                };
                push_scroll(RecordedMouseScroll {
                    x: hook.pt.x,
                    y: hook.pt.y,
                    delta_x,
                    delta_y,
                    captured_at_ms: unix_millis(),
                    target_window: Some(windows::target_window_at_point(hook.pt.x, hook.pt.y)),
                });
            }
        }

        CallNextHookEx(null_mut(), code, w_param, l_param)
    }

    fn mouse_down_button(w_param: usize) -> Option<RecordedMouseButton> {
        match w_param {
            WM_LBUTTONDOWN => Some(RecordedMouseButton::Left),
            WM_RBUTTONDOWN => Some(RecordedMouseButton::Right),
            _ => None,
        }
    }

    fn mouse_up_button(w_param: usize) -> Option<RecordedMouseButton> {
        match w_param {
            WM_LBUTTONUP => Some(RecordedMouseButton::Left),
            WM_RBUTTONUP => Some(RecordedMouseButton::Right),
            _ => None,
        }
    }

    fn remember_down_event(mut event: MouseDownEvent) {
        if event.target_window.process.is_empty() {
            event.target_window = windows::target_window_at_point(event.x, event.y);
        }
        if let Ok(mut down_event) = DOWN_EVENT.lock() {
            *down_event = Some(event);
        }
    }

    fn finish_down_event(button: RecordedMouseButton, x: i32, y: i32, captured_at_ms: u64) {
        let down_event = DOWN_EVENT.lock().ok().and_then(|mut event| event.take());
        let Some(down_event) = down_event else {
            return;
        };
        if down_event.button != button {
            return;
        }

        if is_drag(&down_event, x, y) {
            push_drag(RecordedMouseDrag {
                start_x: down_event.x,
                start_y: down_event.y,
                end_x: x,
                end_y: y,
                button,
                started_at_ms: down_event.captured_at_ms,
                captured_at_ms,
                target_window: Some(down_event.target_window),
            });
        } else {
            push_click(RecordedMouseClick {
                x: down_event.x,
                y: down_event.y,
                button,
                captured_at_ms: down_event.captured_at_ms,
                target_window: Some(down_event.target_window),
            });
        }
    }

    fn is_drag(down_event: &MouseDownEvent, x: i32, y: i32) -> bool {
        (x - down_event.x).abs() > DRAG_THRESHOLD_PX || (y - down_event.y).abs() > DRAG_THRESHOLD_PX
    }

    fn wheel_delta(mouse_data: u32) -> i32 {
        ((mouse_data >> 16) as u16 as i16) as i32
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

    fn push_drag(drag: RecordedMouseDrag) {
        let drag_sink = DRAG_SINK
            .lock()
            .ok()
            .and_then(|sink| sink.as_ref().cloned());
        let Some(drag_sink) = drag_sink else {
            return;
        };
        if let Ok(mut drags) = drag_sink.lock() {
            drags.push(drag);
        };
    }

    fn push_scroll(scroll: RecordedMouseScroll) {
        let scroll_sink = SCROLL_SINK
            .lock()
            .ok()
            .and_then(|sink| sink.as_ref().cloned());
        let Some(scroll_sink) = scroll_sink else {
            return;
        };
        if let Ok(mut scrolls) = scroll_sink.lock() {
            scrolls.push(scroll);
        };
    }

    fn clear_sinks() {
        if let Ok(mut sink) = CLICK_SINK.lock() {
            *sink = None;
        }
        if let Ok(mut sink) = DRAG_SINK.lock() {
            *sink = None;
        }
        if let Ok(mut sink) = SCROLL_SINK.lock() {
            *sink = None;
        }
        if let Ok(mut down_event) = DOWN_EVENT.lock() {
            *down_event = None;
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
    use super::{RecordedMouseClick, RecordedMouseDrag, RecordedMouseScroll};
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    pub struct MouseCaptureGuard;

    impl MouseCaptureGuard {
        pub fn stop(self) {}
    }

    pub fn start_mouse_capture(
        _click_sink: Arc<Mutex<Vec<RecordedMouseClick>>>,
        _drag_sink: Arc<Mutex<Vec<RecordedMouseDrag>>>,
        _scroll_sink: Arc<Mutex<Vec<RecordedMouseScroll>>>,
    ) -> Result<MouseCaptureGuard, String> {
        Err("mouse capture is only available on Windows".to_string())
    }
}

pub(crate) use platform::MouseCaptureGuard;

pub(crate) fn start_mouse_capture(
    click_sink: Arc<Mutex<Vec<RecordedMouseClick>>>,
    drag_sink: Arc<Mutex<Vec<RecordedMouseDrag>>>,
    scroll_sink: Arc<Mutex<Vec<RecordedMouseScroll>>>,
) -> Result<MouseCaptureGuard, String> {
    platform::start_mouse_capture(click_sink, drag_sink, scroll_sink)
}
