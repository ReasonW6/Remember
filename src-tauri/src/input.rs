#[cfg(not(target_os = "windows"))]
use crate::app_state::AppController;
use crate::model::{ButtonState, KeyState, MouseButton};
use crate::player::StepExecutor;
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(not(target_os = "windows"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "windows"))]
use tauri::AppHandle;

pub const REMEMBER_INPUT_EXTRA_INFO: usize = 0x524d_4d42;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowLifetimeToken(u64);

impl WindowLifetimeToken {
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Default)]
pub struct OwnWindowHandles {
    main: AtomicUsize,
    advanced_settings: AtomicUsize,
}

impl OwnWindowHandles {
    pub fn set_main(&self, hwnd: usize) {
        self.main.store(hwnd, Ordering::Release);
    }

    pub fn set_advanced_settings(&self, hwnd: usize) {
        self.advanced_settings.store(hwnd, Ordering::Release);
    }

    pub fn clear_advanced_settings(&self) {
        self.advanced_settings.store(0, Ordering::Release);
    }

    fn snapshot(&self) -> [Option<usize>; 2] {
        [
            nonzero(self.main.load(Ordering::Acquire)),
            nonzero(self.advanced_settings.load(Ordering::Acquire)),
        ]
    }
}

fn nonzero(value: usize) -> Option<usize> {
    (value != 0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_input_extra_info_fits_32_bit_windows_pointer() {
        assert!(REMEMBER_INPUT_EXTRA_INFO <= u32::MAX as usize);
    }

    #[test]
    fn own_window_handles_update_without_locking_the_hook() {
        let handles = OwnWindowHandles::default();
        assert_eq!(handles.snapshot(), [None, None]);

        handles.set_main(0x11);
        handles.set_advanced_settings(0x22);
        assert_eq!(handles.snapshot(), [Some(0x11), Some(0x22)]);

        handles.clear_advanced_settings();
        assert_eq!(handles.snapshot(), [Some(0x11), None]);
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
pub use capture::{
    pause_capture_events, start_capture, window_lifetime_token, CapturePauseGuard,
    InputCaptureRuntime,
};

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Default)]
pub struct InputCaptureRuntime;

#[cfg(not(target_os = "windows"))]
pub fn start_capture(
    _shared: Arc<Mutex<AppController>>,
    _app_handle: AppHandle,
    _own_windows: Arc<OwnWindowHandles>,
) -> Result<InputCaptureRuntime, String> {
    Err("Remember input capture is Windows-only".to_string())
}

#[cfg(not(target_os = "windows"))]
pub struct CapturePauseGuard;

#[cfg(not(target_os = "windows"))]
pub fn pause_capture_events() -> Result<CapturePauseGuard, String> {
    Ok(CapturePauseGuard)
}

#[cfg(not(target_os = "windows"))]
pub fn window_lifetime_token(_hwnd: usize) -> WindowLifetimeToken {
    WindowLifetimeToken::default()
}

#[cfg(target_os = "windows")]
mod capture {
    use crate::{
        app_state::{
            AppController, ControlHotkeyAction, ControlHotkeyDecision, ControlHotkeyRuntime,
        },
        clock::now_ms,
        commands,
        input::{OwnWindowHandles, WindowLifetimeToken, REMEMBER_INPUT_EXTRA_INFO},
        model::{
            ButtonState, KeyState, MouseButton, TargetWindowAvailability, WindowPointerIntent,
        },
        recorder::{
            CaptureOutcome, CaptureSurface, CapturedWindow, RawInputEvent, WindowInstanceId,
        },
        window_target::{self, WindowHandle, WindowSnapshot},
    };
    use std::{
        cell::RefCell,
        collections::{HashMap, HashSet},
        sync::{mpsc, Arc, Mutex, OnceLock, RwLock},
        thread::{self, JoinHandle},
    };
    use tauri::AppHandle;
    use windows::Win32::{
        Foundation::{BOOL, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
            WindowsAndMessaging::{
                CallNextHookEx, DispatchMessageW, EnumWindows, GetAncestor, GetForegroundWindow,
                GetMessageW, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
                UnhookWindowsHookEx, WindowFromPoint, CHILDID_SELF, EVENT_OBJECT_CREATE,
                EVENT_OBJECT_DESTROY, GA_ROOT, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, LLKHF_EXTENDED,
                MSG, MSLLHOOKSTRUCT, OBJID_WINDOW, PM_NOREMOVE, WH_KEYBOARD_LL, WH_MOUSE_LL,
                WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_KEYDOWN, WM_KEYUP,
                WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
                WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
                WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
            },
        },
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct QueuedControlHotkeyAction {
        action: ControlHotkeyAction,
        at_ms: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum WindowLifecycleKind {
        Created,
        Destroyed,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct WindowLifecycleEvent {
        hwnd: usize,
        kind: WindowLifecycleKind,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct EventWindow {
        root_hwnd: usize,
        lifetime_token: WindowLifetimeToken,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct EventWindowContext {
        pointed_root: Option<EventWindow>,
        foreground_root: Option<EventWindow>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct QueuedInputEvent {
        event: RawInputEvent,
        window_context: Option<EventWindowContext>,
    }

    impl QueuedInputEvent {
        #[cfg(test)]
        fn without_window_context(event: RawInputEvent) -> Self {
            Self {
                event,
                window_context: None,
            }
        }
    }

    enum CaptureWorkerMessage {
        Input(QueuedInputEvent),
        WindowLifecycle(WindowLifecycleEvent),
        Action(QueuedControlHotkeyAction),
        ResetHotkeyFilter,
        Pause {
            reached: mpsc::SyncSender<()>,
            resume: mpsc::Receiver<()>,
        },
        Shutdown,
    }

    enum CaptureFeedback {
        ShowUnreadable { message: String, x: i32, y: i32 },
        HideUnreadable,
        StopRecording { at_ms: u64, reason: String },
    }

    struct HookContext {
        control_hotkeys: ControlHotkeyRuntime,
        own_windows: Arc<OwnWindowHandles>,
        capture_event_tx: mpsc::Sender<CaptureWorkerMessage>,
        top_level_windows: RefCell<HashSet<usize>>,
    }

    impl HookContext {
        fn dispatch(&self, message: CaptureWorkerMessage) -> bool {
            self.capture_event_tx.send(message).is_ok()
        }
    }

    // Low-level hook callbacks must return within the system hook timeout or
    // Windows silently removes the hook. Windows invokes these low-level hooks
    // on the thread that installed them, so their immutable context can live in
    // thread-local storage. The only shared read captures two small lifetime
    // tokens; no process metadata or geometry is queried here. Input and hotkey
    // actions share one ordered queue so a start/stop hotkey remains the exact
    // capture boundary without doing disk I/O inside the hook.
    thread_local! {
        static HOOK_CONTEXT: RefCell<Option<HookContext>> = const { RefCell::new(None) };
    }

    // Only non-hook lifecycle and command code uses this sender. Hook callbacks
    // dispatch through their thread-local HookContext above.
    static CAPTURE_CONTROL_TX: Mutex<Option<mpsc::Sender<CaptureWorkerMessage>>> = Mutex::new(None);
    static WINDOW_LIFETIMES: OnceLock<RwLock<WindowLifetimeRegistry>> = OnceLock::new();

    #[derive(Default)]
    struct WindowLifetimeRegistry {
        windows: HashMap<usize, ProcessWindowLifetime>,
    }

    struct ProcessWindowLifetime {
        token: WindowLifetimeToken,
        live: bool,
    }

    impl WindowLifetimeRegistry {
        fn seed(&mut self, live_windows: &HashSet<usize>) {
            for hwnd in live_windows {
                match self.windows.get_mut(hwnd) {
                    Some(window) if !window.live => {
                        window.live = true;
                    }
                    Some(_) => {}
                    None => {
                        self.windows.insert(
                            *hwnd,
                            ProcessWindowLifetime {
                                token: WindowLifetimeToken::default(),
                                live: true,
                            },
                        );
                    }
                }
            }
        }

        fn apply_lifecycle(&mut self, event: WindowLifecycleEvent) {
            match event.kind {
                WindowLifecycleKind::Created => match self.windows.get_mut(&event.hwnd) {
                    Some(window) => window.live = true,
                    None => {
                        self.windows.insert(
                            event.hwnd,
                            ProcessWindowLifetime {
                                token: WindowLifetimeToken::default(),
                                live: true,
                            },
                        );
                    }
                },
                WindowLifecycleKind::Destroyed => match self.windows.get_mut(&event.hwnd) {
                    Some(window) if window.live => {
                        window.token = WindowLifetimeToken(window.token.0.saturating_add(1));
                        window.live = false;
                    }
                    Some(_) => {}
                    None => {
                        self.windows.insert(
                            event.hwnd,
                            ProcessWindowLifetime {
                                token: WindowLifetimeToken(1),
                                live: false,
                            },
                        );
                    }
                },
            }
        }

        fn token(&self, hwnd: usize) -> WindowLifetimeToken {
            self.windows
                .get(&hwnd)
                .map(|window| window.token)
                .unwrap_or_default()
        }
    }

    fn window_lifetimes() -> &'static RwLock<WindowLifetimeRegistry> {
        WINDOW_LIFETIMES.get_or_init(|| RwLock::new(WindowLifetimeRegistry::default()))
    }

    pub fn window_lifetime_token(hwnd: usize) -> WindowLifetimeToken {
        window_lifetimes()
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .token(hwnd)
    }

    fn event_window_context(
        pointed_root_hwnd: Option<usize>,
        foreground_root_hwnd: Option<usize>,
    ) -> EventWindowContext {
        let lifetimes = window_lifetimes()
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        event_window_context_with_lifetimes(pointed_root_hwnd, foreground_root_hwnd, &lifetimes)
    }

    fn event_window_context_with_lifetimes(
        pointed_root_hwnd: Option<usize>,
        foreground_root_hwnd: Option<usize>,
        lifetimes: &WindowLifetimeRegistry,
    ) -> EventWindowContext {
        let capture = |root_hwnd| EventWindow {
            root_hwnd,
            lifetime_token: lifetimes.token(root_hwnd),
        };
        EventWindowContext {
            pointed_root: pointed_root_hwnd.map(capture),
            foreground_root: foreground_root_hwnd.map(capture),
        }
    }

    fn ensure_event_window_lifetime(
        window: EventWindow,
        lifetimes: &WindowLifetimeRegistry,
    ) -> Result<(), window_target::WindowTargetError> {
        if event_window_lifetime_matches(window, lifetimes.token(window.root_hwnd)) {
            Ok(())
        } else {
            Err(window_target::WindowTargetError::WindowLifetimeChanged {
                handle: WindowHandle::from_raw(window.root_hwnd),
            })
        }
    }

    fn event_window_lifetime_matches(
        window: EventWindow,
        current_token: WindowLifetimeToken,
    ) -> bool {
        window.lifetime_token == current_token
    }

    fn ensure_event_context_lifetimes(
        context: EventWindowContext,
        lifetimes: &WindowLifetimeRegistry,
    ) -> Result<(), window_target::WindowTargetError> {
        if let Some(pointed) = context.pointed_root {
            ensure_event_window_lifetime(pointed, lifetimes)?;
        }
        if let Some(foreground) = context.foreground_root {
            ensure_event_window_lifetime(foreground, lifetimes)?;
        }
        Ok(())
    }

    fn apply_process_window_lifecycle(event: WindowLifecycleEvent) {
        window_lifetimes()
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .apply_lifecycle(event);
    }

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
        own_windows: Arc<OwnWindowHandles>,
    ) -> Result<InputCaptureRuntime, String> {
        let control_hotkey_runtime = shared
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?
            .control_hotkey_runtime();
        let (capture_tx, capture_rx) = mpsc::channel();
        set_capture_control_sender(capture_tx.clone())?;
        let hook_context = HookContext {
            control_hotkeys: control_hotkey_runtime,
            own_windows,
            capture_event_tx: capture_tx,
            top_level_windows: RefCell::new(HashSet::new()),
        };
        let capture_shared = shared.clone();
        let action_shared = shared.clone();
        let feedback_shared = shared.clone();
        let action_app = app_handle.clone();
        let feedback_app = app_handle;
        let capture_worker = thread::spawn(move || {
            run_capture_worker_with_actions(
                capture_shared,
                capture_rx,
                |queued| {
                    commands::run_control_hotkey_action(
                        action_app.clone(),
                        action_shared.clone(),
                        queued.action,
                        queued.at_ms,
                    );
                },
                |feedback| match feedback {
                    CaptureFeedback::ShowUnreadable { message, x, y } => {
                        if let Err(error) =
                            crate::capture_warning::show(&feedback_app, &message, x, y)
                        {
                            eprintln!("Remember capture warning could not show: {error}");
                        }
                    }
                    CaptureFeedback::HideUnreadable => {
                        if let Err(error) = crate::capture_warning::hide(&feedback_app) {
                            eprintln!("Remember capture warning could not hide: {error}");
                        }
                    }
                    CaptureFeedback::StopRecording { at_ms, reason } => {
                        if let Err(error) = crate::capture_warning::hide(&feedback_app) {
                            eprintln!("Remember capture warning could not hide: {error}");
                        }
                        if let Err(error) = commands::stop_recording_for_capture_issue(
                            feedback_app.clone(),
                            feedback_shared.clone(),
                            at_ms,
                            reason,
                        ) {
                            eprintln!("Remember could not stop incompatible recording: {error}");
                        }
                    }
                },
            );
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
                .map(|context| context.own_windows.snapshot())
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

    fn dispatch_capture_event(
        event: RawInputEvent,
        window_context: Option<EventWindowContext>,
    ) -> bool {
        dispatch_hook_message(CaptureWorkerMessage::Input(QueuedInputEvent {
            event,
            window_context,
        }))
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
        run_capture_worker_with_actions(shared, receiver, |_| {}, |_| {});
    }

    fn run_capture_worker_with_actions(
        shared: Arc<Mutex<AppController>>,
        receiver: mpsc::Receiver<CaptureWorkerMessage>,
        mut run_action: impl FnMut(QueuedControlHotkeyAction),
        mut report_feedback: impl FnMut(CaptureFeedback),
    ) {
        const BATCH_SIZE: usize = 256;
        let mut messages = Vec::with_capacity(BATCH_SIZE);
        let mut window_generations = WindowGenerationTracker::default();
        let mut ordered_window_lifetimes = WindowLifetimeRegistry::default();
        let mut pointer_snapshots = PointerSnapshotCache::default();
        let mut tracking_window_relative_recording = false;

        while let Ok(message) = receiver.recv() {
            messages.push(message);
            messages.extend(receiver.try_iter().take(BATCH_SIZE - 1));

            let mut controller = None;
            let mut should_shutdown = false;
            for message in messages.drain(..) {
                match message {
                    CaptureWorkerMessage::Input(queued) => {
                        let event = queued.event;
                        if controller.is_none() {
                            controller = shared.lock().ok();
                        }
                        let window_relative = controller
                            .as_ref()
                            .is_some_and(|controller| controller.is_window_relative_recording());
                        sync_window_generation_tracking(
                            window_relative,
                            &mut tracking_window_relative_recording,
                            &mut window_generations,
                            &mut pointer_snapshots,
                        );
                        if !window_relative {
                            if let Some(controller) = controller.as_mut() {
                                controller.capture_input(event);
                            }
                            continue;
                        }

                        drop(controller.take());
                        let mut surface = resolve_capture_surface(
                            queued,
                            &mut window_generations,
                            &ordered_window_lifetimes,
                            &mut pointer_snapshots,
                        );
                        if controller.is_none() {
                            controller = shared.lock().ok();
                        }
                        let mut feedback = None;
                        if let Some(locked_controller) = controller.as_mut() {
                            apply_target_availability(locked_controller, &mut surface);
                            let unreadable_pointer = unreadable_pointer_feedback(event, &surface);
                            let outcome =
                                locked_controller.capture_input_with_surface(event, surface);
                            match outcome {
                                CaptureOutcome::StopRecording(reason) => {
                                    feedback = Some(CaptureFeedback::StopRecording {
                                        at_ms: raw_event_at_ms(event),
                                        reason,
                                    });
                                }
                                CaptureOutcome::UnreadableEnded => {
                                    feedback = Some(CaptureFeedback::HideUnreadable);
                                }
                                CaptureOutcome::UnreadableStarted(_) | CaptureOutcome::Continue => {
                                    if let Some((message, x, y)) = unreadable_pointer {
                                        feedback =
                                            Some(CaptureFeedback::ShowUnreadable { message, x, y });
                                    }
                                }
                            }
                        }
                        let stopped_automatically =
                            matches!(feedback, Some(CaptureFeedback::StopRecording { .. }));
                        if stopped_automatically {
                            drop(controller.take());
                        }
                        if let Some(feedback) = feedback {
                            report_feedback(feedback);
                        }
                        if stopped_automatically {
                            end_window_generation_tracking(
                                &mut tracking_window_relative_recording,
                                &mut pointer_snapshots,
                            );
                        }
                    }
                    CaptureWorkerMessage::WindowLifecycle(event) => {
                        ordered_window_lifetimes.apply_lifecycle(event);
                        if controller.is_none() {
                            controller = shared.lock().ok();
                        }
                        let window_relative = controller
                            .as_ref()
                            .is_some_and(|controller| controller.is_window_relative_recording());
                        sync_window_generation_tracking(
                            window_relative,
                            &mut tracking_window_relative_recording,
                            &mut window_generations,
                            &mut pointer_snapshots,
                        );
                        window_generations.apply_lifecycle(event);
                        pointer_snapshots.apply_lifecycle(event);
                    }
                    CaptureWorkerMessage::Action(action) => {
                        drop(controller.take());
                        run_action(action);
                        controller = shared.lock().ok();
                        let window_relative = controller
                            .as_ref()
                            .is_some_and(|controller| controller.is_window_relative_recording());
                        sync_window_generation_tracking(
                            window_relative,
                            &mut tracking_window_relative_recording,
                            &mut window_generations,
                            &mut pointer_snapshots,
                        );
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
                        controller = shared.lock().ok();
                        let window_relative = controller
                            .as_ref()
                            .is_some_and(|controller| controller.is_window_relative_recording());
                        sync_window_generation_tracking(
                            window_relative,
                            &mut tracking_window_relative_recording,
                            &mut window_generations,
                            &mut pointer_snapshots,
                        );
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

    fn sync_window_generation_tracking(
        window_relative: bool,
        tracking_window_relative: &mut bool,
        generations: &mut WindowGenerationTracker,
        pointer_snapshots: &mut PointerSnapshotCache,
    ) {
        if window_relative && !*tracking_window_relative {
            generations.begin_recording();
            pointer_snapshots.clear();
        }
        *tracking_window_relative = window_relative;
    }

    fn end_window_generation_tracking(
        tracking_window_relative: &mut bool,
        pointer_snapshots: &mut PointerSnapshotCache,
    ) {
        *tracking_window_relative = false;
        pointer_snapshots.clear();
    }

    const POINTER_IDENTITY_REFRESH_INTERVAL_MS: u64 = 16;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum PointerSnapshotRefresh {
        FullIdentity,
        GeometryOnly,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct PointerSnapshotKey {
        event_window: EventWindow,
        normalized_handle: WindowHandle,
    }

    fn pointer_snapshot_refresh(
        cached_key: Option<PointerSnapshotKey>,
        current_key: PointerSnapshotKey,
        last_full_snapshot_at_ms: Option<u64>,
        at_ms: u64,
        force_full_identity: bool,
    ) -> PointerSnapshotRefresh {
        if force_full_identity || cached_key != Some(current_key) {
            return PointerSnapshotRefresh::FullIdentity;
        }
        let Some(last_full_snapshot_at_ms) = last_full_snapshot_at_ms else {
            return PointerSnapshotRefresh::FullIdentity;
        };
        match at_ms.checked_sub(last_full_snapshot_at_ms) {
            Some(elapsed) if elapsed < POINTER_IDENTITY_REFRESH_INTERVAL_MS => {
                PointerSnapshotRefresh::GeometryOnly
            }
            _ => PointerSnapshotRefresh::FullIdentity,
        }
    }

    #[derive(Default)]
    struct PointerSnapshotCache {
        cached: Option<CachedPointerSnapshot>,
    }

    struct CachedPointerSnapshot {
        key: PointerSnapshotKey,
        snapshot: WindowSnapshot,
        last_full_snapshot_at_ms: u64,
    }

    impl PointerSnapshotCache {
        fn clear(&mut self) {
            self.cached = None;
        }

        fn apply_lifecycle(&mut self, event: WindowLifecycleEvent) {
            if self.cached.as_ref().is_some_and(|cached| {
                cached.key.event_window.root_hwnd == event.hwnd
                    || cached.key.normalized_handle.raw() == event.hwnd
                    || cached.snapshot.handle.raw() == event.hwnd
            }) {
                self.clear();
            }
        }

        fn resolve(
            &mut self,
            window_context: EventWindowContext,
            key: PointerSnapshotKey,
            at_ms: u64,
            force_full_identity: bool,
            lifetimes: &WindowLifetimeRegistry,
        ) -> Result<Option<WindowSnapshot>, window_target::WindowTargetError> {
            ensure_event_context_lifetimes(window_context, lifetimes)?;
            let refresh = pointer_snapshot_refresh(
                self.cached.as_ref().map(|cached| cached.key),
                key,
                self.cached
                    .as_ref()
                    .map(|cached| cached.last_full_snapshot_at_ms),
                at_ms,
                force_full_identity,
            );

            match refresh {
                PointerSnapshotRefresh::FullIdentity => {
                    let result = window_target::snapshot_pointed_window(key.normalized_handle);
                    if let Err(error) = ensure_event_context_lifetimes(window_context, lifetimes) {
                        self.clear();
                        return Err(error);
                    }
                    match result {
                        Ok(Some(snapshot)) => {
                            self.cached = Some(CachedPointerSnapshot {
                                key,
                                snapshot: snapshot.clone(),
                                last_full_snapshot_at_ms: at_ms,
                            });
                            Ok(Some(snapshot))
                        }
                        Ok(None) => {
                            self.clear();
                            Ok(None)
                        }
                        Err(error) => {
                            self.clear();
                            Err(error)
                        }
                    }
                }
                PointerSnapshotRefresh::GeometryOnly => {
                    let handle = self
                        .cached
                        .as_ref()
                        .expect("geometry refresh requires a matching cached snapshot")
                        .snapshot
                        .handle;
                    let result = window_target::refresh_window_geometry(handle);
                    if let Err(error) = ensure_event_context_lifetimes(window_context, lifetimes) {
                        self.clear();
                        return Err(error);
                    }
                    match result {
                        Ok(geometry) => {
                            let cached = self
                                .cached
                                .as_mut()
                                .expect("validated geometry refresh must keep its cache entry");
                            cached.snapshot.client_origin = geometry.client_origin;
                            cached.snapshot.client_size = geometry.client_size;
                            cached.snapshot.dpi = geometry.dpi;
                            Ok(Some(cached.snapshot.clone()))
                        }
                        Err(error) => {
                            self.clear();
                            Err(error)
                        }
                    }
                }
            }
        }
    }

    #[derive(Default)]
    struct WindowGenerationTracker {
        windows: HashMap<usize, TrackedWindowLifetime>,
    }

    struct TrackedWindowLifetime {
        identity: Option<ObservedWindowIdentity>,
        generation: u64,
        live: bool,
    }

    #[derive(PartialEq, Eq)]
    struct ObservedWindowIdentity {
        process_id: u32,
        executable_path: String,
        window_class: String,
    }

    impl WindowGenerationTracker {
        fn begin_recording(&mut self) {
            self.windows.retain(|_, window| window.live);
            for window in self.windows.values_mut() {
                window.generation = 0;
            }
        }

        fn apply_lifecycle(&mut self, event: WindowLifecycleEvent) {
            match event.kind {
                WindowLifecycleKind::Created => self.created(event.hwnd),
                WindowLifecycleKind::Destroyed => self.destroyed(event.hwnd),
            }
        }

        fn created(&mut self, hwnd: usize) {
            match self.windows.get_mut(&hwnd) {
                Some(window) if window.live => {
                    window.generation = window.generation.saturating_add(1);
                    window.identity = None;
                }
                Some(window) => {
                    window.live = true;
                    window.identity = None;
                }
                None => {
                    self.windows.insert(
                        hwnd,
                        TrackedWindowLifetime {
                            identity: None,
                            generation: 0,
                            live: true,
                        },
                    );
                }
            }
        }

        fn destroyed(&mut self, hwnd: usize) {
            match self.windows.get_mut(&hwnd) {
                Some(window) if window.live => {
                    window.generation = window.generation.saturating_add(1);
                    window.live = false;
                    window.identity = None;
                }
                Some(_) => {}
                None => {
                    self.windows.insert(
                        hwnd,
                        TrackedWindowLifetime {
                            identity: None,
                            generation: 1,
                            live: false,
                        },
                    );
                }
            }
        }

        fn observe(&mut self, window: &WindowSnapshot) -> WindowInstanceId {
            let hwnd = window.handle.raw();
            let identity = ObservedWindowIdentity {
                process_id: window.process_id,
                executable_path: window.executable_path.clone(),
                window_class: window.window_class.clone(),
            };
            let tracked = self
                .windows
                .entry(hwnd)
                .or_insert_with(|| TrackedWindowLifetime {
                    identity: None,
                    generation: 0,
                    live: true,
                });

            if !tracked.live {
                tracked.live = true;
            } else if tracked
                .identity
                .as_ref()
                .is_some_and(|previous| previous != &identity)
            {
                // Identity remains a fallback for a lifecycle notification that Windows
                // could not deliver. Lifecycle events are authoritative when available.
                tracked.generation = tracked.generation.saturating_add(1);
            }
            tracked.identity = Some(identity);
            WindowInstanceId {
                hwnd,
                generation: tracked.generation,
            }
        }
    }

    fn resolve_capture_surface(
        queued: QueuedInputEvent,
        generations: &mut WindowGenerationTracker,
        lifetimes: &WindowLifetimeRegistry,
        pointer_snapshots: &mut PointerSnapshotCache,
    ) -> CaptureSurface {
        let event = queued.event;
        match event {
            RawInputEvent::MouseMove { x, y, .. }
            | RawInputEvent::MouseButton { x, y, .. }
            | RawInputEvent::MouseWheel { x, y, .. } => {
                let window_context = queued.window_context.unwrap_or_else(|| {
                    event_window_context_with_lifetimes(
                        root_window_from_point(x, y),
                        foreground_root_window(),
                        lifetimes,
                    )
                });
                resolve_pointer_surface(
                    event,
                    window_context,
                    generations,
                    lifetimes,
                    pointer_snapshots,
                )
            }
            RawInputEvent::Key { .. } => {
                let foreground_root =
                    event_foreground_root(queued.window_context, foreground_root_window, lifetimes);
                let Some(foreground_root) = foreground_root else {
                    return CaptureSurface::Screen;
                };
                if let Err(error) = ensure_event_window_lifetime(foreground_root, lifetimes) {
                    return unreadable_surface(error);
                }
                let result = window_target::snapshot_foreground_window(WindowHandle::from_raw(
                    foreground_root.root_hwnd,
                ));
                if let Err(error) = ensure_event_window_lifetime(foreground_root, lifetimes) {
                    return unreadable_surface(error);
                }
                match result {
                    Ok(Some(window)) => CaptureSurface::Window {
                        window: captured_window(&window, generations, true),
                        intent: WindowPointerIntent::Foreground,
                    },
                    Ok(None) => CaptureSurface::Screen,
                    Err(error) => unreadable_surface(error),
                }
            }
        }
    }

    fn event_foreground_root(
        window_context: Option<EventWindowContext>,
        query_current: impl FnOnce() -> Option<usize>,
        lifetimes: &WindowLifetimeRegistry,
    ) -> Option<EventWindow> {
        match window_context {
            Some(context) => context.foreground_root,
            None => {
                event_window_context_with_lifetimes(None, query_current(), lifetimes)
                    .foreground_root
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct NormalizedWindowContext {
        pointed: Option<WindowHandle>,
        foreground: Option<WindowHandle>,
    }

    fn normalize_event_window_context(
        context: EventWindowContext,
        lifetimes: &WindowLifetimeRegistry,
    ) -> Result<NormalizedWindowContext, window_target::WindowTargetError> {
        ensure_event_context_lifetimes(context, lifetimes)?;
        let normalized =
            normalize_event_window_handles(context, window_target::normalize_window_handle)?;
        ensure_event_context_lifetimes(context, lifetimes)?;
        Ok(normalized)
    }

    fn normalize_event_window_handles(
        context: EventWindowContext,
        mut normalize: impl FnMut(
            WindowHandle,
        )
            -> Result<Option<WindowHandle>, window_target::WindowTargetError>,
    ) -> Result<NormalizedWindowContext, window_target::WindowTargetError> {
        let pointed = context
            .pointed_root
            .map(|window| normalize(WindowHandle::from_raw(window.root_hwnd)))
            .transpose()?
            .flatten();
        let foreground = context
            .foreground_root
            .map(|window| normalize(WindowHandle::from_raw(window.root_hwnd)))
            .transpose()?
            .flatten();
        Ok(NormalizedWindowContext {
            pointed,
            foreground,
        })
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum PointerSurfaceDecision {
        Screen,
        Window(WindowPointerIntent),
    }

    fn pointer_surface_decision(
        event: RawInputEvent,
        window_context: NormalizedWindowContext,
    ) -> PointerSurfaceDecision {
        let is_foreground =
            window_context.pointed.is_some() && window_context.pointed == window_context.foreground;
        match event {
            RawInputEvent::MouseMove { .. } if !is_foreground => PointerSurfaceDecision::Screen,
            RawInputEvent::MouseMove { .. } => {
                PointerSurfaceDecision::Window(WindowPointerIntent::Foreground)
            }
            RawInputEvent::MouseButton {
                state: ButtonState::Pressed,
                ..
            } if !is_foreground => {
                PointerSurfaceDecision::Window(WindowPointerIntent::ActivationClick)
            }
            RawInputEvent::MouseButton {
                state: ButtonState::Released,
                ..
            } if !is_foreground => PointerSurfaceDecision::Window(WindowPointerIntent::DropRelease),
            RawInputEvent::MouseButton { .. } => {
                PointerSurfaceDecision::Window(WindowPointerIntent::Foreground)
            }
            RawInputEvent::MouseWheel { .. } if !is_foreground => {
                PointerSurfaceDecision::Window(WindowPointerIntent::BackgroundWheel)
            }
            RawInputEvent::MouseWheel { .. } => {
                PointerSurfaceDecision::Window(WindowPointerIntent::Foreground)
            }
            RawInputEvent::Key { .. } => unreachable!("pointer surface received a key event"),
        }
    }

    fn resolve_pointer_surface(
        event: RawInputEvent,
        window_context: EventWindowContext,
        generations: &mut WindowGenerationTracker,
        lifetimes: &WindowLifetimeRegistry,
        pointer_snapshots: &mut PointerSnapshotCache,
    ) -> CaptureSurface {
        let normalized_context = match normalize_event_window_context(window_context, lifetimes) {
            Ok(context) => context,
            Err(error) => return unreadable_surface(error),
        };
        let decision = pointer_surface_decision(event, normalized_context);
        let PointerSurfaceDecision::Window(intent) = decision else {
            return CaptureSurface::Screen;
        };
        let (Some(pointed_root), Some(normalized_handle)) =
            (window_context.pointed_root, normalized_context.pointed)
        else {
            return CaptureSurface::Screen;
        };
        let event_window = [window_context.pointed_root, window_context.foreground_root]
            .into_iter()
            .flatten()
            .find(|window| window.root_hwnd == normalized_handle.raw())
            .unwrap_or(pointed_root);
        let force_full_identity = !matches!(event, RawInputEvent::MouseMove { .. });
        let pointed_window = match pointer_snapshots.resolve(
            window_context,
            PointerSnapshotKey {
                event_window,
                normalized_handle,
            },
            raw_event_at_ms(event),
            force_full_identity,
            lifetimes,
        ) {
            Ok(Some(window)) => window,
            Ok(None) => return CaptureSurface::Screen,
            Err(error) => return unreadable_surface(error),
        };

        CaptureSurface::Window {
            window: captured_window(
                &pointed_window,
                generations,
                pointed_root.root_hwnd == normalized_handle.raw(),
            ),
            intent,
        }
    }

    fn captured_window(
        snapshot: &WindowSnapshot,
        generations: &mut WindowGenerationTracker,
        direct_pointer_target: bool,
    ) -> CapturedWindow {
        CapturedWindow {
            instance: generations.observe(snapshot),
            executable_path: snapshot.executable_path.clone(),
            window_class: snapshot.window_class.clone(),
            title: snapshot.title.clone(),
            client_origin_x: snapshot.client_origin.x,
            client_origin_y: snapshot.client_origin.y,
            client_size: snapshot.client_size,
            dpi: snapshot.dpi,
            availability: TargetWindowAvailability::Deferred,
            direct_pointer_target,
        }
    }

    fn apply_target_availability(controller: &AppController, surface: &mut CaptureSurface) {
        if let CaptureSurface::Window { window, .. } = surface {
            window.availability = controller.target_availability(window.instance);
        }
    }

    fn unreadable_surface(error: impl std::fmt::Display) -> CaptureSurface {
        CaptureSurface::Unreadable(format!("该窗口无法读取：{error}"))
    }

    fn unreadable_pointer_feedback(
        event: RawInputEvent,
        surface: &CaptureSurface,
    ) -> Option<(String, i32, i32)> {
        let CaptureSurface::Unreadable(message) = surface else {
            return None;
        };
        match event {
            RawInputEvent::MouseMove { x, y, .. }
            | RawInputEvent::MouseButton { x, y, .. }
            | RawInputEvent::MouseWheel { x, y, .. } => Some((message.clone(), x, y)),
            RawInputEvent::Key { .. } => None,
        }
    }

    fn raw_event_at_ms(event: RawInputEvent) -> u64 {
        match event {
            RawInputEvent::MouseMove { at_ms, .. }
            | RawInputEvent::MouseButton { at_ms, .. }
            | RawInputEvent::MouseWheel { at_ms, .. }
            | RawInputEvent::Key { at_ms, .. } => at_ms,
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
            Ok(hooks) => hooks,
            Err(error) => {
                let _ = installed_tx.send(Err(error));
                return;
            }
        };
        if let Err(error) = seed_top_level_windows() {
            hooks.unhook();
            let _ = installed_tx.send(Err(error));
            return;
        }
        let thread_id = unsafe { GetCurrentThreadId() };
        let _ = installed_tx.send(Ok(thread_id));

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
        window_lifecycle: HWINEVENTHOOK,
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

            let window_lifecycle = unsafe {
                SetWinEventHook(
                    EVENT_OBJECT_CREATE,
                    EVENT_OBJECT_DESTROY,
                    HMODULE::default(),
                    Some(window_lifecycle_hook_proc),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                )
            };
            if window_lifecycle.is_invalid() {
                let error = windows::core::Error::from_win32();
                unsafe {
                    let _ = UnhookWindowsHookEx(mouse);
                    let _ = UnhookWindowsHookEx(keyboard);
                }
                return Err(format!(
                    "SetWinEventHook window lifecycle hook failed: {error}"
                ));
            }

            Ok(Self {
                mouse,
                keyboard,
                window_lifecycle,
            })
        }

        fn unhook(self) {
            unsafe {
                let _ = UnhookWinEvent(self.window_lifecycle);
                let _ = UnhookWindowsHookEx(self.mouse);
                let _ = UnhookWindowsHookEx(self.keyboard);
            }
        }
    }

    fn seed_top_level_windows() -> Result<(), String> {
        let mut windows = HashSet::new();
        unsafe {
            EnumWindows(
                Some(collect_top_level_window),
                LPARAM((&mut windows as *mut HashSet<usize>) as isize),
            )
        }
        .map_err(|error| format!("EnumWindows lifecycle seed failed: {error}"))?;
        window_lifetimes()
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .seed(&windows);
        HOOK_CONTEXT.with(|current| {
            if let Some(context) = current.borrow().as_ref() {
                *context.top_level_windows.borrow_mut() = windows;
            }
        });
        Ok(())
    }

    unsafe extern "system" fn collect_top_level_window(hwnd: HWND, context: LPARAM) -> BOOL {
        let windows = (context.0 as *mut HashSet<usize>).as_mut();
        if let Some(windows) = windows {
            windows.insert(hwnd.0 as usize);
        }
        BOOL(1)
    }

    fn lifecycle_kind(event: u32, id_object: i32, id_child: i32) -> Option<WindowLifecycleKind> {
        if id_object != OBJID_WINDOW.0 || id_child != CHILDID_SELF as i32 {
            return None;
        }
        match event {
            EVENT_OBJECT_CREATE => Some(WindowLifecycleKind::Created),
            EVENT_OBJECT_DESTROY => Some(WindowLifecycleKind::Destroyed),
            _ => None,
        }
    }

    fn accept_top_level_lifecycle(
        kind: WindowLifecycleKind,
        hwnd: usize,
        is_top_level_create: bool,
        top_level_windows: &mut HashSet<usize>,
    ) -> Option<WindowLifecycleEvent> {
        if hwnd == 0 {
            return None;
        }
        let accepted = match kind {
            WindowLifecycleKind::Created => is_top_level_create && top_level_windows.insert(hwnd),
            WindowLifecycleKind::Destroyed => top_level_windows.remove(&hwnd),
        };
        accepted.then_some(WindowLifecycleEvent { hwnd, kind })
    }

    unsafe extern "system" fn window_lifecycle_hook_proc(
        _hook: HWINEVENTHOOK,
        event: u32,
        hwnd: HWND,
        id_object: i32,
        id_child: i32,
        _event_thread: u32,
        _event_time_ms: u32,
    ) {
        let Some(kind) = lifecycle_kind(event, id_object, id_child) else {
            return;
        };
        // EVENT_OBJECT_DESTROY is delivered after the object is no longer queryable,
        // so destruction is checked against the top-level set captured while alive.
        let is_top_level_create =
            kind == WindowLifecycleKind::Created && GetAncestor(hwnd, GA_ROOT) == hwnd;
        let lifecycle = HOOK_CONTEXT.with(|current| {
            let current = current.borrow();
            let context = current.as_ref()?;
            let mut top_level_windows = context.top_level_windows.borrow_mut();
            accept_top_level_lifecycle(
                kind,
                hwnd.0 as usize,
                is_top_level_create,
                &mut top_level_windows,
            )
        });
        if let Some(lifecycle) = lifecycle {
            apply_process_window_lifecycle(lifecycle);
            let _ = dispatch_hook_message(CaptureWorkerMessage::WindowLifecycle(lifecycle));
        }
    }

    unsafe extern "system" fn mouse_hook_proc(
        code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if code == HC_ACTION as i32 {
            if let Some((event, pointed_root_hwnd)) = mouse_event(w_param, l_param) {
                let foreground_root_hwnd = foreground_root_window();
                capture(
                    event,
                    Some(event_window_context(
                        pointed_root_hwnd,
                        foreground_root_hwnd,
                    )),
                );
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
            let window_context = event_window_context(None, foreground_root_hwnd);
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
                capture(event, Some(window_context));
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

    fn capture(event: RawInputEvent, window_context: Option<EventWindowContext>) {
        let _ = dispatch_capture_event(event, window_context);
    }

    fn mouse_event(w_param: WPARAM, l_param: LPARAM) -> Option<(RawInputEvent, Option<usize>)> {
        let info = unsafe { (l_param.0 as *const MSLLHOOKSTRUCT).as_ref()? };
        if info.dwExtraInfo == REMEMBER_INPUT_EXTRA_INFO {
            return None;
        }

        let at_ms = now_ms();
        let x = info.pt.x;
        let y = info.pt.y;
        let pointed_root_hwnd = root_window_from_point(x, y);
        if same_root_window(pointed_root_hwnd, current_own_window_hwnds()) {
            return None;
        }

        let event = match w_param.0 as u32 {
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
        }?;
        Some((event, pointed_root_hwnd))
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
        use crate::{
            model::{ClientSize, MacroStep},
            window_target::{ScreenPoint, WindowHandle},
        };
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

        fn input_message(event: RawInputEvent) -> CaptureWorkerMessage {
            CaptureWorkerMessage::Input(QueuedInputEvent::without_window_context(event))
        }

        fn snapshot(
            hwnd: usize,
            process_id: u32,
            executable_path: &str,
            window_class: &str,
        ) -> WindowSnapshot {
            WindowSnapshot {
                handle: WindowHandle::from_raw(hwnd),
                executable_path: executable_path.to_string(),
                window_class: window_class.to_string(),
                title: "Window".to_string(),
                client_origin: ScreenPoint { x: 10, y: 20 },
                client_size: ClientSize {
                    width: 800,
                    height: 600,
                },
                dpi: 96,
                process_id,
                visible: true,
                minimized: false,
            }
        }

        fn event_window(hwnd: usize) -> EventWindow {
            EventWindow {
                root_hwnd: hwnd,
                lifetime_token: WindowLifetimeToken::default(),
            }
        }

        fn pointer_snapshot_key(hwnd: usize) -> PointerSnapshotKey {
            PointerSnapshotKey {
                event_window: event_window(hwnd),
                normalized_handle: WindowHandle::from_raw(hwnd),
            }
        }

        #[test]
        fn process_window_lifetime_token_changes_once_per_destroy_recreate_cycle() {
            let mut registry = WindowLifetimeRegistry::default();
            registry.seed(&HashSet::from([10]));
            assert_eq!(registry.token(10).value(), 0);

            registry.apply_lifecycle(WindowLifecycleEvent {
                hwnd: 10,
                kind: WindowLifecycleKind::Destroyed,
            });
            assert_eq!(registry.token(10).value(), 1);
            registry.apply_lifecycle(WindowLifecycleEvent {
                hwnd: 10,
                kind: WindowLifecycleKind::Destroyed,
            });
            assert_eq!(
                registry.token(10).value(),
                1,
                "duplicate destroy is ignored"
            );

            registry.apply_lifecycle(WindowLifecycleEvent {
                hwnd: 10,
                kind: WindowLifecycleKind::Created,
            });
            assert_eq!(registry.token(10).value(), 1);
            registry.seed(&HashSet::from([10]));
            assert_eq!(
                registry.token(10).value(),
                1,
                "reseeding capture must not reset the process lifetime token"
            );
        }

        #[test]
        fn event_window_rejects_a_reused_hwnd_token() {
            let captured = EventWindow {
                root_hwnd: 10,
                lifetime_token: WindowLifetimeToken(4),
            };

            assert!(event_window_lifetime_matches(
                captured,
                WindowLifetimeToken(4)
            ));
            assert!(!event_window_lifetime_matches(
                captured,
                WindowLifetimeToken(5)
            ));
        }

        #[test]
        fn queued_input_is_validated_before_a_later_window_destroy() {
            let mut ordered_lifetimes = WindowLifetimeRegistry::default();
            ordered_lifetimes.seed(&HashSet::from([10]));
            let captured = event_window(10);

            assert_eq!(
                ensure_event_window_lifetime(captured, &ordered_lifetimes),
                Ok(()),
                "an input message ahead of destroy in the queue must remain valid"
            );

            ordered_lifetimes.apply_lifecycle(WindowLifecycleEvent {
                hwnd: 10,
                kind: WindowLifecycleKind::Destroyed,
            });
            assert!(matches!(
                ensure_event_window_lifetime(captured, &ordered_lifetimes),
                Err(window_target::WindowTargetError::WindowLifetimeChanged { .. })
            ));
        }

        #[test]
        fn pointer_snapshot_cache_key_includes_event_time_lifetime() {
            let original = PointerSnapshotKey {
                event_window: EventWindow {
                    root_hwnd: 10,
                    lifetime_token: WindowLifetimeToken(4),
                },
                normalized_handle: WindowHandle::from_raw(10),
            };
            let reused = PointerSnapshotKey {
                event_window: EventWindow {
                    root_hwnd: 10,
                    lifetime_token: WindowLifetimeToken(5),
                },
                normalized_handle: WindowHandle::from_raw(10),
            };

            assert_eq!(
                pointer_snapshot_refresh(Some(original), reused, Some(100), 101, false),
                PointerSnapshotRefresh::FullIdentity
            );
        }

        #[test]
        fn automatic_stop_forces_the_next_v2_session_to_rebase_without_idle_input() {
            let mut generations = WindowGenerationTracker::default();
            let window = snapshot(10, 42, r"C:\Apps\same.exe", "SameWindow");
            generations.created(10);
            generations.begin_recording();
            generations.destroyed(10);
            generations.created(10);
            assert_eq!(generations.observe(&window).generation, 1);

            let mut tracking_window_relative = true;
            let mut pointer_snapshots = PointerSnapshotCache {
                cached: Some(CachedPointerSnapshot {
                    key: pointer_snapshot_key(10),
                    snapshot: window.clone(),
                    last_full_snapshot_at_ms: 10,
                }),
            };
            end_window_generation_tracking(&mut tracking_window_relative, &mut pointer_snapshots);
            assert!(!tracking_window_relative);
            assert!(pointer_snapshots.cached.is_none());

            // This models the very next queued message being the start hotkey action:
            // there was no intervening idle Input to observe a false mode.
            sync_window_generation_tracking(
                true,
                &mut tracking_window_relative,
                &mut generations,
                &mut pointer_snapshots,
            );
            assert!(tracking_window_relative);
            assert_eq!(generations.observe(&window).generation, 0);
        }

        #[test]
        fn stable_pointer_identity_snapshot_is_capped_at_the_sampling_rate() {
            let mut cached_key = None;
            let mut last_full = None;
            let mut full_snapshots = 0;
            let key = pointer_snapshot_key(10);

            for at_ms in 0..1_000 {
                if pointer_snapshot_refresh(cached_key, key, last_full, at_ms, false)
                    == PointerSnapshotRefresh::FullIdentity
                {
                    full_snapshots += 1;
                    cached_key = Some(key);
                    last_full = Some(at_ms);
                }
            }

            assert_eq!(full_snapshots, 63);
        }

        #[test]
        fn pointer_snapshot_refresh_is_immediate_for_edges_and_non_move_events() {
            let first = pointer_snapshot_key(10);
            let second = pointer_snapshot_key(20);
            assert_eq!(
                pointer_snapshot_refresh(Some(first), second, Some(100), 101, false),
                PointerSnapshotRefresh::FullIdentity
            );
            assert_eq!(
                pointer_snapshot_refresh(Some(first), first, Some(100), 101, true),
                PointerSnapshotRefresh::FullIdentity
            );
            assert_eq!(
                pointer_snapshot_refresh(Some(first), first, Some(100), 115, false),
                PointerSnapshotRefresh::GeometryOnly
            );
            assert_eq!(
                pointer_snapshot_refresh(Some(first), first, Some(100), 116, false),
                PointerSnapshotRefresh::FullIdentity
            );
        }

        #[test]
        fn activation_intent_uses_the_hook_time_foreground_roots() {
            let event = RawInputEvent::MouseButton {
                at_ms: 10,
                x: 20,
                y: 30,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            };

            assert_eq!(
                pointer_surface_decision(
                    event,
                    NormalizedWindowContext {
                        pointed: Some(WindowHandle::from_raw(10)),
                        foreground: Some(WindowHandle::from_raw(20)),
                    }
                ),
                PointerSurfaceDecision::Window(WindowPointerIntent::ActivationClick)
            );
            assert_eq!(
                pointer_surface_decision(
                    event,
                    NormalizedWindowContext {
                        pointed: Some(WindowHandle::from_raw(10)),
                        foreground: Some(WindowHandle::from_raw(10)),
                    }
                ),
                PointerSurfaceDecision::Window(WindowPointerIntent::Foreground)
            );
        }

        #[test]
        fn owned_transient_folded_to_foreground_keeps_all_pointer_intents_foreground() {
            let popup = WindowHandle::from_raw(99);
            let owner = WindowHandle::from_raw(10);
            let folded = normalize_event_window_handles(
                EventWindowContext {
                    pointed_root: Some(event_window(popup.raw())),
                    foreground_root: Some(event_window(owner.raw())),
                },
                |handle| Ok(Some(if handle == popup { owner } else { handle })),
            )
            .expect("owned popup normalization");
            assert_eq!(folded.pointed, folded.foreground);
            let events = [
                RawInputEvent::MouseMove {
                    at_ms: 1,
                    x: 2,
                    y: 3,
                },
                RawInputEvent::MouseButton {
                    at_ms: 2,
                    x: 2,
                    y: 3,
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                },
                RawInputEvent::MouseButton {
                    at_ms: 3,
                    x: 2,
                    y: 3,
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                },
                RawInputEvent::MouseWheel {
                    at_ms: 4,
                    x: 2,
                    y: 3,
                    delta: 120,
                },
            ];

            for event in events {
                assert_eq!(
                    pointer_surface_decision(event, folded),
                    PointerSurfaceDecision::Window(WindowPointerIntent::Foreground)
                );
            }
        }

        #[test]
        fn normalized_background_pointer_intents_remain_distinct() {
            let background = NormalizedWindowContext {
                pointed: Some(WindowHandle::from_raw(10)),
                foreground: Some(WindowHandle::from_raw(20)),
            };

            assert_eq!(
                pointer_surface_decision(
                    RawInputEvent::MouseMove {
                        at_ms: 1,
                        x: 2,
                        y: 3,
                    },
                    background,
                ),
                PointerSurfaceDecision::Screen
            );
            assert_eq!(
                pointer_surface_decision(
                    RawInputEvent::MouseButton {
                        at_ms: 2,
                        x: 2,
                        y: 3,
                        button: MouseButton::Left,
                        state: ButtonState::Pressed,
                    },
                    background,
                ),
                PointerSurfaceDecision::Window(WindowPointerIntent::ActivationClick)
            );
            assert_eq!(
                pointer_surface_decision(
                    RawInputEvent::MouseButton {
                        at_ms: 3,
                        x: 2,
                        y: 3,
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                    },
                    background,
                ),
                PointerSurfaceDecision::Window(WindowPointerIntent::DropRelease)
            );
            assert_eq!(
                pointer_surface_decision(
                    RawInputEvent::MouseWheel {
                        at_ms: 4,
                        x: 2,
                        y: 3,
                        delta: 120,
                    },
                    background,
                ),
                PointerSurfaceDecision::Window(WindowPointerIntent::BackgroundWheel)
            );
        }

        #[test]
        fn keyboard_target_uses_hook_time_foreground_without_a_later_query() {
            let lifetimes = WindowLifetimeRegistry::default();
            let root = event_foreground_root(
                Some(EventWindowContext {
                    pointed_root: None,
                    foreground_root: Some(event_window(10)),
                }),
                || panic!("worker must not query foreground after the key event"),
                &lifetimes,
            );

            assert_eq!(root, Some(event_window(10)));
        }

        #[test]
        fn background_move_is_screen_relative_without_a_target_snapshot() {
            assert_eq!(
                pointer_surface_decision(
                    RawInputEvent::MouseMove {
                        at_ms: 10,
                        x: 20,
                        y: 30,
                    },
                    NormalizedWindowContext {
                        pointed: Some(WindowHandle::from_raw(10)),
                        foreground: Some(WindowHandle::from_raw(20)),
                    }
                ),
                PointerSurfaceDecision::Screen
            );
        }

        #[test]
        fn lifecycle_filter_accepts_only_window_self_events() {
            assert_eq!(
                lifecycle_kind(EVENT_OBJECT_CREATE, OBJID_WINDOW.0, CHILDID_SELF as i32),
                Some(WindowLifecycleKind::Created)
            );
            assert_eq!(
                lifecycle_kind(EVENT_OBJECT_DESTROY, OBJID_WINDOW.0, CHILDID_SELF as i32),
                Some(WindowLifecycleKind::Destroyed)
            );
            assert_eq!(
                lifecycle_kind(EVENT_OBJECT_CREATE, OBJID_WINDOW.0 + 1, CHILDID_SELF as i32),
                None
            );
            assert_eq!(lifecycle_kind(EVENT_OBJECT_CREATE, OBJID_WINDOW.0, 1), None);
            assert_eq!(
                lifecycle_kind(
                    EVENT_OBJECT_CREATE + 10,
                    OBJID_WINDOW.0,
                    CHILDID_SELF as i32
                ),
                None
            );
        }

        #[test]
        fn top_level_lifecycle_filter_uses_the_live_window_set_for_destroy_events() {
            let mut top_level_windows = HashSet::from([10]);

            assert_eq!(
                accept_top_level_lifecycle(
                    WindowLifecycleKind::Created,
                    20,
                    false,
                    &mut top_level_windows,
                ),
                None,
                "child-window creation must be ignored"
            );
            assert_eq!(
                accept_top_level_lifecycle(
                    WindowLifecycleKind::Created,
                    20,
                    true,
                    &mut top_level_windows,
                ),
                Some(WindowLifecycleEvent {
                    hwnd: 20,
                    kind: WindowLifecycleKind::Created,
                })
            );
            assert_eq!(
                accept_top_level_lifecycle(
                    WindowLifecycleKind::Created,
                    20,
                    true,
                    &mut top_level_windows,
                ),
                None,
                "duplicate create notification must not start another lifetime"
            );
            assert_eq!(
                accept_top_level_lifecycle(
                    WindowLifecycleKind::Destroyed,
                    10,
                    false,
                    &mut top_level_windows,
                ),
                Some(WindowLifecycleEvent {
                    hwnd: 10,
                    kind: WindowLifecycleKind::Destroyed,
                }),
                "a seeded top-level window remains identifiable after destruction"
            );
            assert_eq!(
                accept_top_level_lifecycle(
                    WindowLifecycleKind::Destroyed,
                    99,
                    false,
                    &mut top_level_windows,
                ),
                None,
                "unknown child-window destruction must be ignored"
            );
        }

        #[test]
        fn lifecycle_reopen_increments_generation_even_when_identity_is_identical() {
            let mut generations = WindowGenerationTracker::default();
            generations.apply_lifecycle(WindowLifecycleEvent {
                hwnd: 10,
                kind: WindowLifecycleKind::Created,
            });
            generations.begin_recording();
            let original = snapshot(10, 42, r"C:\Apps\same.exe", "SameWindow");

            assert_eq!(
                generations.observe(&original),
                WindowInstanceId {
                    hwnd: 10,
                    generation: 0,
                }
            );
            assert_eq!(generations.observe(&original).generation, 0);

            generations.apply_lifecycle(WindowLifecycleEvent {
                hwnd: 10,
                kind: WindowLifecycleKind::Destroyed,
            });
            generations.apply_lifecycle(WindowLifecycleEvent {
                hwnd: 10,
                kind: WindowLifecycleKind::Created,
            });

            assert_eq!(
                generations.observe(&original),
                WindowInstanceId {
                    hwnd: 10,
                    generation: 1,
                }
            );
        }

        #[test]
        fn generation_tracking_rebases_live_windows_for_each_recording() {
            let mut generations = WindowGenerationTracker::default();
            let window = snapshot(10, 42, r"C:\Apps\same.exe", "SameWindow");
            generations.created(10);
            let _ = generations.observe(&window);
            generations.destroyed(10);
            generations.created(10);
            assert_eq!(generations.observe(&window).generation, 1);

            generations.begin_recording();
            assert_eq!(
                generations.observe(&window).generation,
                0,
                "a live window at the new recording boundary is initial"
            );

            generations.destroyed(10);
            generations.created(10);
            assert_eq!(
                generations.observe(&window).generation,
                1,
                "the same HWND reopened during recording is deferred"
            );
        }

        #[test]
        fn identity_change_remains_a_fallback_when_lifecycle_events_are_missed() {
            let mut generations = WindowGenerationTracker::default();
            generations.begin_recording();
            let original = snapshot(10, 42, r"C:\Apps\first.exe", "FirstWindow");
            let replacement = snapshot(10, 42, r"C:\Apps\second.exe", "SecondWindow");

            assert_eq!(generations.observe(&original).generation, 0);
            assert_eq!(generations.observe(&replacement).generation, 1);
            assert_eq!(generations.observe(&replacement).generation, 1);
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
                tx.send(input_message(event)).unwrap();
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
            tx.send(input_message(RawInputEvent::MouseButton {
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
            tx.send(input_message(RawInputEvent::MouseButton {
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
            let own_windows = Arc::new(OwnWindowHandles::default());
            own_windows.set_main(0x55);
            own_windows.set_advanced_settings(0x66);
            let hook_context = HookContext {
                control_hotkeys: ControlHotkeyRuntime::default(),
                own_windows,
                capture_event_tx: tx,
                top_level_windows: RefCell::new(HashSet::new()),
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
                assert!(dispatch_hook_message(input_message(
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
                assert!(dispatch_hook_message(input_message(
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
                run_capture_worker_with_actions(
                    worker_shared,
                    rx,
                    |queued| {
                        assert_eq!(queued.action, ControlHotkeyAction::Stop);
                        let recording = action_shared
                            .lock()
                            .unwrap()
                            .stop_recording(queued.at_ms)
                            .unwrap();
                        *action_recording.lock().unwrap() = Some(recording);
                    },
                    |_| {},
                );
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

            tx.send(input_message(RawInputEvent::MouseButton {
                at_ms: 1_010,
                x: 10,
                y: 10,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            }))
            .unwrap();
            tx.send(input_message(RawInputEvent::MouseButton {
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

            tx.send(input_message(RawInputEvent::MouseButton {
                at_ms: 1_001,
                x: 0,
                y: 0,
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            }))
            .unwrap();
            for point in 1..=600 {
                tx.send(input_message(RawInputEvent::MouseMove {
                    at_ms: 1_001 + point,
                    x: point as i32,
                    y: (point * 2) as i32,
                }))
                .unwrap();
            }
            tx.send(input_message(RawInputEvent::MouseButton {
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

            tx.send(input_message(RawInputEvent::Key {
                at_ms: 1_010,
                vk_code: 0xA2,
                scan_code: 0x1D,
                extended: false,
                state: KeyState::Pressed,
            }))
            .unwrap();
            tx.send(CaptureWorkerMessage::ResetHotkeyFilter).unwrap();
            tx.send(input_message(RawInputEvent::Key {
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
        send_positioned_mouse_action(x, y, flags, mouse_data)
    }

    pub fn mouse_wheel(x: i32, y: i32, delta: i32) -> Result<(), String> {
        send_positioned_mouse_action(x, y, MOUSEEVENTF_WHEEL, delta as u32)
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

    fn send_positioned_mouse_action(
        x: i32,
        y: i32,
        action_flags: MOUSE_EVENT_FLAGS,
        mouse_data: u32,
    ) -> Result<(), String> {
        let bounds = virtual_desktop_bounds()?;
        send_inputs(&positioned_mouse_action_inputs(
            x,
            y,
            bounds,
            action_flags,
            mouse_data,
        ))
    }

    fn positioned_mouse_action_inputs(
        x: i32,
        y: i32,
        bounds: (i32, i32, i32, i32),
        action_flags: MOUSE_EVENT_FLAGS,
        mouse_data: u32,
    ) -> [INPUT; 2] {
        [
            mouse_input(positioned_mouse_input(
                x,
                y,
                bounds,
                MOUSE_EVENT_FLAGS(0),
                0,
            )),
            mouse_input(MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: mouse_data,
                dwFlags: action_flags,
                time: 0,
                dwExtraInfo: REMEMBER_INPUT_EXTRA_INFO,
            }),
        ]
    }

    fn virtual_desktop_bounds() -> Result<(i32, i32, i32, i32), String> {
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        if width <= 0 || height <= 0 {
            return Err("无法读取 Windows 虚拟桌面范围。".to_string());
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
        send_inputs(&[input])
    }

    fn send_inputs(inputs: &[INPUT]) -> Result<(), String> {
        let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
        if usize::try_from(sent).ok() == Some(inputs.len()) {
            Ok(())
        } else {
            Err(format!(
                "系统未能完整发送模拟输入（已发送 {sent}/{} 个事件）：{}",
                inputs.len(),
                windows::core::Error::from_win32()
            ))
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
        fn positioned_movement_uses_absolute_virtual_desktop_flags_and_sentinel() {
            let input =
                positioned_mouse_input(320, 240, (0, 0, 1_920, 1_080), MOUSE_EVENT_FLAGS(0), 0);

            assert_eq!(
                input.dwFlags,
                MOUSEEVENTF_MOVE
                    | MOUSEEVENTF_ABSOLUTE
                    | MOUSEEVENTF_VIRTUALDESK
                    | MOUSEEVENTF_MOVE_NOCOALESCE
            );
            assert_eq!(input.dwExtraInfo, REMEMBER_INPUT_EXTRA_INFO);
        }

        #[test]
        fn positioned_actions_are_separate_from_cursor_movement() {
            let inputs = positioned_mouse_action_inputs(
                320,
                240,
                (0, 0, 1_920, 1_080),
                MOUSEEVENTF_WHEEL,
                (-120_i32) as u32,
            );
            let movement = unsafe { inputs[0].Anonymous.mi };
            let action = unsafe { inputs[1].Anonymous.mi };
            assert!(movement.dwFlags.contains(MOUSEEVENTF_MOVE));
            assert!(!movement.dwFlags.contains(MOUSEEVENTF_WHEEL));
            assert_eq!(action.dwFlags, MOUSEEVENTF_WHEEL);
            assert_eq!(action.mouseData, (-120_i32) as u32);
            assert_eq!(action.dwExtraInfo, REMEMBER_INPUT_EXTRA_INFO);
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
