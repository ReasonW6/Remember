#[cfg(not(target_os = "windows"))]
use crate::app_state::AppController;
use crate::model::{ButtonState, KeyState, MouseButton};
use crate::player::StepExecutor;
#[cfg(not(target_os = "windows"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "windows"))]
use tauri::AppHandle;

pub const REMEMBER_INPUT_EXTRA_INFO: usize = 0x524d_4d42;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_input_extra_info_fits_32_bit_windows_pointer() {
        assert!(REMEMBER_INPUT_EXTRA_INFO <= u32::MAX as usize);
    }
}

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

    fn key(
        &self,
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
    ) -> Result<(), String> {
        platform::key(vk_code, scan_code, extended, state)
    }

    fn release_mouse_button(&self, button: MouseButton) -> Result<(), String> {
        platform::release_mouse_button(button)
    }
}

#[cfg(target_os = "windows")]
pub use capture::{pause_capture_events, start_capture, CapturePauseGuard, InputCaptureRuntime};

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Default)]
pub struct InputCaptureRuntime;

#[cfg(not(target_os = "windows"))]
pub fn start_capture(
    _shared: Arc<Mutex<AppController>>,
    _app_handle: AppHandle,
    _own_window_hwnds: [Option<usize>; 2],
) -> Result<InputCaptureRuntime, String> {
    Err("Remember input capture is Windows-only".to_string())
}

#[cfg(not(target_os = "windows"))]
pub struct CapturePauseGuard;

#[cfg(not(target_os = "windows"))]
pub fn pause_capture_events() -> Result<CapturePauseGuard, String> {
    Ok(CapturePauseGuard)
}

#[cfg(target_os = "windows")]
mod capture {
    use crate::{
        app_state::{
            AppController, ControlHotkeyAction, ControlHotkeyDecision, ControlHotkeyRuntime,
        },
        clock::now_ms,
        commands,
        input::REMEMBER_INPUT_EXTRA_INFO,
        model::{ButtonState, KeyState, MouseButton},
        recorder::RawInputEvent,
    };
    use std::{
        cell::RefCell,
        sync::{mpsc, Arc, Mutex},
        thread::{self, JoinHandle},
    };
    use tauri::AppHandle;
    use windows::Win32::{
        Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetAncestor, GetForegroundWindow, GetMessageW,
            PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
            UnhookWindowsHookEx, WindowFromPoint, GA_ROOT, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
            LLKHF_EXTENDED, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, WH_KEYBOARD_LL, WH_MOUSE_LL,
            WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
            WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
            WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
        },
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct QueuedControlHotkeyAction {
        action: ControlHotkeyAction,
        at_ms: u64,
    }

    enum CaptureWorkerMessage {
        Input(RawInputEvent),
        Action(QueuedControlHotkeyAction),
        ResetHotkeyFilter,
        Pause {
            reached: mpsc::SyncSender<()>,
            resume: mpsc::Receiver<()>,
        },
        Shutdown,
    }

    struct HookContext {
        control_hotkeys: ControlHotkeyRuntime,
        own_window_hwnds: [Option<usize>; 2],
        capture_event_tx: mpsc::Sender<CaptureWorkerMessage>,
    }

    impl HookContext {
        fn dispatch(&self, message: CaptureWorkerMessage) -> bool {
            self.capture_event_tx.send(message).is_ok()
        }
    }

    // Low-level hook callbacks must return within the system hook timeout or
    // Windows silently removes the hook. Windows invokes these low-level hooks
    // on the thread that installed them, so their immutable context can live in
    // thread-local storage and be read without a shared lock. Input and hotkey
    // actions share its one ordered queue so a start/stop hotkey remains the
    // exact capture boundary without doing disk I/O inside the hook.
    thread_local! {
        static HOOK_CONTEXT: RefCell<Option<HookContext>> = const { RefCell::new(None) };
    }

    // Only non-hook lifecycle and command code uses this sender. Hook callbacks
    // dispatch through their thread-local HookContext above.
    static CAPTURE_CONTROL_TX: Mutex<Option<mpsc::Sender<CaptureWorkerMessage>>> = Mutex::new(None);

    pub struct CapturePauseGuard {
        resume: Option<mpsc::Sender<()>>,
    }

    impl Drop for CapturePauseGuard {
        fn drop(&mut self) {
            if let Some(resume) = self.resume.take() {
                let _ = resume.send(());
            }
        }
    }

    pub struct InputCaptureRuntime {
        hook_thread_id: u32,
        worker: Option<JoinHandle<()>>,
        capture_worker: Option<JoinHandle<()>>,
    }

    impl Drop for InputCaptureRuntime {
        fn drop(&mut self) {
            unsafe {
                let _ = PostThreadMessageW(self.hook_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }

            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }

            stop_capture_worker();
            if let Some(capture_worker) = self.capture_worker.take() {
                let _ = capture_worker.join();
            }
        }
    }

    pub fn start_capture(
        shared: Arc<Mutex<AppController>>,
        app_handle: AppHandle,
        own_window_hwnds: [Option<usize>; 2],
    ) -> Result<InputCaptureRuntime, String> {
        let control_hotkey_runtime = shared
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?
            .control_hotkey_runtime();
        let (capture_tx, capture_rx) = mpsc::channel();
        set_capture_control_sender(capture_tx.clone())?;
        let hook_context = HookContext {
            control_hotkeys: control_hotkey_runtime,
            own_window_hwnds,
            capture_event_tx: capture_tx,
        };
        let capture_shared = shared.clone();
        let action_shared = shared.clone();
        let capture_worker = thread::spawn(move || {
            run_capture_worker_with_actions(capture_shared, capture_rx, |queued| {
                commands::run_control_hotkey_action(
                    app_handle.clone(),
                    action_shared.clone(),
                    queued.action,
                    queued.at_ms,
                );
            });
        });

        let (installed_tx, installed_rx) = mpsc::channel();

        let worker = thread::spawn(move || {
            run_capture_thread(installed_tx, hook_context);
        });

        let cleanup = |worker: JoinHandle<()>, capture_worker: JoinHandle<()>| {
            let _ = worker.join();
            stop_capture_worker();
            let _ = capture_worker.join();
        };

        match installed_rx.recv() {
            Ok(Ok(hook_thread_id)) => Ok(InputCaptureRuntime {
                hook_thread_id,
                worker: Some(worker),
                capture_worker: Some(capture_worker),
            }),
            Ok(Err(error)) => {
                cleanup(worker, capture_worker);
                Err(error)
            }
            Err(_) => {
                cleanup(worker, capture_worker);
                Err("input capture thread stopped before installing hooks".to_string())
            }
        }
    }

    fn set_capture_control_sender(
        sender: mpsc::Sender<CaptureWorkerMessage>,
    ) -> Result<(), String> {
        let mut current = CAPTURE_CONTROL_TX
            .lock()
            .map_err(|_| "capture event queue lock poisoned".to_string())?;
        if current.is_some() {
            return Err("input capture event queue already started".to_string());
        }
        *current = Some(sender);
        Ok(())
    }

    fn current_own_window_hwnds() -> [Option<usize>; 2] {
        HOOK_CONTEXT.with(|current| {
            current
                .borrow()
                .as_ref()
                .map(|context| context.own_window_hwnds)
                .unwrap_or([None, None])
        })
    }

    fn stop_capture_worker() {
        let sender = CAPTURE_CONTROL_TX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(sender) = sender {
            let _ = sender.send(CaptureWorkerMessage::Shutdown);
        }
    }

    fn dispatch_capture_event(event: RawInputEvent) -> bool {
        dispatch_hook_message(CaptureWorkerMessage::Input(event))
    }

    fn dispatch_hook_message(message: CaptureWorkerMessage) -> bool {
        HOOK_CONTEXT.with(|current| {
            current
                .borrow()
                .as_ref()
                .is_some_and(|context| context.dispatch(message))
        })
    }

    pub fn pause_capture_events() -> Result<CapturePauseGuard, String> {
        let (reached_tx, reached_rx) = mpsc::sync_channel(0);
        let (resume_tx, resume_rx) = mpsc::channel();
        let sender = CAPTURE_CONTROL_TX
            .lock()
            .map_err(|_| "capture event queue lock poisoned".to_string())?
            .clone();
        let Some(sender) = sender else {
            return Ok(CapturePauseGuard { resume: None });
        };
        sender
            .send(CaptureWorkerMessage::Pause {
                reached: reached_tx,
                resume: resume_rx,
            })
            .map_err(|_| "input capture event queue stopped".to_string())?;

        reached_rx
            .recv()
            .map_err(|_| "input capture event queue stopped before pause".to_string())?;
        Ok(CapturePauseGuard {
            resume: Some(resume_tx),
        })
    }

    #[cfg(test)]
    fn run_capture_worker(
        shared: Arc<Mutex<AppController>>,
        receiver: mpsc::Receiver<CaptureWorkerMessage>,
    ) {
        run_capture_worker_with_actions(shared, receiver, |_| {});
    }

    fn run_capture_worker_with_actions(
        shared: Arc<Mutex<AppController>>,
        receiver: mpsc::Receiver<CaptureWorkerMessage>,
        mut run_action: impl FnMut(QueuedControlHotkeyAction),
    ) {
        const BATCH_SIZE: usize = 256;
        let mut messages = Vec::with_capacity(BATCH_SIZE);

        while let Ok(message) = receiver.recv() {
            messages.push(message);
            messages.extend(receiver.try_iter().take(BATCH_SIZE - 1));

            let mut controller = None;
            let mut should_shutdown = false;
            for message in messages.drain(..) {
                match message {
                    CaptureWorkerMessage::Input(event) => {
                        if controller.is_none() {
                            controller = shared.lock().ok();
                        }
                        if let Some(controller) = controller.as_mut() {
                            controller.capture_input(event);
                        }
                    }
                    CaptureWorkerMessage::Action(action) => {
                        drop(controller.take());
                        run_action(action);
                    }
                    CaptureWorkerMessage::ResetHotkeyFilter => {
                        if controller.is_none() {
                            controller = shared.lock().ok();
                        }
                        if let Some(controller) = controller.as_mut() {
                            controller.reset_recording_hotkey_filter();
                        }
                    }
                    CaptureWorkerMessage::Pause { reached, resume } => {
                        drop(controller.take());
                        if reached.send(()).is_ok() {
                            let _ = resume.recv();
                        }
                    }
                    CaptureWorkerMessage::Shutdown => {
                        drop(controller.take());
                        should_shutdown = true;
                        break;
                    }
                }
            }
            if should_shutdown {
                break;
            }
        }
    }

    fn dispatch_hotkey_action(action: ControlHotkeyAction, at_ms: u64) -> bool {
        dispatch_hook_message(CaptureWorkerMessage::Action(QueuedControlHotkeyAction {
            action,
            at_ms,
        }))
    }

    fn run_capture_thread(
        installed_tx: mpsc::Sender<Result<u32, String>>,
        hook_context: HookContext,
    ) {
        HOOK_CONTEXT.with(|current| {
            debug_assert!(current.borrow().is_none());
            *current.borrow_mut() = Some(hook_context);
        });
        let mut message = MSG::default();
        unsafe {
            let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        }

        let hooks = match HookHandles::install() {
            Ok(hooks) => {
                let thread_id = unsafe { GetCurrentThreadId() };
                let _ = installed_tx.send(Ok(thread_id));
                hooks
            }
            Err(error) => {
                let _ = installed_tx.send(Err(error));
                return;
            }
        };

        loop {
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
            match result {
                -1 | 0 => break,
                _ => unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                },
            }
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
            let foreground_root_hwnd = foreground_root_window();
            let own_window_hwnds = current_own_window_hwnds();

            if same_root_window(foreground_root_hwnd, own_window_hwnds) {
                reset_control_hotkey_state();
            } else {
                if let Some(event) = raw_key_event(w_param, l_param) {
                    let decision = handle_control_hotkey(event);
                    if let Some(action) = decision.action {
                        let at_ms = match event {
                            RawInputEvent::Key { at_ms, .. } => at_ms,
                            _ => unreachable!("keyboard hook produced a non-key event"),
                        };
                        dispatch_hotkey_action(action, at_ms);
                    }
                    if decision.suppress {
                        return LRESULT(1);
                    }
                }
            }

            if let Some(event) = key_event_from_foreground_root(
                w_param,
                l_param,
                foreground_root_hwnd,
                own_window_hwnds,
            ) {
                capture(event);
            }
        }

        CallNextHookEx(HHOOK::default(), code, w_param, l_param)
    }

    fn handle_control_hotkey(event: RawInputEvent) -> ControlHotkeyDecision {
        HOOK_CONTEXT.with(|current| {
            current
                .borrow()
                .as_ref()
                .map(|context| context.control_hotkeys.decide(event))
                .unwrap_or(ControlHotkeyDecision {
                    suppress: false,
                    action: None,
                })
        })
    }

    fn reset_control_hotkey_state() {
        HOOK_CONTEXT.with(|current| {
            if let Some(context) = current.borrow().as_ref() {
                context.control_hotkeys.reset_action();
            }
        });
        let _ = dispatch_hook_message(CaptureWorkerMessage::ResetHotkeyFilter);
    }

    fn capture(event: RawInputEvent) {
        let _ = dispatch_capture_event(event);
    }

    fn mouse_event(w_param: WPARAM, l_param: LPARAM) -> Option<RawInputEvent> {
        let info = unsafe { (l_param.0 as *const MSLLHOOKSTRUCT).as_ref()? };
        if info.dwExtraInfo == REMEMBER_INPUT_EXTRA_INFO {
            return None;
        }

        let at_ms = now_ms();
        let x = info.pt.x;
        let y = info.pt.y;
        if same_root_window(root_window_from_point(x, y), current_own_window_hwnds()) {
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

    #[cfg(test)]
    fn key_event(w_param: WPARAM, l_param: LPARAM) -> Option<RawInputEvent> {
        key_event_from_foreground_root(
            w_param,
            l_param,
            foreground_root_window(),
            current_own_window_hwnds(),
        )
    }

    fn key_event_from_foreground_root(
        w_param: WPARAM,
        l_param: LPARAM,
        foreground_root_hwnd: Option<usize>,
        own_window_hwnds: [Option<usize>; 2],
    ) -> Option<RawInputEvent> {
        let event = raw_key_event(w_param, l_param)?;
        if same_root_window(foreground_root_hwnd, own_window_hwnds) {
            return None;
        }

        Some(event)
    }

    fn raw_key_event(w_param: WPARAM, l_param: LPARAM) -> Option<RawInputEvent> {
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
            extended: info.flags.contains(LLKHF_EXTENDED),
            state,
        })
    }

    fn foreground_root_window() -> Option<usize> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            return None;
        }

        root_window(hwnd)
    }

    fn root_window_from_point(x: i32, y: i32) -> Option<usize> {
        let hwnd = unsafe { WindowFromPoint(POINT { x, y }) };
        if hwnd.is_invalid() {
            return None;
        }

        root_window(hwnd)
    }

    fn root_window(hwnd: windows::Win32::Foundation::HWND) -> Option<usize> {
        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        if root.is_invalid() {
            None
        } else {
            Some(root.0 as usize)
        }
    }

    fn same_root_window(
        event_root_hwnd: Option<usize>,
        own_window_hwnds: [Option<usize>; 2],
    ) -> bool {
        matches!(
            event_root_hwnd,
            Some(event_root_hwnd)
                if event_root_hwnd != 0
                    && own_window_hwnds.contains(&Some(event_root_hwnd))
        )
    }

    fn high_word(value: u32) -> u16 {
        ((value >> 16) & 0xffff) as u16
    }

    fn signed_high_word(value: u32) -> i16 {
        high_word(value) as i16
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::model::MacroStep;
        use std::sync::mpsc::TryRecvError;
        use std::time::Duration;
        use windows::Win32::Foundation::POINT;

        fn pause_capture_worker(sender: &mpsc::Sender<CaptureWorkerMessage>) -> CapturePauseGuard {
            let (reached_tx, reached_rx) = mpsc::sync_channel(0);
            let (resume_tx, resume_rx) = mpsc::channel();
            sender
                .send(CaptureWorkerMessage::Pause {
                    reached: reached_tx,
                    resume: resume_rx,
                })
                .unwrap();
            reached_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            CapturePauseGuard {
                resume: Some(resume_tx),
            }
        }

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
        fn key_event_ignores_input_when_foreground_root_is_main_window() {
            let info = KBDLLHOOKSTRUCT {
                vkCode: 0x41,
                scanCode: 0x1E,
                flags: Default::default(),
                time: 0,
                dwExtraInfo: 0,
            };

            let event = key_event_from_foreground_root(
                WPARAM(WM_KEYDOWN as usize),
                LPARAM((&info as *const KBDLLHOOKSTRUCT) as isize),
                Some(0x55),
                [Some(0x55), Some(0x66)],
            );

            assert_eq!(event, None);
        }

        #[test]
        fn key_event_keeps_input_when_foreground_root_is_not_main_window() {
            let info = KBDLLHOOKSTRUCT {
                vkCode: 0x41,
                scanCode: 0x1E,
                flags: Default::default(),
                time: 0,
                dwExtraInfo: 0,
            };

            let event = key_event_from_foreground_root(
                WPARAM(WM_KEYDOWN as usize),
                LPARAM((&info as *const KBDLLHOOKSTRUCT) as isize),
                Some(0x55),
                [Some(0x66), Some(0x77)],
            );

            assert!(matches!(
                event,
                Some(RawInputEvent::Key {
                    vk_code: 0x41,
                    scan_code: 0x1E,
                    extended: false,
                    state: KeyState::Pressed,
                    ..
                })
            ));
        }

        #[test]
        fn root_window_match_filters_all_remember_windows() {
            assert!(same_root_window(Some(0x55), [Some(0x55), None]));
            assert!(same_root_window(Some(0x66), [Some(0x55), Some(0x66)]));
            assert!(!same_root_window(Some(0x55), [Some(0x66), None]));
            assert!(!same_root_window(Some(0x55), [None, None]));
            assert!(!same_root_window(None, [Some(0x55), Some(0x66)]));
        }

        #[test]
        fn capture_worker_queues_events_and_hotkeys_stay_responsive_while_controller_is_busy() {
            let shared = Arc::new(Mutex::new(AppController::new()));
            let control_hotkeys = {
                let mut controller = shared.lock().unwrap();
                controller
                    .start_recording("queued", 1_000, "2026-07-25T00:00:00Z")
                    .unwrap();
                controller.control_hotkey_runtime()
            };

            let (tx, rx) = mpsc::channel();
            let worker_shared = shared.clone();
            let worker = thread::spawn(move || run_capture_worker(worker_shared, rx));

            let controller_guard = shared.lock().unwrap();
            let hotkey_decision = control_hotkeys.decide(RawInputEvent::Key {
                at_ms: 1_009,
                vk_code: 0x77,
                scan_code: 0x42,
                extended: false,
                state: KeyState::Pressed,
            });
            assert_eq!(
                hotkey_decision,
                ControlHotkeyDecision {
                    suppress: true,
                    action: Some(ControlHotkeyAction::Stop),
                }
            );

            for event in [
                RawInputEvent::MouseButton {
                    at_ms: 1_010,
                    x: 10,
                    y: 10,
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                },
                RawInputEvent::MouseMove {
                    at_ms: 1_011,
                    x: 11,
                    y: 12,
                },
                RawInputEvent::MouseMove {
                    at_ms: 1_012,
                    x: 12,
                    y: 14,
                },
                RawInputEvent::MouseButton {
                    at_ms: 1_013,
                    x: 12,
                    y: 14,
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                },
            ] {
                tx.send(CaptureWorkerMessage::Input(event)).unwrap();
            }
            let (reached_tx, reached_rx) = mpsc::sync_channel(0);
            let (resume_tx, resume_rx) = mpsc::channel();
            tx.send(CaptureWorkerMessage::Pause {
                reached: reached_tx,
                resume: resume_rx,
            })
            .unwrap();
            assert_eq!(reached_rx.try_recv(), Err(TryRecvError::Empty));

            drop(controller_guard);
            reached_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            resume_tx.send(()).unwrap();
            drop(tx);
            worker.join().unwrap();

            let recording = shared.lock().unwrap().stop_recording(1_020).unwrap();
            assert!(matches!(
                recording.steps.as_slice(),
                [
                    MacroStep::MouseButton {
                        button: MouseButton::Left,
                        state: ButtonState::Pressed,
                        ..
                    },
                    MacroStep::MouseMove { x: 11, y: 12, .. },
                    MacroStep::MouseMove { x: 12, y: 14, .. },
                    MacroStep::MouseButton {
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                        ..
                    }
                ]
            ));
        }

        #[test]
        fn capture_pause_applies_later_events_on_the_new_side_of_mode_transitions() {
            let shared = Arc::new(Mutex::new(AppController::new()));
            let (tx, rx) = mpsc::channel();
            let worker_shared = shared.clone();
            let worker = thread::spawn(move || run_capture_worker(worker_shared, rx));

            let start_pause = pause_capture_worker(&tx);
            tx.send(CaptureWorkerMessage::Input(RawInputEvent::MouseButton {
                at_ms: 1_010,
                x: 10,
                y: 10,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            }))
            .unwrap();
            shared
                .lock()
                .unwrap()
                .start_recording("boundary", 1_000, "2026-07-25T00:00:00Z")
                .unwrap();
            drop(start_pause);

            let stop_pause = pause_capture_worker(&tx);
            tx.send(CaptureWorkerMessage::Input(RawInputEvent::MouseButton {
                at_ms: 1_020,
                x: 20,
                y: 20,
                button: MouseButton::Left,
                state: ButtonState::Released,
            }))
            .unwrap();
            let recording = shared.lock().unwrap().stop_recording(1_015).unwrap();
            drop(stop_pause);

            drop(pause_capture_worker(&tx));
            drop(tx);
            worker.join().unwrap();

            assert!(matches!(
                recording.steps.as_slice(),
                [MacroStep::MouseButton {
                    elapsed_ms: 10,
                    x: 10,
                    y: 10,
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                }]
            ));
        }

        #[test]
        fn immutable_hook_context_preserves_the_ordered_hotkey_capture_boundary() {
            let shared = Arc::new(Mutex::new(AppController::new()));
            shared
                .lock()
                .unwrap()
                .start_recording("boundary", 1_000, "2026-07-25T00:00:00Z")
                .unwrap();
            let stopped_recording = Arc::new(Mutex::new(None));
            let (tx, rx) = mpsc::channel();
            let hook_context = HookContext {
                control_hotkeys: ControlHotkeyRuntime::default(),
                own_window_hwnds: [Some(0x55), Some(0x66)],
                capture_event_tx: tx,
            };
            let lifecycle_sender_guard = CAPTURE_CONTROL_TX
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let (dispatch_done_tx, dispatch_done_rx) = mpsc::channel();
            let hook_worker = thread::spawn(move || {
                HOOK_CONTEXT.with(|current| {
                    *current.borrow_mut() = Some(hook_context);
                });
                assert_eq!(current_own_window_hwnds(), [Some(0x55), Some(0x66)]);
                assert!(dispatch_hook_message(CaptureWorkerMessage::Input(
                    RawInputEvent::MouseButton {
                        at_ms: 1_010,
                        x: 10,
                        y: 10,
                        button: MouseButton::Left,
                        state: ButtonState::Pressed,
                    }
                )));
                assert!(dispatch_hook_message(CaptureWorkerMessage::Action(
                    QueuedControlHotkeyAction {
                        action: ControlHotkeyAction::Stop,
                        at_ms: 1_015,
                    }
                )));
                assert!(dispatch_hook_message(CaptureWorkerMessage::Input(
                    RawInputEvent::MouseButton {
                        at_ms: 1_020,
                        x: 20,
                        y: 20,
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                    }
                )));
                assert!(dispatch_hook_message(CaptureWorkerMessage::Shutdown));
                dispatch_done_tx.send(()).unwrap();
            });
            let worker_shared = shared.clone();
            let action_shared = shared.clone();
            let action_recording = stopped_recording.clone();
            let worker = thread::spawn(move || {
                run_capture_worker_with_actions(worker_shared, rx, |queued| {
                    assert_eq!(queued.action, ControlHotkeyAction::Stop);
                    let recording = action_shared
                        .lock()
                        .unwrap()
                        .stop_recording(queued.at_ms)
                        .unwrap();
                    *action_recording.lock().unwrap() = Some(recording);
                });
            });

            dispatch_done_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("hook dispatch must not wait for the lifecycle sender lock");
            drop(lifecycle_sender_guard);
            hook_worker.join().unwrap();
            worker.join().unwrap();

            let recording = stopped_recording.lock().unwrap().take().unwrap();
            assert_eq!(recording.duration_ms, 15);
            assert!(matches!(
                recording.steps.as_slice(),
                [MacroStep::MouseButton {
                    elapsed_ms: 10,
                    x: 10,
                    y: 10,
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                }]
            ));
        }

        #[test]
        fn capture_worker_shutdown_discards_nothing_queued_before_it() {
            let shared = Arc::new(Mutex::new(AppController::new()));
            shared
                .lock()
                .unwrap()
                .start_recording("shutdown", 1_000, "2026-07-25T00:00:00Z")
                .unwrap();
            let (tx, rx) = mpsc::channel();
            let worker_shared = shared.clone();
            let worker = thread::spawn(move || run_capture_worker(worker_shared, rx));

            tx.send(CaptureWorkerMessage::Input(RawInputEvent::MouseButton {
                at_ms: 1_010,
                x: 10,
                y: 10,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            }))
            .unwrap();
            tx.send(CaptureWorkerMessage::Input(RawInputEvent::MouseButton {
                at_ms: 1_020,
                x: 20,
                y: 20,
                button: MouseButton::Left,
                state: ButtonState::Released,
            }))
            .unwrap();
            tx.send(CaptureWorkerMessage::Shutdown).unwrap();
            worker.join().unwrap();

            let recording = shared.lock().unwrap().stop_recording(1_025).unwrap();
            assert!(matches!(
                recording.steps.as_slice(),
                [
                    MacroStep::MouseButton {
                        elapsed_ms: 10,
                        button: MouseButton::Left,
                        state: ButtonState::Pressed,
                        ..
                    },
                    MacroStep::MouseButton {
                        elapsed_ms: 20,
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                        ..
                    }
                ]
            ));
        }

        #[test]
        fn capture_worker_batches_preserve_every_ordered_drag_point() {
            let shared = Arc::new(Mutex::new(AppController::new()));
            shared
                .lock()
                .unwrap()
                .start_recording("batched-drag", 1_000, "2026-07-25T00:00:00Z")
                .unwrap();
            let (tx, rx) = mpsc::channel();

            tx.send(CaptureWorkerMessage::Input(RawInputEvent::MouseButton {
                at_ms: 1_001,
                x: 0,
                y: 0,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            }))
            .unwrap();
            for point in 1..=600 {
                tx.send(CaptureWorkerMessage::Input(RawInputEvent::MouseMove {
                    at_ms: 1_001 + point,
                    x: point as i32,
                    y: (point * 2) as i32,
                }))
                .unwrap();
            }
            tx.send(CaptureWorkerMessage::Input(RawInputEvent::MouseButton {
                at_ms: 1_602,
                x: 600,
                y: 1_200,
                button: MouseButton::Left,
                state: ButtonState::Released,
            }))
            .unwrap();
            tx.send(CaptureWorkerMessage::Shutdown).unwrap();

            run_capture_worker(shared.clone(), rx);

            let recording = shared.lock().unwrap().stop_recording(1_610).unwrap();
            assert_eq!(recording.steps.len(), 602);
            assert!(matches!(
                recording.steps.get(300),
                Some(MacroStep::MouseMove { x: 300, y: 600, .. })
            ));
            assert!(matches!(
                recording.steps.last(),
                Some(MacroStep::MouseButton {
                    x: 600,
                    y: 1_200,
                    state: ButtonState::Released,
                    ..
                })
            ));
        }

        #[test]
        fn queued_hotkey_filter_reset_clears_earlier_modifier_state_in_order() {
            let shared = Arc::new(Mutex::new(AppController::new()));
            shared
                .lock()
                .unwrap()
                .start_recording("reset", 1_000, "2026-07-25T00:00:00Z")
                .unwrap();
            let (tx, rx) = mpsc::channel();
            let worker_shared = shared.clone();
            let worker = thread::spawn(move || run_capture_worker(worker_shared, rx));

            tx.send(CaptureWorkerMessage::Input(RawInputEvent::Key {
                at_ms: 1_010,
                vk_code: 0xA2,
                scan_code: 0x1D,
                extended: false,
                state: KeyState::Pressed,
            }))
            .unwrap();
            tx.send(CaptureWorkerMessage::ResetHotkeyFilter).unwrap();
            tx.send(CaptureWorkerMessage::Input(RawInputEvent::Key {
                at_ms: 1_020,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            }))
            .unwrap();
            drop(pause_capture_worker(&tx));
            drop(tx);
            worker.join().unwrap();

            let recording = shared.lock().unwrap().stop_recording(1_030).unwrap();
            assert!(matches!(
                recording.steps.as_slice(),
                [MacroStep::Key {
                    elapsed_ms: 20,
                    vk_code: 0x41,
                    state: KeyState::Pressed,
                    ..
                }]
            ));
        }
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
        KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE,
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
        MOUSEEVENTF_MOVE, MOUSEEVENTF_MOVE_NOCOALESCE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
        MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT,
        MOUSE_EVENT_FLAGS, VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN, XBUTTON1, XBUTTON2,
    };

    pub fn mouse_move(x: i32, y: i32) -> Result<(), String> {
        send_positioned_mouse_input(x, y, MOUSE_EVENT_FLAGS(0), 0)
    }

    pub fn mouse_button(
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> Result<(), String> {
        let (flags, mouse_data) = mouse_button_input(button, state);
        send_positioned_mouse_input(x, y, flags, mouse_data)
    }

    pub fn mouse_wheel(x: i32, y: i32, delta: i32) -> Result<(), String> {
        send_positioned_mouse_input(x, y, MOUSEEVENTF_WHEEL, delta as u32)
    }

    pub fn key(
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
    ) -> Result<(), String> {
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if state == KeyState::Released {
            flags |= KEYEVENTF_KEYUP;
        }
        if scan_code != 0 {
            flags |= KEYEVENTF_SCANCODE;
        }
        if extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: if scan_code == 0 {
                        VIRTUAL_KEY(vk_code)
                    } else {
                        VIRTUAL_KEY(0)
                    },
                    wScan: scan_code,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: REMEMBER_INPUT_EXTRA_INFO,
                },
            },
        };

        send_input(input)
    }

    pub fn release_mouse_button(button: MouseButton) -> Result<(), String> {
        let (flags, mouse_data) = mouse_button_input(button, ButtonState::Released);
        send_mouse_input(flags, mouse_data)
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
        let input = mouse_input(MOUSEINPUT {
            dx: 0,
            dy: 0,
            mouseData: mouse_data,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: REMEMBER_INPUT_EXTRA_INFO,
        });

        send_input(input)
    }

    fn send_positioned_mouse_input(
        x: i32,
        y: i32,
        event_flags: MOUSE_EVENT_FLAGS,
        mouse_data: u32,
    ) -> Result<(), String> {
        let bounds = virtual_desktop_bounds()?;
        let input = positioned_mouse_input(x, y, bounds, event_flags, mouse_data);
        send_input(mouse_input(input))
    }

    fn virtual_desktop_bounds() -> Result<(i32, i32, i32, i32), String> {
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        if width <= 0 || height <= 0 {
            return Err("GetSystemMetrics virtual desktop failed".to_string());
        }

        Ok((left, top, width, height))
    }

    fn positioned_mouse_input(
        x: i32,
        y: i32,
        bounds: (i32, i32, i32, i32),
        event_flags: MOUSE_EVENT_FLAGS,
        mouse_data: u32,
    ) -> MOUSEINPUT {
        let (left, top, width, height) = bounds;
        MOUSEINPUT {
            dx: normalize_absolute_coordinate(x, left, width),
            dy: normalize_absolute_coordinate(y, top, height),
            mouseData: mouse_data,
            dwFlags: MOUSEEVENTF_MOVE
                | MOUSEEVENTF_ABSOLUTE
                | MOUSEEVENTF_VIRTUALDESK
                | MOUSEEVENTF_MOVE_NOCOALESCE
                | event_flags,
            time: 0,
            dwExtraInfo: REMEMBER_INPUT_EXTRA_INFO,
        }
    }

    fn normalize_absolute_coordinate(coordinate: i32, origin: i32, extent: i32) -> i32 {
        if extent <= 1 {
            return 0;
        }

        let last_offset = i64::from(extent) - 1;
        let offset = (i64::from(coordinate) - i64::from(origin)).clamp(0, last_offset);
        ((offset * i64::from(u16::MAX)) / last_offset) as i32
    }

    fn mouse_input(input: MOUSEINPUT) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi: input },
        }
    }

    fn send_input(input: INPUT) -> Result<(), String> {
        let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
        if sent == 1 {
            Ok(())
        } else {
            Err("SendInput failed".to_string())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn normalizes_negative_virtual_desktop_edges() {
            assert_eq!(normalize_absolute_coordinate(-1_920, -1_920, 4_480), 0);
            assert_eq!(
                normalize_absolute_coordinate(2_559, -1_920, 4_480),
                i32::from(u16::MAX)
            );
        }

        #[test]
        fn clamps_coordinates_outside_the_virtual_desktop() {
            assert_eq!(normalize_absolute_coordinate(-101, -100, 200), 0);
            assert_eq!(
                normalize_absolute_coordinate(100, -100, 200),
                i32::from(u16::MAX)
            );
        }

        #[test]
        fn handles_empty_and_single_pixel_extents() {
            assert_eq!(normalize_absolute_coordinate(500, 100, -1), 0);
            assert_eq!(normalize_absolute_coordinate(500, 100, 0), 0);
            assert_eq!(normalize_absolute_coordinate(500, 100, 1), 0);
        }

        #[test]
        fn normalizes_extreme_i32_coordinates_without_overflow() {
            assert_eq!(
                normalize_absolute_coordinate(i32::MIN, i32::MIN, i32::MAX),
                0
            );
            assert_eq!(
                normalize_absolute_coordinate(i32::MAX, i32::MIN, i32::MAX),
                i32::from(u16::MAX)
            );
        }

        #[test]
        fn positioned_button_input_includes_absolute_virtual_desktop_flags_and_sentinel() {
            let input =
                positioned_mouse_input(320, 240, (0, 0, 1_920, 1_080), MOUSEEVENTF_LEFTDOWN, 0);

            assert_eq!(
                input.dwFlags,
                MOUSEEVENTF_MOVE
                    | MOUSEEVENTF_ABSOLUTE
                    | MOUSEEVENTF_VIRTUALDESK
                    | MOUSEEVENTF_MOVE_NOCOALESCE
                    | MOUSEEVENTF_LEFTDOWN
            );
            assert_eq!(input.dwExtraInfo, REMEMBER_INPUT_EXTRA_INFO);
        }

        #[test]
        fn positioned_wheel_input_keeps_mouse_data() {
            let input = positioned_mouse_input(
                320,
                240,
                (0, 0, 1_920, 1_080),
                MOUSEEVENTF_WHEEL,
                (-120_i32) as u32,
            );

            assert_eq!(input.mouseData, (-120_i32) as u32);
            assert!(input.dwFlags.contains(MOUSEEVENTF_WHEEL));
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

    pub fn key(
        _vk_code: u16,
        _scan_code: u16,
        _extended: bool,
        _state: KeyState,
    ) -> Result<(), String> {
        Err(WINDOWS_ONLY_MESSAGE.to_string())
    }

    pub fn release_mouse_button(_button: MouseButton) -> Result<(), String> {
        Err(WINDOWS_ONLY_MESSAGE.to_string())
    }
}
