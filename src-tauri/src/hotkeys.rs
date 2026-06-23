use crate::player::{PlaybackControlPayload, PlayerError, PlayerState};

pub const EMERGENCY_HOTKEY_LABEL: &str = "Ctrl + Alt + S";

#[derive(Debug, Clone, PartialEq)]
pub enum EmergencyHotkeyOutcome {
    Stopped(PlaybackControlPayload),
    NotPlaying,
}

pub fn trigger_emergency_stop(player: &mut PlayerState) -> EmergencyHotkeyOutcome {
    match player.emergency_stop() {
        Ok(payload) => EmergencyHotkeyOutcome::Stopped(payload),
        Err(PlayerError::NotPlaying) => EmergencyHotkeyOutcome::NotPlaying,
        Err(_) => EmergencyHotkeyOutcome::NotPlaying,
    }
}

pub use platform::GlobalHotkeyGuard;

pub fn start_emergency_hotkey<F>(on_trigger: F) -> Result<GlobalHotkeyGuard, String>
where
    F: Fn() + Send + Sync + 'static,
{
    platform::start_emergency_hotkey(std::sync::Arc::new(on_trigger))
}

#[cfg(target_os = "windows")]
mod platform {
    use std::{
        ffi::c_void,
        mem,
        ptr::null_mut,
        sync::Arc,
        thread::{self, JoinHandle},
        time::Duration,
    };

    type Handle = *mut c_void;

    const EMERGENCY_HOTKEY_ID: i32 = 0x5253;
    const MOD_ALT: u32 = 0x0001;
    const MOD_CONTROL: u32 = 0x0002;
    const MOD_NOREPEAT: u32 = 0x4000;
    const VK_S: u32 = 0x53;
    const WM_HOTKEY: u32 = 0x0312;
    const WM_QUIT: u32 = 0x0012;

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

    #[link(name = "user32")]
    extern "system" {
        fn DispatchMessageW(message: *const Msg) -> isize;
        fn GetMessageW(message: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
        fn PostThreadMessageW(thread_id: u32, message: u32, w_param: usize, l_param: isize) -> i32;
        fn RegisterHotKey(hwnd: Handle, id: i32, modifiers: u32, virtual_key: u32) -> i32;
        fn TranslateMessage(message: *const Msg) -> i32;
        fn UnregisterHotKey(hwnd: Handle, id: i32) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    #[derive(Debug)]
    pub struct GlobalHotkeyGuard {
        thread_id: u32,
        join_handle: Option<JoinHandle<()>>,
    }

    impl GlobalHotkeyGuard {
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

    impl Drop for GlobalHotkeyGuard {
        fn drop(&mut self) {
            self.stop_inner();
        }
    }

    pub fn start_emergency_hotkey(
        on_trigger: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<GlobalHotkeyGuard, String> {
        let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
        let join_handle = thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            let registered = unsafe {
                RegisterHotKey(
                    null_mut(),
                    EMERGENCY_HOTKEY_ID,
                    MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                    VK_S,
                )
            };
            if registered == 0 {
                let _ = ready_sender.send(Err(
                    "failed to register Ctrl + Alt + S emergency hotkey".to_string(),
                ));
                return;
            }

            let _ = ready_sender.send(Ok(thread_id));

            let mut message = unsafe { mem::zeroed::<Msg>() };
            loop {
                let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
                if result <= 0 {
                    break;
                }
                if message.message == WM_HOTKEY && message.w_param == EMERGENCY_HOTKEY_ID as usize {
                    on_trigger();
                    continue;
                }
                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }

            unsafe {
                UnregisterHotKey(null_mut(), EMERGENCY_HOTKEY_ID);
            }
        });

        match ready_receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(thread_id)) => Ok(GlobalHotkeyGuard {
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
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use std::sync::Arc;

    #[derive(Debug)]
    pub struct GlobalHotkeyGuard;

    impl GlobalHotkeyGuard {
        pub fn stop(self) {}
    }

    pub fn start_emergency_hotkey(
        _on_trigger: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<GlobalHotkeyGuard, String> {
        Err("global emergency hotkey is only available on Windows".to_string())
    }
}
