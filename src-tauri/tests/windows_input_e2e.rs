#![cfg(target_os = "windows")]

use remember_lib::{
    input::{SystemInputExecutor, REMEMBER_INPUT_EXTRA_INFO},
    model::KeyState,
    player::{play_recording, PlaybackSettings, StopToken},
    recorder::{RawInputEvent, Recorder},
    storage::{load_recording, save_recording},
};
use std::{
    mem::size_of,
    path::PathBuf,
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
            KEYEVENTF_KEYUP, VIRTUAL_KEY,
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, PeekMessageW, SetWindowsHookExW, TranslateMessage,
            UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, PM_REMOVE, WH_KEYBOARD_LL,
            WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};

#[derive(Debug, Clone, Copy)]
struct ObservedKey {
    vk_code: u16,
    scan_code: u16,
    state: KeyState,
    extra_info: usize,
}

static OBSERVER: OnceLock<Mutex<Option<mpsc::SyncSender<ObservedKey>>>> = OnceLock::new();

unsafe extern "system" fn keyboard_observer(
    code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32 {
        if let Some(info) = (l_param.0 as *const KBDLLHOOKSTRUCT).as_ref() {
            let state = match w_param.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => Some(KeyState::Pressed),
                WM_KEYUP | WM_SYSKEYUP => Some(KeyState::Released),
                _ => None,
            };
            if info.vkCode == 0x41 {
                if let Some(state) = state {
                    if let Ok(observer) = OBSERVER.get_or_init(Default::default).lock() {
                        if let Some(sender) = observer.as_ref() {
                            let _ = sender.try_send(ObservedKey {
                                vk_code: info.vkCode as u16,
                                scan_code: info.scanCode as u16,
                                state,
                                extra_info: info.dwExtraInfo,
                            });
                        }
                    }
                }
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, w_param, l_param)
}

struct KeyboardHook(HHOOK);

impl KeyboardHook {
    fn install() -> Result<Self, String> {
        let module = unsafe { GetModuleHandleW(None) }.map_err(|error| error.to_string())?;
        let hook = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_observer),
                HINSTANCE(module.0),
                0,
            )
        }
        .map_err(|error| error.to_string())?;
        Ok(Self(hook))
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
    }
}

fn observe_keys(
    send: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> Result<Vec<ObservedKey>, String> {
    let _hook = KeyboardHook::install()?;
    let (event_tx, event_rx) = mpsc::sync_channel(8);
    *OBSERVER
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "observer lock poisoned".to_string())? = Some(event_tx);
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    let sender = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        let _ = result_tx.send(send());
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while events.len() < 2 && Instant::now() < deadline {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() } {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        events.extend(event_rx.try_iter());
        thread::sleep(Duration::from_millis(5));
    }

    *OBSERVER
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "observer lock poisoned".to_string())? = None;
    sender
        .join()
        .map_err(|_| "input sender panicked".to_string())?;
    result_rx
        .recv()
        .map_err(|_| "input sender stopped".to_string())??;
    if events.len() != 2 {
        return Err(format!(
            "expected two keyboard hook events, observed {events:?}"
        ));
    }
    Ok(events)
}

fn send_external_a_pair() -> Result<(), String> {
    let inputs = [
        raw_key_input(KEYBD_EVENT_FLAGS(0)),
        raw_key_input(KEYEVENTF_KEYUP),
    ];
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(format!(
            "SendInput accepted {sent} of {} events",
            inputs.len()
        ))
    }
}

fn raw_key_input(flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0x41),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn temporary_recording_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "remember-windows-input-e2e-{}-{}.remember.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ))
}

#[test]
#[ignore = "sends real input; CI runs this on an isolated Windows runner"]
fn hook_record_save_load_and_playback_round_trip_real_windows_input() {
    let captured = observe_keys(send_external_a_pair).expect("capture injected user input");
    assert_eq!(captured[0].state, KeyState::Pressed);
    assert_eq!(captured[1].state, KeyState::Released);
    assert!(captured.iter().all(|event| event.extra_info == 0));

    let mut recorder = Recorder::new(16);
    recorder
        .start("windows input e2e", 1_000, "2026-08-14T00:00:00Z")
        .expect("start recorder");
    for (index, event) in captured.iter().enumerate() {
        recorder.capture(RawInputEvent::Key {
            at_ms: 1_010 + (index as u64 * 10),
            vk_code: event.vk_code,
            scan_code: event.scan_code,
            extended: false,
            state: event.state,
        });
    }
    let recording = recorder.stop(1_030).expect("stop recorder");
    let path = temporary_recording_path();
    save_recording(&path, &recording).expect("save recording");
    let loaded = load_recording(&path).expect("load recording");
    std::fs::remove_file(&path).expect("remove temporary recording");
    assert_eq!(loaded, recording);

    let replayed = observe_keys(move || {
        play_recording(
            &loaded,
            PlaybackSettings::new(Some(1), 1.0)?,
            &SystemInputExecutor,
            &StopToken::default(),
        )
    })
    .expect("observe real playback input");
    assert_eq!(replayed[0].state, KeyState::Pressed);
    assert_eq!(replayed[1].state, KeyState::Released);
    assert!(replayed
        .iter()
        .all(|event| event.extra_info == REMEMBER_INPUT_EXTRA_INFO));
}
