pub(crate) const ELEVATED_RESTART_ARG: &str = "--remember-elevated-restart";

pub(crate) fn is_elevated_restart_request(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> bool {
    args.into_iter().any(|arg| arg == ELEVATED_RESTART_ARG)
}

#[cfg(target_os = "windows")]
mod platform {
    use super::is_elevated_restart_request;
    use std::{
        ffi::c_void,
        thread,
        time::{Duration, Instant},
    };
    use tauri::AppHandle;
    use windows::{
        core::w,
        Win32::{
            Foundation::{
                CloseHandle, GetLastError, SetLastError, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS,
                ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
                WAIT_TIMEOUT, WIN32_ERROR,
            },
            System::Threading::{
                CreateEventW, CreateMutexW, OpenEventW, OpenMutexW, SetEvent, WaitForSingleObject,
                EVENT_MODIFY_STATE, INFINITE, SYNCHRONIZATION_ACCESS_RIGHTS,
            },
        },
    };

    const ACTIVATION_EVENT_NAME: windows::core::PCWSTR = w!("Local\\com.remember.desktop.activate");
    const INSTANCE_MUTEX_NAME: windows::core::PCWSTR = w!("Local\\com.remember.desktop.instance");
    const ELEVATED_RESTART_WAIT_MS: u32 = 15_000;
    const SECONDARY_TAKEOVER_WAIT_MS: u32 = 250;
    const OBJECT_RETRY_INTERVAL_MS: u64 = 25;
    const PRIMARY_EVENT_RETRY_COUNT: usize = 10;
    const SYNCHRONIZE_ACCESS: SYNCHRONIZATION_ACCESS_RIGHTS =
        SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000);

    struct OwnedKernelHandle(usize);

    impl OwnedKernelHandle {
        fn new(handle: HANDLE) -> Self {
            Self(handle.0 as usize)
        }

        fn get(&self) -> HANDLE {
            HANDLE(self.0 as *mut c_void)
        }
    }

    impl Drop for OwnedKernelHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.get());
            }
        }
    }

    pub(crate) struct PrimaryInstance {
        instance_mutex: OwnedKernelHandle,
        activation_event: OwnedKernelHandle,
    }

    enum MutexState {
        Primary(OwnedKernelHandle),
        Existing(OwnedKernelHandle),
        Inaccessible(String),
        Unavailable(String),
    }

    fn is_win32_error(error: &windows::core::Error, code: WIN32_ERROR) -> bool {
        error.code() == windows::core::HRESULT::from_win32(code.0)
    }

    fn locate_instance_mutex() -> MutexState {
        match unsafe { OpenMutexW(SYNCHRONIZE_ACCESS, false, INSTANCE_MUTEX_NAME) } {
            Ok(handle) => MutexState::Existing(OwnedKernelHandle::new(handle)),
            Err(error) if is_win32_error(&error, ERROR_FILE_NOT_FOUND) => {
                unsafe {
                    SetLastError(ERROR_SUCCESS);
                }
                match unsafe { CreateMutexW(None, true, INSTANCE_MUTEX_NAME) } {
                    Ok(handle) => {
                        let already_exists = unsafe { GetLastError() == ERROR_ALREADY_EXISTS };
                        let handle = OwnedKernelHandle::new(handle);
                        if already_exists {
                            MutexState::Existing(handle)
                        } else {
                            MutexState::Primary(handle)
                        }
                    }
                    Err(error) if is_win32_error(&error, ERROR_ACCESS_DENIED) => {
                        MutexState::Inaccessible(error.to_string())
                    }
                    Err(error) => MutexState::Unavailable(error.to_string()),
                }
            }
            Err(error) if is_win32_error(&error, ERROR_ACCESS_DENIED) => {
                MutexState::Inaccessible(error.to_string())
            }
            Err(error) => MutexState::Unavailable(error.to_string()),
        }
    }

    fn signal_existing_activation() {
        for attempt in 0..PRIMARY_EVENT_RETRY_COUNT {
            match unsafe { OpenEventW(EVENT_MODIFY_STATE, false, ACTIVATION_EVENT_NAME) } {
                Ok(handle) => {
                    let handle = OwnedKernelHandle::new(handle);
                    if let Err(error) = unsafe { SetEvent(handle.get()) } {
                        eprintln!("Remember could not activate the running instance: {error}");
                    }
                    return;
                }
                Err(error)
                    if is_win32_error(&error, ERROR_FILE_NOT_FOUND)
                        && attempt + 1 < PRIMARY_EVENT_RETRY_COUNT =>
                {
                    thread::sleep(Duration::from_millis(OBJECT_RETRY_INTERVAL_MS));
                }
                Err(error) => {
                    eprintln!(
                        "Remember could not open the running instance's activation event: {error}"
                    );
                    return;
                }
            }
        }
    }

    fn create_primary_activation_event() -> Option<OwnedKernelHandle> {
        for attempt in 0..PRIMARY_EVENT_RETRY_COUNT {
            match unsafe { OpenEventW(SYNCHRONIZE_ACCESS, false, ACTIVATION_EVENT_NAME) } {
                Ok(handle) => return Some(OwnedKernelHandle::new(handle)),
                Err(error) if is_win32_error(&error, ERROR_FILE_NOT_FOUND) => {
                    match unsafe { CreateEventW(None, false, false, ACTIVATION_EVENT_NAME) } {
                        Ok(handle) => return Some(OwnedKernelHandle::new(handle)),
                        Err(error)
                            if (is_win32_error(&error, ERROR_ACCESS_DENIED)
                                || is_win32_error(&error, ERROR_FILE_NOT_FOUND))
                                && attempt + 1 < PRIMARY_EVENT_RETRY_COUNT =>
                        {
                            thread::sleep(Duration::from_millis(OBJECT_RETRY_INTERVAL_MS));
                        }
                        Err(error) => {
                            eprintln!(
                                "Remember could not create its activation event; startup was cancelled: {error}"
                            );
                            return None;
                        }
                    }
                }
                Err(error)
                    if is_win32_error(&error, ERROR_ACCESS_DENIED)
                        && attempt + 1 < PRIMARY_EVENT_RETRY_COUNT =>
                {
                    thread::sleep(Duration::from_millis(OBJECT_RETRY_INTERVAL_MS));
                }
                Err(error) => {
                    eprintln!(
                        "Remember could not open its activation event; startup was cancelled: {error}"
                    );
                    return None;
                }
            }
        }

        eprintln!(
            "Remember could not obtain its activation event; startup was cancelled to preserve single-instance safety"
        );
        None
    }

    fn finish_primary(instance_mutex: OwnedKernelHandle) -> Option<PrimaryInstance> {
        create_primary_activation_event().map(|activation_event| PrimaryInstance {
            instance_mutex,
            activation_event,
        })
    }

    fn wait_for_mutex(instance_mutex: &OwnedKernelHandle, timeout_ms: u32) -> bool {
        let wait = unsafe { WaitForSingleObject(instance_mutex.get(), timeout_ms) };
        if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
            return true;
        }
        if wait != WAIT_TIMEOUT {
            let error = unsafe { GetLastError() };
            eprintln!(
                "Remember could not verify the running instance state (wait {}, error {}); this launch will exit",
                wait.0, error.0
            );
        }
        false
    }

    pub(crate) fn acquire() -> Result<Option<PrimaryInstance>, String> {
        let elevated_restart = is_elevated_restart_request(std::env::args_os());
        let elevated_deadline =
            Instant::now() + Duration::from_millis(u64::from(ELEVATED_RESTART_WAIT_MS));

        loop {
            match locate_instance_mutex() {
                MutexState::Primary(instance_mutex) => {
                    return Ok(finish_primary(instance_mutex));
                }
                MutexState::Existing(instance_mutex) => {
                    if !elevated_restart {
                        signal_existing_activation();
                    }

                    let timeout_ms = if elevated_restart {
                        let remaining = elevated_deadline.saturating_duration_since(Instant::now());
                        remaining.as_millis().min(u128::from(u32::MAX)) as u32
                    } else {
                        SECONDARY_TAKEOVER_WAIT_MS
                    };

                    if wait_for_mutex(&instance_mutex, timeout_ms) {
                        return Ok(finish_primary(instance_mutex));
                    }

                    if elevated_restart {
                        eprintln!(
                            "The previous Remember instance did not exit before administrator restart; this launch will exit"
                        );
                    }
                    return Ok(None);
                }
                MutexState::Inaccessible(error) => {
                    if !elevated_restart {
                        signal_existing_activation();
                        eprintln!(
                            "Remember could not inspect the running instance ({error}); this launch will exit"
                        );
                        return Ok(None);
                    }

                    let remaining = elevated_deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        eprintln!(
                            "Remember could not access the previous instance before administrator restart ({error}); this launch will exit"
                        );
                        return Ok(None);
                    }
                    thread::sleep(remaining.min(Duration::from_millis(OBJECT_RETRY_INTERVAL_MS)));
                }
                MutexState::Unavailable(error) => {
                    eprintln!(
                        "Remember could not establish single-instance ownership ({error}); this launch will exit"
                    );
                    return Ok(None);
                }
            }
        }
    }

    impl PrimaryInstance {
        pub(crate) fn listen_for_activation(self, app: AppHandle) -> Result<(), String> {
            let Self {
                instance_mutex,
                activation_event,
            } = self;

            thread::Builder::new()
                .name("remember-single-instance".to_string())
                .spawn(move || {
                    let _instance_mutex = instance_mutex;
                    loop {
                        let wait = unsafe { WaitForSingleObject(activation_event.get(), INFINITE) };
                        if wait != WAIT_OBJECT_0 {
                            eprintln!(
                                "Remember single-instance listener stopped unexpectedly: {}",
                                wait.0
                            );
                            break;
                        }

                        let app_for_window = app.clone();
                        if let Err(error) = app.run_on_main_thread(move || {
                            if let Err(error) = crate::tray::show_main_window(&app_for_window) {
                                eprintln!("Remember could not show its main window: {error}");
                            }
                        }) {
                            eprintln!(
                                "Remember could not schedule main-window activation: {error}"
                            );
                            break;
                        }
                    }
                })
                .map(|_| ())
                .map_err(|error| format!("single-instance listener could not start: {error}"))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::is_win32_error;
        use windows::{
            core::{Error, HRESULT},
            Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND},
        };

        #[test]
        fn identifies_only_the_requested_win32_error() {
            let access_denied = Error::from_hresult(HRESULT::from_win32(ERROR_ACCESS_DENIED.0));

            assert!(is_win32_error(&access_denied, ERROR_ACCESS_DENIED));
            assert!(!is_win32_error(&access_denied, ERROR_FILE_NOT_FOUND));
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use tauri::AppHandle;

    pub(crate) struct PrimaryInstance;

    pub(crate) fn acquire() -> Result<Option<PrimaryInstance>, String> {
        Ok(Some(PrimaryInstance))
    }

    impl PrimaryInstance {
        pub(crate) fn listen_for_activation(self, _app: AppHandle) -> Result<(), String> {
            Ok(())
        }
    }
}

pub(crate) use platform::acquire;

#[cfg(test)]
mod tests {
    use super::{is_elevated_restart_request, ELEVATED_RESTART_ARG};
    use std::ffi::OsString;

    #[test]
    fn recognizes_only_the_internal_elevated_restart_argument() {
        assert!(is_elevated_restart_request([
            OsString::from("remember.exe"),
            OsString::from(ELEVATED_RESTART_ARG),
        ]));
        assert!(!is_elevated_restart_request([
            OsString::from("remember.exe"),
            OsString::from("--unrelated"),
        ]));
    }
}
