use crate::{
    input::{self, SystemInputExecutor, WindowLifetimeToken},
    model::{
        ButtonState, KeyState, MouseButton, TargetWindowId, WindowPointerIntent, WindowTarget,
    },
    player::{StepExecutor, StopToken},
    window_target::{self, ScreenPoint, WindowHandle, WindowSnapshot, WindowTargetError},
};
use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};
use tauri::AppHandle;

const DEFERRED_BIND_TIMEOUT: Duration = Duration::from_secs(30);
const DEFERRED_BIND_POLL_INTERVAL: Duration = Duration::from_millis(100);
const BINDING_SETTLE_DELAY: Duration = Duration::from_millis(50);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct WindowPlaybackExecutor {
    input: SystemInputExecutor,
    stop_token: StopToken,
    session: Mutex<PlaybackSession>,
}

impl WindowPlaybackExecutor {
    pub fn new(stop_token: StopToken) -> Self {
        Self {
            input: SystemInputExecutor,
            stop_token,
            session: Mutex::new(PlaybackSession::default()),
        }
    }

    pub fn with_app(_app: AppHandle, stop_token: StopToken) -> Self {
        Self::new(stop_token)
    }

    fn session(&self) -> Result<MutexGuard<'_, PlaybackSession>, String> {
        self.session
            .lock()
            .map_err(|_| "window playback session lock poisoned".to_string())
    }

    fn foreground_established(&self) -> Result<bool, String> {
        Ok(self.session()?.foreground_established)
    }

    fn mark_foreground_established(&self) -> Result<(), String> {
        self.session()?.mark_foreground_established();
        Ok(())
    }

    fn ensure_binding(&self, target: &WindowTarget) -> Result<BindingOutcome, String> {
        let mut session = self.session()?;
        if let Some(binding) = session.binding(target.id) {
            if window_target::window_is_alive(binding.handle) {
                match window_target::snapshot_window(binding.handle) {
                    Ok(snapshot) if binding.matches_snapshot(&snapshot) => {
                        return Ok(BindingOutcome {
                            binding,
                            pause_ms: 0,
                        });
                    }
                    Ok(_) | Err(WindowTargetError::WindowUnavailable { .. }) => {
                        session.remove_binding(target.id);
                    }
                    Err(error) => return Err(error.to_string()),
                }
            } else {
                session.remove_binding(target.id);
            }
        }

        bind_target_with_wait(&mut session, target, &self.stop_token)
    }

    fn prepare_pointer_target(
        &self,
        target: &WindowTarget,
        relative_x: i32,
        relative_y: i32,
        intent: WindowPointerIntent,
        _input_held: bool,
    ) -> Result<(WindowHandle, ScreenPoint, ScreenPoint, u64), String> {
        let outcome = self.ensure_binding(target)?;
        let adjustment = self.session()?.pointer_adjustment(target.id);
        let adjustment_active = intent == WindowPointerIntent::WindowAdjustment
            && adjustment.is_some_and(|active| active.handle == outcome.binding.handle);
        let snapshot = prepare_compatible_target(target, &outcome.binding, !adjustment_active)?;
        let restore_needed = !snapshot.visible || snapshot.minimized;
        let restore_started = Instant::now();
        let mut snapshot = restore_if_needed(target, &outcome.binding, snapshot)?;
        let mut pause_ms = outcome.pause_ms;
        if restore_needed {
            pause_ms = pause_ms.saturating_add(elapsed_millis(restore_started));
        }
        match foreground_preparation(
            self.foreground_established()?,
            TargetedInputKind::Pointer(intent),
        ) {
            ForegroundPreparation::None => {}
            ForegroundPreparation::Establish => {
                if !target_is_foreground(outcome.binding.handle)? {
                    let foreground_started = Instant::now();
                    snapshot = restore_foreground_target(target, &outcome.binding)?;
                    pause_ms = pause_ms.saturating_add(elapsed_millis(foreground_started));
                }
                enforce_pre_input_foreground(intent, outcome.binding.handle)?;
                self.mark_foreground_established()?;
            }
            ForegroundPreparation::Verify => {
                enforce_pre_input_foreground(intent, outcome.binding.handle)?;
            }
        }
        let client_origin = adjustment
            .filter(|active| active.handle == outcome.binding.handle)
            .map(|active| active.client_origin)
            .unwrap_or(snapshot.client_origin);
        let point = relative_screen_point(
            outcome.binding.handle,
            client_origin,
            relative_x,
            relative_y,
        )?;
        if !window_target::is_screen_point_reachable(point).map_err(|error| error.to_string())? {
            return Err(format!(
                "目标窗口 {} 的操作点 ({}, {}) 位于当前虚拟桌面之外。",
                target.id.0, point.x, point.y
            ));
        }
        if !adjustment_active {
            verify_unoccluded(target, outcome.binding.handle, point)?;
        }
        ensure_playback_running(&self.stop_token)?;
        Ok((outcome.binding.handle, point, client_origin, pause_ms))
    }

    fn prepare_keyboard_target(
        &self,
        target: &WindowTarget,
        _input_held: bool,
    ) -> Result<(WindowHandle, u64), String> {
        let outcome = self.ensure_binding(target)?;
        let snapshot = prepare_compatible_target(target, &outcome.binding, true)?;
        let restore_needed = !snapshot.visible || snapshot.minimized;
        let restore_started = Instant::now();
        let _snapshot = restore_if_needed(target, &outcome.binding, snapshot)?;
        let mut pause_ms = outcome.pause_ms;
        if restore_needed {
            pause_ms = pause_ms.saturating_add(elapsed_millis(restore_started));
        }
        match foreground_preparation(
            self.foreground_established()?,
            TargetedInputKind::KeyboardSequence,
        ) {
            ForegroundPreparation::None => unreachable!("keyboard input always needs foreground"),
            ForegroundPreparation::Establish => {
                if !target_is_foreground(outcome.binding.handle)? {
                    let foreground_started = Instant::now();
                    let _snapshot = restore_foreground_target(target, &outcome.binding)?;
                    pause_ms = pause_ms.saturating_add(elapsed_millis(foreground_started));
                }
                require_foreground(outcome.binding.handle, target)?;
                self.mark_foreground_established()?;
            }
            ForegroundPreparation::Verify => {
                require_foreground(outcome.binding.handle, target)?;
            }
        }
        ensure_playback_running(&self.stop_token)?;
        Ok((outcome.binding.handle, pause_ms))
    }
}

impl StepExecutor for WindowPlaybackExecutor {
    fn prepare_window_relative_playback(&self) -> Result<(), String> {
        let mut session = self.session()?;
        session.clear();
        drop(session);
        ensure_playback_running(&self.stop_token)
    }

    fn begin_window_relative_loop(&self, _targets: &[WindowTarget]) -> Result<(), String> {
        let mut session = self.session()?;
        session.begin_loop();
        Ok(())
    }

    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        self.input.mouse_move(x, y)
    }

    fn mouse_button(
        &self,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    ) -> Result<(), String> {
        self.input.mouse_button(x, y, button, state)
    }

    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String> {
        self.input.mouse_wheel(x, y, delta)
    }

    fn key(
        &self,
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
    ) -> Result<(), String> {
        self.input.key(vk_code, scan_code, extended, state)
    }

    fn window_mouse_move(
        &self,
        target: &WindowTarget,
        x: i32,
        y: i32,
        intent: WindowPointerIntent,
        input_held: bool,
    ) -> Result<u64, String> {
        let (_handle, point, _origin, pause_ms) =
            self.prepare_pointer_target(target, x, y, intent, input_held)?;
        self.input.mouse_move(point.x, point.y)?;
        Ok(pause_ms)
    }

    #[allow(clippy::too_many_arguments)]
    fn window_mouse_button(
        &self,
        target: &WindowTarget,
        x: i32,
        y: i32,
        intent: WindowPointerIntent,
        button: MouseButton,
        state: ButtonState,
        input_held: bool,
    ) -> Result<u64, String> {
        let (handle, point, client_origin, pause_ms) =
            self.prepare_pointer_target(target, x, y, intent, input_held)?;
        self.input.mouse_button(point.x, point.y, button, state)?;
        if intent == WindowPointerIntent::WindowAdjustment && state == ButtonState::Pressed {
            self.session()?
                .begin_pointer_adjustment(target.id, handle, client_origin);
        }
        if intent == WindowPointerIntent::WindowAdjustment && state == ButtonState::Released {
            self.session()?.finish_pointer_adjustment(target.id)?;
        }
        Ok(pause_ms)
    }

    fn window_mouse_wheel(
        &self,
        target: &WindowTarget,
        x: i32,
        y: i32,
        intent: WindowPointerIntent,
        delta: i32,
        input_held: bool,
    ) -> Result<u64, String> {
        let (_handle, point, _origin, pause_ms) =
            self.prepare_pointer_target(target, x, y, intent, input_held)?;
        self.input.mouse_wheel(point.x, point.y, delta)?;
        Ok(pause_ms)
    }

    #[allow(clippy::too_many_arguments)]
    fn targeted_key(
        &self,
        target: &WindowTarget,
        vk_code: u16,
        scan_code: u16,
        extended: bool,
        state: KeyState,
        sequence_start: bool,
        input_held: bool,
    ) -> Result<u64, String> {
        let pause_ms = if sequence_start {
            let (_handle, pause_ms) = self.prepare_keyboard_target(target, input_held)?;
            pause_ms
        } else {
            0
        };
        self.input.key(vk_code, scan_code, extended, state)?;
        Ok(pause_ms)
    }

    fn release_mouse_button(&self, button: MouseButton) -> Result<(), String> {
        self.input.release_mouse_button(button)
    }
}

#[derive(Default)]
struct PlaybackSession {
    bindings: HashMap<TargetWindowId, Binding>,
    assigned_handles: HashMap<WindowHandle, TargetWindowId>,
    foreground_established: bool,
    pointer_adjustment: Option<PointerAdjustment>,
}

#[derive(Debug, Clone, Copy)]
struct PointerAdjustment {
    target_id: TargetWindowId,
    handle: WindowHandle,
    client_origin: ScreenPoint,
}

#[derive(Debug, Clone)]
struct BoundWindow {
    handle: WindowHandle,
    lifetime_token: WindowLifetimeToken,
    executable_path: String,
    window_class: String,
    process_id: u32,
    dpi: u32,
}

#[derive(Debug, Clone)]
struct BindingOutcome {
    binding: BoundWindow,
    pause_ms: u64,
}

impl BoundWindow {
    fn from_stable_snapshot(
        snapshot: &WindowSnapshot,
        lifetime_token: WindowLifetimeToken,
    ) -> Self {
        Self {
            handle: snapshot.handle,
            lifetime_token,
            executable_path: snapshot.executable_path.clone(),
            window_class: snapshot.window_class.clone(),
            process_id: snapshot.process_id,
            dpi: snapshot.dpi,
        }
    }

    fn matches_snapshot(&self, snapshot: &WindowSnapshot) -> bool {
        self.handle == snapshot.handle
            && self.lifetime_token == input::window_lifetime_token(snapshot.handle.raw())
            && self.process_id == snapshot.process_id
            && windows_path_eq(&self.executable_path, &snapshot.executable_path)
            && self.window_class == snapshot.window_class
    }
}

type Binding = BoundWindow;

impl PlaybackSession {
    fn clear(&mut self) {
        self.bindings.clear();
        self.assigned_handles.clear();
        self.foreground_established = false;
        self.pointer_adjustment = None;
    }

    fn binding(&self, target_id: TargetWindowId) -> Option<BoundWindow> {
        self.bindings.get(&target_id).cloned()
    }

    fn bind(
        &mut self,
        target_id: TargetWindowId,
        snapshot: &WindowSnapshot,
        lifetime_token: WindowLifetimeToken,
    ) -> Result<BoundWindow, String> {
        let handle = snapshot.handle;
        if let Some(other_target) = self.assigned_handles.get(&snapshot.handle) {
            if *other_target != target_id {
                return Err(format!(
                    "当前窗口句柄 0x{:X} 已绑定到目标 {}，不能同时绑定到目标 {}。",
                    handle.raw(),
                    other_target.0,
                    target_id.0
                ));
            }
        }
        self.remove_binding(target_id);
        let binding = BoundWindow::from_stable_snapshot(snapshot, lifetime_token);
        self.bindings.insert(target_id, binding.clone());
        self.assigned_handles.insert(handle, target_id);
        Ok(binding)
    }

    fn remove_binding(&mut self, target_id: TargetWindowId) {
        if let Some(binding) = self.bindings.remove(&target_id) {
            self.assigned_handles.remove(&binding.handle);
        }
    }

    fn begin_loop(&mut self) {
        self.remove_expired_lifetimes();
        self.foreground_established = false;
    }

    fn remove_expired_lifetimes(&mut self) {
        let expired_targets = self
            .bindings
            .iter()
            .filter_map(|(target_id, binding)| {
                (!binding_has_same_live_instance(binding)).then_some(*target_id)
            })
            .collect::<Vec<_>>();
        for target_id in expired_targets {
            self.remove_binding(target_id);
        }
    }

    fn mark_foreground_established(&mut self) {
        self.foreground_established = true;
    }

    fn pointer_adjustment(&self, target_id: TargetWindowId) -> Option<PointerAdjustment> {
        self.pointer_adjustment
            .filter(|adjustment| adjustment.target_id == target_id)
    }

    fn begin_pointer_adjustment(
        &mut self,
        target_id: TargetWindowId,
        handle: WindowHandle,
        client_origin: ScreenPoint,
    ) {
        self.pointer_adjustment = Some(PointerAdjustment {
            target_id,
            handle,
            client_origin,
        });
    }

    fn finish_pointer_adjustment(&mut self, target_id: TargetWindowId) -> Result<(), String> {
        let Some(adjustment) = self.pointer_adjustment.take() else {
            return Ok(());
        };
        if adjustment.target_id != target_id {
            return Err("窗口调整手势的目标发生了变化，已停止回放。".to_string());
        }
        let snapshot =
            window_target::snapshot_window(adjustment.handle).map_err(|error| error.to_string())?;
        let binding = self
            .bindings
            .get_mut(&target_id)
            .ok_or_else(|| format!("目标窗口 {} 的绑定已失效。", target_id.0))?;
        if !binding.matches_snapshot(&snapshot) {
            return Err(format!(
                "目标窗口 {} 在窗口调整期间被关闭或替换。",
                target_id.0
            ));
        }
        if snapshot.dpi != binding.dpi {
            return Err(format!(
                "目标窗口 {} 的 DPI 在窗口调整期间从 {} 变为 {}，无法安全继续回放。",
                target_id.0, binding.dpi, snapshot.dpi
            ));
        }
        Ok(())
    }
}

fn binding_has_same_live_instance(binding: &BoundWindow) -> bool {
    window_target::window_is_alive(binding.handle)
        && binding.lifetime_token == input::window_lifetime_token(binding.handle.raw())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CandidateChoice {
    Missing,
    Unique(WindowSnapshot),
    Incompatible {
        candidate_count: usize,
        size_mismatch_count: usize,
        dpi_mismatch_count: usize,
    },
    AlreadyAssigned {
        assigned_count: usize,
    },
}

fn choose_candidate(
    target: &WindowTarget,
    candidates: &[WindowSnapshot],
    assigned_handles: &HashMap<WindowHandle, TargetWindowId>,
) -> CandidateChoice {
    let mut compatible_count = 0;
    let mut size_mismatch_count = 0;
    let mut dpi_mismatch_count = 0;

    for candidate in candidates {
        let size_matches = candidate.client_size == target.client_size;
        let dpi_matches = candidate.dpi == target.dpi;
        if !size_matches {
            size_mismatch_count += 1;
        }
        if !dpi_matches {
            dpi_mismatch_count += 1;
        }
        if !size_matches || !dpi_matches {
            continue;
        }

        compatible_count += 1;
        if !assigned_handles.contains_key(&candidate.handle) {
            return CandidateChoice::Unique(candidate.clone());
        }
    }

    if compatible_count > 0 {
        CandidateChoice::AlreadyAssigned {
            assigned_count: compatible_count,
        }
    } else if candidates.is_empty() {
        CandidateChoice::Missing
    } else {
        CandidateChoice::Incompatible {
            candidate_count: candidates.len(),
            size_mismatch_count,
            dpi_mismatch_count,
        }
    }
}

fn discover_candidate_choice(
    target: &WindowTarget,
    assigned_handles: &HashMap<WindowHandle, TargetWindowId>,
) -> Result<CandidateChoice, String> {
    let exact =
        window_target::enumerate_matching_windows(target).map_err(|error| error.to_string())?;
    Ok(choose_candidate(target, &exact, assigned_handles))
}

fn bind_target_with_wait(
    session: &mut PlaybackSession,
    target: &WindowTarget,
    stop_token: &StopToken,
) -> Result<BindingOutcome, String> {
    let started = Instant::now();
    let deadline = started + DEFERRED_BIND_TIMEOUT;
    loop {
        if stop_token.is_stopped() {
            return Err("playback stopped".to_string());
        }
        session.remove_expired_lifetimes();
        let unavailable = match discover_candidate_choice(target, &session.assigned_handles)? {
            unavailable @ (CandidateChoice::Missing
            | CandidateChoice::Incompatible { .. }
            | CandidateChoice::AlreadyAssigned { .. }) => unavailable,
            choice => {
                let mut outcome = bind_candidate_choice(session, target, choice)?;
                wait_for_binding_settle(stop_token)?;
                let snapshot = window_target::snapshot_window(outcome.binding.handle)
                    .map_err(|error| error.to_string())?;
                validate_bound_snapshot(target, &outcome.binding, &snapshot)?;
                outcome.pause_ms = elapsed_millis(started).max(outcome.pause_ms);
                return Ok(outcome);
            }
        };

        if Instant::now() >= deadline {
            return Err(deferred_timeout_error(target, unavailable));
        }
        sleep_until_next_poll(stop_token, deadline)?;
    }
}

fn wait_for_binding_settle(stop_token: &StopToken) -> Result<(), String> {
    let deadline = Instant::now() + BINDING_SETTLE_DELAY;
    while Instant::now() < deadline {
        if stop_token.is_stopped() {
            return Err("playback stopped".to_string());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(remaining.min(STOP_POLL_INTERVAL));
    }
    Ok(())
}

fn deferred_timeout_error(target: &WindowTarget, choice: CandidateChoice) -> String {
    match choice {
        CandidateChoice::Missing => format!(
            "等待目标窗口 {} 超过 30 秒；未找到路径为“{}”、类名为“{}”的窗口。",
            target.id.0, target.executable_path, target.window_class
        ),
        CandidateChoice::Incompatible { .. } | CandidateChoice::AlreadyAssigned { .. } => {
            format!(
                "等待目标窗口 {} 超过 30 秒；{}",
                target.id.0,
                candidate_choice_error(target, choice)
            )
        }
        CandidateChoice::Unique(_) => {
            unreachable!("a unique candidate completes deferred binding before timeout")
        }
    }
}

fn bind_candidate_choice(
    session: &mut PlaybackSession,
    target: &WindowTarget,
    choice: CandidateChoice,
) -> Result<BindingOutcome, String> {
    match choice {
        choice @ (CandidateChoice::Missing
        | CandidateChoice::Incompatible { .. }
        | CandidateChoice::AlreadyAssigned { .. }) => Err(candidate_choice_error(target, choice)),
        CandidateChoice::Unique(snapshot) => {
            let (snapshot, lifetime_token) = stable_binding_snapshot(&snapshot)?;
            validate_snapshot(target, &snapshot)?;
            let binding = session.bind(target.id, &snapshot, lifetime_token)?;
            Ok(BindingOutcome {
                binding,
                pause_ms: 0,
            })
        }
    }
}

fn candidate_choice_error(target: &WindowTarget, choice: CandidateChoice) -> String {
    match choice {
        CandidateChoice::Missing => format!(
            "缺少目标窗口 {}：未找到路径为“{}”、类名为“{}”的窗口。",
            target.id.0, target.executable_path, target.window_class
        ),
        CandidateChoice::Incompatible {
            candidate_count,
            size_mismatch_count,
            dpi_mismatch_count,
        } => format!(
            "目标窗口 {} 找到 {candidate_count} 个程序路径和窗口类匹配的候选，但尺寸或 DPI 不兼容（客户区尺寸不匹配 {size_mismatch_count} 个，DPI 不匹配 {dpi_mismatch_count} 个）。",
            target.id.0
        ),
        CandidateChoice::AlreadyAssigned { assigned_count } => format!(
            "目标窗口 {} 的 {assigned_count} 个兼容候选均已自动绑定到其他录制目标，没有可用的未占用窗口。",
            target.id.0
        ),
        CandidateChoice::Unique(_) => {
            unreachable!("a unique candidate is bindable and has no selection error")
        }
    }
}

fn sleep_until_next_poll(stop_token: &StopToken, deadline: Instant) -> Result<(), String> {
    let poll_deadline = (Instant::now() + DEFERRED_BIND_POLL_INTERVAL).min(deadline);
    while Instant::now() < poll_deadline {
        if stop_token.is_stopped() {
            return Err("playback stopped".to_string());
        }
        let remaining = poll_deadline.saturating_duration_since(Instant::now());
        thread::sleep(remaining.min(STOP_POLL_INTERVAL));
    }
    Ok(())
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn ensure_playback_running(stop_token: &StopToken) -> Result<(), String> {
    if stop_token.is_stopped() {
        Err("playback stopped".to_string())
    } else {
        Ok(())
    }
}

fn stable_binding_snapshot(
    expected: &WindowSnapshot,
) -> Result<(WindowSnapshot, WindowLifetimeToken), String> {
    let before = input::window_lifetime_token(expected.handle.raw());
    let snapshot =
        window_target::snapshot_window(expected.handle).map_err(|error| error.to_string())?;
    let after = input::window_lifetime_token(expected.handle.raw());
    if !lifetime_values_match(before.value(), after.value())
        || !same_window_instance(expected, &snapshot)
    {
        return Err(format!(
            "候选窗口 0x{:X} 在绑定期间已关闭或被重新创建，拒绝绑定已变化的实例。",
            expected.handle.raw()
        ));
    }
    Ok((snapshot, after))
}

fn lifetime_values_match(expected: u64, current: u64) -> bool {
    expected == current
}

fn same_window_instance(recorded: &WindowSnapshot, current: &WindowSnapshot) -> bool {
    recorded.handle == current.handle
        && recorded.process_id == current.process_id
        && windows_path_eq(&recorded.executable_path, &current.executable_path)
        && recorded.window_class == current.window_class
}

fn windows_path_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right) || left.to_lowercase() == right.to_lowercase()
}

fn prepare_compatible_target(
    target: &WindowTarget,
    binding: &BoundWindow,
    validate_current_geometry: bool,
) -> Result<WindowSnapshot, String> {
    if !window_target::window_is_alive(binding.handle) {
        return Err(format!("目标窗口 {} 已关闭或不可用。", target.id.0));
    }
    let snapshot =
        window_target::snapshot_window(binding.handle).map_err(|error| error.to_string())?;
    if !binding.matches_snapshot(&snapshot) {
        return Err(format!(
            "目标窗口 {} 的已绑定实例发生变化，拒绝继续回放。",
            target.id.0
        ));
    }
    if validate_current_geometry {
        validate_bound_geometry(target, binding, &snapshot)?;
    }
    Ok(snapshot)
}

fn validate_snapshot(target: &WindowTarget, snapshot: &WindowSnapshot) -> Result<(), String> {
    if !window_target::matches_recorded_target(target, snapshot) {
        return Err(format!(
            "目标窗口 {} 的程序路径或窗口类已变化，拒绝继续回放。",
            target.id.0
        ));
    }
    validate_geometry(target, snapshot)
}

fn validate_bound_snapshot(
    target: &WindowTarget,
    binding: &BoundWindow,
    snapshot: &WindowSnapshot,
) -> Result<(), String> {
    if !binding.matches_snapshot(snapshot) {
        return Err(format!(
            "目标窗口 {} 的已绑定实例发生变化，拒绝继续回放。",
            target.id.0
        ));
    }
    validate_bound_geometry(target, binding, snapshot)
}

fn validate_bound_geometry(
    target: &WindowTarget,
    binding: &BoundWindow,
    snapshot: &WindowSnapshot,
) -> Result<(), String> {
    if snapshot.dpi != binding.dpi {
        return Err(format!(
            "目标窗口 {} 的 DPI 在回放期间从 {} 变为 {}。",
            target.id.0, binding.dpi, snapshot.dpi
        ));
    }
    Ok(())
}

fn validate_geometry(target: &WindowTarget, snapshot: &WindowSnapshot) -> Result<(), String> {
    if snapshot.client_size != target.client_size {
        return Err(format!(
            "目标窗口 {} 的客户区尺寸不匹配：录制为 {}×{}，当前为 {}×{}。",
            target.id.0,
            target.client_size.width,
            target.client_size.height,
            snapshot.client_size.width,
            snapshot.client_size.height
        ));
    }
    if snapshot.dpi != target.dpi {
        return Err(format!(
            "目标窗口 {} 的 DPI 不匹配：录制为 {}，当前为 {}。",
            target.id.0, target.dpi, snapshot.dpi
        ));
    }
    Ok(())
}

fn restore_if_needed(
    target: &WindowTarget,
    binding: &BoundWindow,
    snapshot: WindowSnapshot,
) -> Result<WindowSnapshot, String> {
    if snapshot.visible && !snapshot.minimized {
        return Ok(snapshot);
    }
    window_target::restore_and_activate_window(snapshot.handle)
        .map_err(|error| error.to_string())?;
    let refreshed =
        window_target::snapshot_window(snapshot.handle).map_err(|error| error.to_string())?;
    validate_bound_snapshot(target, binding, &refreshed)?;
    Ok(refreshed)
}

fn restore_foreground_target(
    target: &WindowTarget,
    binding: &BoundWindow,
) -> Result<WindowSnapshot, String> {
    window_target::restore_and_activate_window(binding.handle)
        .map_err(|error| error.to_string())?;
    let snapshot =
        window_target::snapshot_window(binding.handle).map_err(|error| error.to_string())?;
    validate_bound_snapshot(target, binding, &snapshot)?;
    Ok(snapshot)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetedInputKind {
    Pointer(WindowPointerIntent),
    KeyboardSequence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForegroundPreparation {
    None,
    Establish,
    Verify,
}

fn foreground_preparation(
    foreground_established: bool,
    input: TargetedInputKind,
) -> ForegroundPreparation {
    let needs_foreground = matches!(
        input,
        TargetedInputKind::Pointer(WindowPointerIntent::Foreground)
            | TargetedInputKind::Pointer(WindowPointerIntent::WindowAdjustment)
            | TargetedInputKind::KeyboardSequence
    );
    if !needs_foreground {
        ForegroundPreparation::None
    } else if !foreground_established {
        ForegroundPreparation::Establish
    } else {
        ForegroundPreparation::Verify
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PointerPolicy {
    require_foreground_before_input: bool,
}

fn pointer_policy(intent: WindowPointerIntent) -> PointerPolicy {
    match intent {
        WindowPointerIntent::Foreground => PointerPolicy {
            require_foreground_before_input: true,
        },
        WindowPointerIntent::WindowAdjustment => PointerPolicy {
            require_foreground_before_input: true,
        },
        WindowPointerIntent::ActivationClick => PointerPolicy {
            require_foreground_before_input: false,
        },
        WindowPointerIntent::DropRelease | WindowPointerIntent::BackgroundWheel => PointerPolicy {
            require_foreground_before_input: false,
        },
    }
}

fn enforce_pre_input_foreground(
    intent: WindowPointerIntent,
    handle: WindowHandle,
) -> Result<(), String> {
    if !pointer_policy(intent).require_foreground_before_input {
        return Ok(());
    }
    if target_is_foreground(handle)? {
        Ok(())
    } else {
        Err(format!(
            "目标窗口 0x{:X} 已不再位于前台，已停止回放以避免误操作。",
            handle.raw()
        ))
    }
}

fn target_is_foreground(handle: WindowHandle) -> Result<bool, String> {
    Ok(window_target::foreground_window()
        .map_err(|error| error.to_string())?
        .is_some_and(|snapshot| snapshot.handle == handle))
}

fn require_foreground(handle: WindowHandle, target: &WindowTarget) -> Result<(), String> {
    if target_is_foreground(handle)? {
        Ok(())
    } else {
        Err(format!(
            "键盘序列目标窗口 {} 已不再位于前台，已停止回放以避免误输入。",
            target.id.0
        ))
    }
}

fn relative_screen_point(
    handle: WindowHandle,
    origin: ScreenPoint,
    relative_x: i32,
    relative_y: i32,
) -> Result<ScreenPoint, String> {
    let x = origin.x.checked_add(relative_x).ok_or_else(|| {
        format!(
            "目标窗口 0x{:X} 的横向相对坐标 {relative_x} 溢出。",
            handle.raw()
        )
    })?;
    let y = origin.y.checked_add(relative_y).ok_or_else(|| {
        format!(
            "目标窗口 0x{:X} 的纵向相对坐标 {relative_y} 溢出。",
            handle.raw()
        )
    })?;
    Ok(ScreenPoint { x, y })
}

fn verify_unoccluded(
    target: &WindowTarget,
    handle: WindowHandle,
    point: ScreenPoint,
) -> Result<(), String> {
    let hit = window_target::window_from_screen_point(point).map_err(|error| error.to_string())?;
    match hit {
        Some(snapshot) if snapshot.handle == handle => Ok(()),
        Some(snapshot) => Err(format!(
            "目标窗口 {} 的操作点 ({}, {}) 被窗口“{}”遮挡。",
            target.id.0, point.x, point.y, snapshot.title
        )),
        None => Err(format!(
            "目标窗口 {} 的操作点 ({}, {}) 当前未命中该窗口。",
            target.id.0, point.x, point.y
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClientSize, TargetWindowAvailability};

    fn target(id: u32) -> WindowTarget {
        WindowTarget {
            id: TargetWindowId(id),
            executable_path: r"C:\Apps\Example.exe".to_string(),
            window_class: "ExampleWindow".to_string(),
            title: "Example".to_string(),
            client_size: ClientSize {
                width: 800,
                height: 600,
            },
            dpi: 96,
            availability: TargetWindowAvailability::Initial,
        }
    }

    fn snapshot(raw: usize) -> WindowSnapshot {
        WindowSnapshot {
            handle: WindowHandle::from_raw(raw),
            executable_path: r"C:\Apps\Example.exe".to_string(),
            window_class: "ExampleWindow".to_string(),
            title: "Example".to_string(),
            client_origin: ScreenPoint { x: 100, y: 200 },
            client_size: ClientSize {
                width: 800,
                height: 600,
            },
            dpi: 96,
            process_id: 20,
            visible: true,
            minimized: false,
        }
    }

    #[test]
    fn candidate_choice_automatically_uses_the_first_compatible_unassigned_window() {
        let mut first = snapshot(1);
        first.title = "Example".to_string();
        let mut second = snapshot(2);
        second.title = "Example - document".to_string();
        let mut candidates = vec![second.clone(), first.clone()];
        window_target::sort_candidates_for_target(&target(1), &mut candidates);
        let mut assigned = HashMap::new();

        assert_eq!(
            choose_candidate(&target(1), &[], &assigned),
            CandidateChoice::Missing
        );
        assert_eq!(
            choose_candidate(&target(1), std::slice::from_ref(&first), &assigned),
            CandidateChoice::Unique(first.clone())
        );
        assert_eq!(
            choose_candidate(&target(1), &candidates, &assigned),
            CandidateChoice::Unique(first.clone()),
            "multiple exact candidates are resolved by the existing title order"
        );

        assigned.insert(first.handle, TargetWindowId(9));
        assert_eq!(
            choose_candidate(&target(1), &candidates, &assigned),
            CandidateChoice::Unique(second.clone())
        );
        assigned.insert(second.handle, TargetWindowId(10));
        assert_eq!(
            choose_candidate(&target(1), &candidates, &assigned),
            CandidateChoice::AlreadyAssigned { assigned_count: 2 }
        );
    }

    #[test]
    fn multiple_candidates_are_resolved_without_a_manual_choice() {
        let target = target(1);
        let first = snapshot(1);
        let second = snapshot(2);
        let choice = choose_candidate(&target, &[first.clone(), second], &HashMap::new());
        assert_eq!(choice, CandidateChoice::Unique(first));
    }

    #[test]
    fn missing_exact_identity_never_uses_changed_identity_or_manual_resolution() {
        let target = target(1);
        let mut session = PlaybackSession::default();
        let error = bind_candidate_choice(&mut session, &target, CandidateChoice::Missing);
        assert!(error
            .expect_err("missing exact identity must stop")
            .contains("未找到路径"));
    }

    #[test]
    fn incompatible_best_title_is_skipped_for_a_compatible_candidate() {
        let target = target(1);
        let mut best_title = snapshot(1);
        best_title.title = target.title.clone();
        best_title.client_size.width += 1;
        let mut compatible = snapshot(2);
        compatible.title = "Unrelated title".to_string();
        let mut candidates = vec![compatible.clone(), best_title];
        window_target::sort_candidates_for_target(&target, &mut candidates);

        assert_eq!(
            choose_candidate(&target, &candidates, &HashMap::new()),
            CandidateChoice::Unique(compatible)
        );
    }

    #[test]
    fn incompatible_exact_candidates_report_geometry_details() {
        let target = target(1);
        let mut wrong_size = snapshot(1);
        wrong_size.client_size.width += 1;
        let mut wrong_dpi = snapshot(2);
        wrong_dpi.dpi = 120;
        let choice = choose_candidate(&target, &[wrong_size, wrong_dpi], &HashMap::new());
        assert_eq!(
            choice,
            CandidateChoice::Incompatible {
                candidate_count: 2,
                size_mismatch_count: 1,
                dpi_mismatch_count: 1,
            }
        );
        let error = candidate_choice_error(&target, choice);
        assert!(error.contains("客户区尺寸不匹配 1 个"));
        assert!(error.contains("DPI 不匹配 1 个"));
    }

    #[test]
    fn stable_lifetime_values_must_match() {
        assert!(lifetime_values_match(7, 7));
        assert!(!lifetime_values_match(7, 8));
    }

    #[test]
    fn stopped_token_blocks_initial_binding_work() {
        let stop_token = StopToken::default();
        assert_eq!(ensure_playback_running(&stop_token), Ok(()));
        stop_token.request_stop();
        assert_eq!(
            ensure_playback_running(&stop_token),
            Err("playback stopped".to_string())
        );
    }

    #[test]
    fn launcher_activation_click_defers_foreground_validation_to_the_next_target() {
        assert_eq!(
            pointer_policy(WindowPointerIntent::Foreground),
            PointerPolicy {
                require_foreground_before_input: true,
            }
        );
        assert_eq!(
            pointer_policy(WindowPointerIntent::ActivationClick),
            PointerPolicy {
                require_foreground_before_input: false,
            }
        );
        for intent in [
            WindowPointerIntent::DropRelease,
            WindowPointerIntent::BackgroundWheel,
        ] {
            assert_eq!(
                pointer_policy(intent),
                PointerPolicy {
                    require_foreground_before_input: false,
                }
            );
        }
    }

    #[test]
    fn each_loop_establishes_foreground_once_then_only_verifies_it() {
        assert_eq!(
            foreground_preparation(
                false,
                TargetedInputKind::Pointer(WindowPointerIntent::Foreground)
            ),
            ForegroundPreparation::Establish
        );
        assert_eq!(
            foreground_preparation(
                true,
                TargetedInputKind::Pointer(WindowPointerIntent::Foreground)
            ),
            ForegroundPreparation::Verify
        );
        assert_eq!(
            foreground_preparation(false, TargetedInputKind::KeyboardSequence),
            ForegroundPreparation::Establish
        );
        for intent in [
            WindowPointerIntent::ActivationClick,
            WindowPointerIntent::DropRelease,
            WindowPointerIntent::BackgroundWheel,
        ] {
            assert_eq!(
                foreground_preparation(false, TargetedInputKind::Pointer(intent)),
                ForegroundPreparation::None
            );
        }

        let mut session = PlaybackSession::default();
        assert!(!session.foreground_established);
        session.mark_foreground_established();
        assert!(session.foreground_established);
        assert_eq!(
            foreground_preparation(
                false,
                TargetedInputKind::Pointer(WindowPointerIntent::BackgroundWheel)
            ),
            ForegroundPreparation::None
        );
        assert_eq!(
            foreground_preparation(
                false,
                TargetedInputKind::Pointer(WindowPointerIntent::Foreground)
            ),
            ForegroundPreparation::Establish
        );
        session.mark_foreground_established();
        session.begin_loop();
        assert!(!session.foreground_established);
    }

    #[test]
    fn compatibility_rejects_identity_size_and_dpi_changes() {
        let target = target(1);
        let valid = snapshot(1);
        assert_eq!(validate_snapshot(&target, &valid), Ok(()));

        let mut wrong_identity = valid.clone();
        wrong_identity.window_class = "OtherWindow".to_string();
        assert!(validate_snapshot(&target, &wrong_identity)
            .expect_err("identity mismatch")
            .contains("程序路径或窗口类"));

        let mut wrong_size = valid.clone();
        wrong_size.client_size.width += 1;
        assert!(validate_snapshot(&target, &wrong_size)
            .expect_err("size mismatch")
            .contains("客户区尺寸不匹配"));

        let mut wrong_dpi = valid;
        wrong_dpi.dpi = 120;
        assert!(validate_snapshot(&target, &wrong_dpi)
            .expect_err("DPI mismatch")
            .contains("DPI 不匹配"));
    }

    #[test]
    fn bound_window_accepts_recorded_resizing_but_not_a_dpi_change() {
        let target = target(1);
        let valid = snapshot(1);
        let binding = BoundWindow::from_stable_snapshot(
            &valid,
            input::window_lifetime_token(valid.handle.raw()),
        );
        let mut resized = valid.clone();
        resized.client_size.width += 120;
        resized.client_size.height += 80;
        assert_eq!(validate_bound_geometry(&target, &binding, &resized), Ok(()));

        resized.dpi = 144;
        assert!(validate_bound_geometry(&target, &binding, &resized)
            .expect_err("DPI change")
            .contains("DPI 在回放期间"));
    }

    #[test]
    fn relative_point_conversion_supports_negative_origins_and_rejects_overflow() {
        let handle = WindowHandle::from_raw(4);
        assert_eq!(
            relative_screen_point(handle, ScreenPoint { x: -100, y: 25 }, 30, -5),
            Ok(ScreenPoint { x: -70, y: 20 })
        );
        assert!(relative_screen_point(handle, ScreenPoint { x: i32::MAX, y: 0 }, 1, 0,).is_err());
    }
}
