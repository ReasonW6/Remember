use crate::{
    capture_warning, combo_box,
    input::{self, SystemInputExecutor, WindowLifetimeToken},
    model::{
        ButtonState, ControlBounds, KeyState, MouseButton, TargetWindowId, WindowPointerIntent,
        WindowTarget,
    },
    player::{StepExecutor, StopToken},
    window_target::{self, ScreenPoint, WindowHandle, WindowSnapshot},
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
    app: Option<AppHandle>,
}

impl WindowPlaybackExecutor {
    pub fn new(stop_token: StopToken) -> Self {
        Self {
            input: SystemInputExecutor,
            stop_token,
            session: Mutex::new(PlaybackSession::default()),
            app: None,
        }
    }

    pub fn with_app(app: AppHandle, stop_token: StopToken) -> Self {
        Self {
            input: SystemInputExecutor,
            stop_token,
            session: Mutex::new(PlaybackSession::default()),
            app: Some(app),
        }
    }

    fn session(&self) -> Result<MutexGuard<'_, PlaybackSession>, String> {
        self.session
            .lock()
            .map_err(|_| "window playback session lock poisoned".to_string())
    }

    fn ensure_binding(
        &self,
        target: &WindowTarget,
        input_held: bool,
    ) -> Result<BindingOutcome, String> {
        let mut session = self.session()?;
        if let Some(binding) = session.binding(target.id) {
            if binding_has_same_live_instance(&binding) {
                return Ok(BindingOutcome {
                    binding,
                    pause_ms: 0,
                });
            }
            session.remove_binding(target.id);
        }

        if input_held {
            let choice = discover_candidate_choice(target, &session.assigned_handles)?;
            require_immediate_candidate_while_input_held(target, &choice)?;
            return bind_candidate_choice(&mut session, target, choice);
        }
        bind_target_with_wait(&mut session, target, &self.stop_token, self.app.as_ref())
    }

    fn passive_binding(&self, target: &WindowTarget) -> Result<Option<BoundWindow>, String> {
        let mut session = self.session()?;
        if let Some(binding) = session.binding(target.id) {
            if binding_has_same_live_instance(&binding) {
                return Ok(Some(binding));
            }
            session.remove_binding(target.id);
        }
        if let Some(binding) = session.passive_binding(target.id) {
            if binding_has_same_live_instance(&binding) {
                return Ok(Some(binding));
            }
            session.remove_passive_binding(target.id);
        }

        let choice = match discover_candidate_choice(target, &session.assigned_handles) {
            Ok(choice) => choice,
            Err(_) => return Ok(None),
        };
        let Some(candidate) = passive_pointer_candidate(choice) else {
            return Ok(None);
        };
        let (snapshot, lifetime_token) = match stable_binding_snapshot(&candidate) {
            Ok(stable) => stable,
            Err(_) => return Ok(None),
        };
        if !snapshot.visible || snapshot.minimized {
            return Ok(None);
        }
        Ok(Some(session.cache_passive_binding(
            target.id,
            &snapshot,
            lifetime_token,
        )))
    }

    fn prepare_passive_pointer_target(
        &self,
        target: &WindowTarget,
        relative_x: i32,
        relative_y: i32,
    ) -> Result<Option<ScreenPoint>, String> {
        ensure_playback_running(&self.stop_token)?;
        let Some(binding) = self.passive_binding(target)? else {
            return Ok(None);
        };
        let snapshot = match prepare_compatible_target(target, &binding, true) {
            Ok(snapshot) if snapshot.visible && !snapshot.minimized => snapshot,
            Ok(_) | Err(_) => return Ok(None),
        };
        let point = relative_screen_point(
            binding.handle,
            snapshot.client_origin,
            relative_x,
            relative_y,
        )?;
        if !window_target::is_screen_point_reachable(point).map_err(|error| error.to_string())? {
            return Ok(None);
        }
        ensure_playback_running(&self.stop_token)?;
        Ok(Some(point))
    }

    fn prepare_pointer_target(
        &self,
        target: &WindowTarget,
        relative_x: i32,
        relative_y: i32,
        intent: WindowPointerIntent,
        input_kind: PointerInputKind,
        input_held: bool,
    ) -> Result<(WindowHandle, ScreenPoint, ScreenPoint, u64), String> {
        let outcome = self.ensure_binding(target, input_held)?;
        let adjustment = self.session()?.pointer_adjustment(target.id);
        let adjustment_active = intent == WindowPointerIntent::WindowAdjustment
            && adjustment.is_some_and(|active| active.handle == outcome.binding.handle);
        let snapshot = prepare_compatible_target(target, &outcome.binding, !adjustment_active)?;
        let transient_target = window_target::is_owned_transient_window_class(&target.window_class);
        let restore_needed = !snapshot.visible || snapshot.minimized;
        if transient_target && restore_needed {
            return Err(format!(
                "瞬时目标窗口 {} 已在执行操作前关闭，请重新展开对应的下拉框或菜单。",
                target.id.0
            ));
        }
        if restore_needed {
            require_no_held_input_for_target_transition(input_held, target, "恢复并激活")?;
        }
        let restore_started = Instant::now();
        let mut snapshot = restore_if_needed(target, &outcome.binding, snapshot)?;
        let mut pause_ms = outcome.pause_ms;
        if restore_needed {
            pause_ms = pause_ms.saturating_add(elapsed_millis(restore_started));
        }
        match pointer_foreground_preparation(input_kind, intent, input_held, transient_target) {
            ForegroundPreparation::None => {}
            ForegroundPreparation::Establish => {
                if !target_is_foreground(outcome.binding.handle)? {
                    require_no_held_input_for_target_transition(input_held, target, "切换到前台")?;
                    let foreground_started = Instant::now();
                    snapshot = restore_foreground_target(target, &outcome.binding)?;
                    pause_ms = pause_ms.saturating_add(elapsed_millis(foreground_started));
                }
                enforce_pre_input_foreground(intent, outcome.binding.handle)?;
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
        if !adjustment_active && !transient_target {
            verify_unoccluded(target, outcome.binding.handle, point)?;
        }
        ensure_playback_running(&self.stop_token)?;
        Ok((outcome.binding.handle, point, client_origin, pause_ms))
    }

    fn prepare_foreground_target(
        &self,
        target: &WindowTarget,
        input_held: bool,
    ) -> Result<(WindowHandle, u64), String> {
        let outcome = self.ensure_binding(target, input_held)?;
        let snapshot = prepare_compatible_target(target, &outcome.binding, true)?;
        let restore_needed = !snapshot.visible || snapshot.minimized;
        if restore_needed {
            require_no_held_input_for_target_transition(input_held, target, "恢复并激活")?;
        }
        let restore_started = Instant::now();
        let _snapshot = restore_if_needed(target, &outcome.binding, snapshot)?;
        let mut pause_ms = outcome.pause_ms;
        if restore_needed {
            pause_ms = pause_ms.saturating_add(elapsed_millis(restore_started));
        }
        if !target_is_foreground(outcome.binding.handle)? {
            require_no_held_input_for_target_transition(input_held, target, "切换到前台")?;
            let foreground_started = Instant::now();
            let _snapshot = restore_foreground_target(target, &outcome.binding)?;
            pause_ms = pause_ms.saturating_add(elapsed_millis(foreground_started));
        }
        require_foreground(outcome.binding.handle, target)?;
        ensure_playback_running(&self.stop_token)?;
        Ok((outcome.binding.handle, pause_ms))
    }
}

fn require_no_held_input_for_target_transition(
    input_held: bool,
    target: &WindowTarget,
    transition: &str,
) -> Result<(), String> {
    if input_held {
        Err(format!(
            "仍有按键或鼠标按钮处于按下状态，不能为目标窗口 {} 执行{transition}；已停止回放以避免输入被卡住。",
            target.id.0
        ))
    } else {
        Ok(())
    }
}

fn require_immediate_candidate_while_input_held(
    target: &WindowTarget,
    choice: &CandidateChoice,
) -> Result<(), String> {
    if matches!(choice, CandidateChoice::Unique(_)) {
        Ok(())
    } else {
        Err(format!(
            "仍有按键或鼠标按钮处于按下状态，不能等待目标窗口 {} 出现或变为可用；已停止回放以避免输入被卡住。",
            target.id.0
        ))
    }
}

impl StepExecutor for WindowPlaybackExecutor {
    fn prepare_window_relative_playback(&self) -> Result<(), String> {
        if let Some(app) = &self.app {
            let _ = capture_warning::hide(app);
        }
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
        if !input_held {
            if let Some(point) = self.prepare_passive_pointer_target(target, x, y)? {
                self.input.mouse_move(point.x, point.y)?;
            }
            return Ok(0);
        }
        let (_handle, point, _origin, pause_ms) =
            self.prepare_pointer_target(target, x, y, intent, PointerInputKind::Move, input_held)?;
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
        let (handle, point, client_origin, pause_ms) = self.prepare_pointer_target(
            target,
            x,
            y,
            intent,
            PointerInputKind::Button(state),
            input_held,
        )?;
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
            self.prepare_pointer_target(target, x, y, intent, PointerInputKind::Wheel, input_held)?;
        self.input.mouse_wheel(point.x, point.y, delta)?;
        Ok(pause_ms)
    }

    fn window_select_combo_option(
        &self,
        target: &WindowTarget,
        control_id: i32,
        control_bounds: ControlBounds,
        option_name: &str,
        input_held: bool,
    ) -> Result<u64, String> {
        let (handle, pause_ms) = self.prepare_foreground_target(target, input_held)?;
        combo_box::select_option(handle, control_id, control_bounds, option_name).map_err(
            |error| {
                format!(
                    "无法在目标窗口 {} 中选择下拉选项“{}”：{}",
                    target.id.0, option_name, error
                )
            },
        )?;
        ensure_playback_running(&self.stop_token)?;
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
            let (_handle, pause_ms) = self.prepare_foreground_target(target, input_held)?;
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
    passive_bindings: HashMap<TargetWindowId, Binding>,
    assigned_handles: HashMap<WindowHandle, TargetWindowId>,
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
    dpi: u32,
}

#[derive(Debug, Clone)]
struct BindingOutcome {
    binding: BoundWindow,
    pause_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct PreparedWindow {
    client_origin: ScreenPoint,
    visible: bool,
    minimized: bool,
}

impl BoundWindow {
    fn from_stable_snapshot(
        snapshot: &WindowSnapshot,
        lifetime_token: WindowLifetimeToken,
    ) -> Self {
        Self {
            handle: snapshot.handle,
            lifetime_token,
            dpi: snapshot.dpi,
        }
    }
}

type Binding = BoundWindow;

impl PlaybackSession {
    fn clear(&mut self) {
        self.bindings.clear();
        self.passive_bindings.clear();
        self.assigned_handles.clear();
        self.pointer_adjustment = None;
    }

    fn binding(&self, target_id: TargetWindowId) -> Option<BoundWindow> {
        self.bindings.get(&target_id).cloned()
    }

    fn passive_binding(&self, target_id: TargetWindowId) -> Option<BoundWindow> {
        self.passive_bindings.get(&target_id).cloned()
    }

    fn cache_passive_binding(
        &mut self,
        target_id: TargetWindowId,
        snapshot: &WindowSnapshot,
        lifetime_token: WindowLifetimeToken,
    ) -> BoundWindow {
        let binding = BoundWindow::from_stable_snapshot(snapshot, lifetime_token);
        self.passive_bindings.insert(target_id, binding.clone());
        binding
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
        self.remove_passive_binding(target_id);
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

    fn remove_passive_binding(&mut self, target_id: TargetWindowId) {
        self.passive_bindings.remove(&target_id);
    }

    fn begin_loop(&mut self) {
        self.remove_expired_lifetimes();
        self.pointer_adjustment = None;
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
        self.passive_bindings
            .retain(|_, binding| binding_has_same_live_instance(binding));
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
        let binding = self
            .bindings
            .get(&target_id)
            .ok_or_else(|| format!("目标窗口 {} 的绑定已失效。", target_id.0))?;
        if !binding_has_same_live_instance(binding) {
            return Err(format!(
                "目标窗口 {} 在窗口调整期间被关闭或替换。",
                target_id.0
            ));
        }
        let geometry = window_target::refresh_window_geometry(adjustment.handle)
            .map_err(|error| error.to_string())?;
        if geometry.dpi != binding.dpi {
            return Err(format!(
                "目标窗口 {} 的 DPI 在窗口调整期间从 {} 变为 {}，无法安全继续回放。",
                target_id.0, binding.dpi, geometry.dpi
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

fn passive_pointer_candidate(choice: CandidateChoice) -> Option<WindowSnapshot> {
    match choice {
        CandidateChoice::Unique(candidate) if candidate.visible && !candidate.minimized => {
            Some(candidate)
        }
        CandidateChoice::Missing
        | CandidateChoice::Unique(_)
        | CandidateChoice::Incompatible { .. }
        | CandidateChoice::AlreadyAssigned { .. } => None,
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
    app: Option<&AppHandle>,
) -> Result<BindingOutcome, String> {
    let started = Instant::now();
    let deadline = started + DEFERRED_BIND_TIMEOUT;
    loop {
        if stop_token.is_stopped() {
            hide_playback_notice(app);
            return Err("playback stopped".to_string());
        }
        session.remove_expired_lifetimes();
        let choice = match discover_candidate_choice(target, &session.assigned_handles) {
            Ok(choice) => choice,
            Err(error) => {
                hide_playback_notice(app);
                return Err(error);
            }
        };
        let unavailable = match choice {
            unavailable @ (CandidateChoice::Missing
            | CandidateChoice::Incompatible { .. }
            | CandidateChoice::AlreadyAssigned { .. }) => unavailable,
            choice => {
                let mut outcome = match bind_candidate_choice(session, target, choice) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        hide_playback_notice(app);
                        return Err(error);
                    }
                };
                if let Err(error) = wait_for_binding_settle(stop_token) {
                    hide_playback_notice(app);
                    return Err(error);
                }
                if let Err(error) = prepare_compatible_target(target, &outcome.binding, true) {
                    hide_playback_notice(app);
                    return Err(error);
                }
                outcome.pause_ms = elapsed_millis(started).max(outcome.pause_ms);
                hide_playback_notice(app);
                return Ok(outcome);
            }
        };

        if Instant::now() >= deadline {
            let error = deferred_timeout_error(target, unavailable);
            show_playback_stopped_notice(app, &error);
            return Err(error);
        }
        show_playback_waiting_notice(app, target, &unavailable, deadline);
        if let Err(error) = sleep_until_next_poll(stop_token, deadline) {
            hide_playback_notice(app);
            return Err(error);
        }
    }
}

fn show_playback_waiting_notice(
    app: Option<&AppHandle>,
    target: &WindowTarget,
    choice: &CandidateChoice,
    deadline: Instant,
) {
    let Some(app) = app else {
        return;
    };
    let message = playback_wait_message(target, choice);
    let remaining = deadline.saturating_duration_since(Instant::now());
    let seconds_remaining = u64::try_from(remaining.as_millis().div_ceil(1_000))
        .unwrap_or(u64::MAX)
        .max(1);
    if let Err(error) =
        capture_warning::show_playback_waiting_at_cursor(app, &message, seconds_remaining)
    {
        eprintln!("Remember playback wait notice could not show: {error}");
    }
}

fn playback_wait_message(target: &WindowTarget, choice: &CandidateChoice) -> String {
    let label = if target.title.trim().is_empty() {
        format!("{} 窗口", target.window_class)
    } else {
        format!("“{}”", target.title)
    };
    match choice {
        CandidateChoice::Missing if target.window_class.eq_ignore_ascii_case("ComboLBox") => {
            "请展开录制时使用的下拉框；列表出现后会继续这一操作。".to_string()
        }
        CandidateChoice::Missing => format!("请打开或恢复{label}，并保持它可操作。"),
        CandidateChoice::Incompatible { .. } => format!(
            "请把{label}的客户区恢复为录制尺寸 {}×{}，并保持显示缩放不变。",
            target.client_size.width, target.client_size.height
        ),
        CandidateChoice::AlreadyAssigned { .. } => {
            format!("当前匹配窗口已用于其他录制目标；请再打开一个{label}。")
        }
        CandidateChoice::Unique(_) => {
            unreachable!("a unique candidate never displays a playback wait notice")
        }
    }
}

fn show_playback_stopped_notice(app: Option<&AppHandle>, message: &str) {
    let Some(app) = app else {
        return;
    };
    if let Err(error) = capture_warning::show_playback_stopped_at_cursor(app, message) {
        eprintln!("Remember playback stopped notice could not show: {error}");
    }
}

fn hide_playback_notice(app: Option<&AppHandle>) {
    let Some(app) = app else {
        return;
    };
    if let Err(error) = capture_warning::hide(app) {
        eprintln!("Remember playback notice could not hide: {error}");
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
            "等待目标窗口 {} 超过 30 秒；未找到类名为“{}”、路径为“{}”的窗口。",
            target.id.0, target.window_class, target.executable_path
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
) -> Result<PreparedWindow, String> {
    if !binding_has_same_live_instance(binding) {
        return Err(format!("目标窗口 {} 已关闭或不可用。", target.id.0));
    }
    let geometry = window_target::refresh_window_geometry(binding.handle)
        .map_err(|error| error.to_string())?;
    let display =
        window_target::window_display_state(binding.handle).map_err(|error| error.to_string())?;
    if !binding_has_same_live_instance(binding) {
        return Err(format!(
            "目标窗口 {} 的已绑定实例发生变化，拒绝继续回放。",
            target.id.0
        ));
    }
    if validate_current_geometry {
        validate_bound_dpi(target, binding, geometry.dpi)?;
    }
    Ok(PreparedWindow {
        client_origin: geometry.client_origin,
        visible: display.visible,
        minimized: display.minimized,
    })
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

fn validate_bound_dpi(
    target: &WindowTarget,
    binding: &BoundWindow,
    current_dpi: u32,
) -> Result<(), String> {
    if current_dpi != binding.dpi {
        return Err(format!(
            "目标窗口 {} 的 DPI 在回放期间从 {} 变为 {}。",
            target.id.0, binding.dpi, current_dpi
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
    snapshot: PreparedWindow,
) -> Result<PreparedWindow, String> {
    if snapshot.visible && !snapshot.minimized {
        return Ok(snapshot);
    }
    window_target::restore_and_activate_window(binding.handle)
        .map_err(|error| error.to_string())?;
    prepare_compatible_target(target, binding, true)
}

fn restore_foreground_target(
    target: &WindowTarget,
    binding: &BoundWindow,
) -> Result<PreparedWindow, String> {
    window_target::restore_and_activate_window(binding.handle)
        .map_err(|error| error.to_string())?;
    prepare_compatible_target(target, binding, true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerInputKind {
    Move,
    Button(ButtonState),
    Wheel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForegroundPreparation {
    None,
    Establish,
    Verify,
}

fn pointer_foreground_preparation(
    input_kind: PointerInputKind,
    intent: WindowPointerIntent,
    input_held: bool,
    transient_target: bool,
) -> ForegroundPreparation {
    if transient_target {
        return ForegroundPreparation::None;
    }
    let starts_effective_operation = matches!(
        input_kind,
        PointerInputKind::Button(ButtonState::Pressed) | PointerInputKind::Wheel
    );
    if !starts_effective_operation || !pointer_policy(intent).require_foreground_before_input {
        ForegroundPreparation::None
    } else if input_held {
        ForegroundPreparation::Verify
    } else {
        ForegroundPreparation::Establish
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
    fn held_input_blocks_restore_and_focus_transitions_immediately() {
        let target = target(7);

        for transition in ["恢复并激活", "切换到前台"] {
            let error = require_no_held_input_for_target_transition(true, &target, transition)
                .expect_err("held input must reject a target transition");
            assert!(error.contains("仍有按键或鼠标按钮处于按下状态"));
            assert!(error.contains(transition));
        }
        assert_eq!(
            require_no_held_input_for_target_transition(false, &target, "恢复并激活"),
            Ok(())
        );
    }

    #[test]
    fn held_input_allows_only_an_already_available_binding_candidate() {
        let target = target(7);
        assert_eq!(
            require_immediate_candidate_while_input_held(
                &target,
                &CandidateChoice::Unique(snapshot(10))
            ),
            Ok(())
        );
        let error =
            require_immediate_candidate_while_input_held(&target, &CandidateChoice::Missing)
                .expect_err("held input must not wait for a missing target");
        assert!(error.contains("不能等待目标窗口 7"));
    }

    #[test]
    fn each_loop_discards_an_incomplete_window_adjustment() {
        let mut session = PlaybackSession::default();
        session.begin_pointer_adjustment(
            TargetWindowId(7),
            WindowHandle::from_raw(10),
            ScreenPoint { x: 100, y: 200 },
        );
        assert!(session.pointer_adjustment(TargetWindowId(7)).is_some());

        session.begin_loop();

        assert!(session.pointer_adjustment(TargetWindowId(7)).is_none());
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
    fn passive_pointer_motion_never_establishes_or_verifies_foreground() {
        assert_eq!(
            pointer_foreground_preparation(
                PointerInputKind::Move,
                WindowPointerIntent::Foreground,
                false,
                false,
            ),
            ForegroundPreparation::None
        );
        assert_eq!(
            pointer_foreground_preparation(
                PointerInputKind::Move,
                WindowPointerIntent::Foreground,
                true,
                false,
            ),
            ForegroundPreparation::None
        );
    }

    #[test]
    fn effective_pointer_operations_establish_focus_only_at_their_start() {
        assert_eq!(
            pointer_foreground_preparation(
                PointerInputKind::Button(ButtonState::Pressed),
                WindowPointerIntent::Foreground,
                false,
                false,
            ),
            ForegroundPreparation::Establish
        );
        assert_eq!(
            pointer_foreground_preparation(
                PointerInputKind::Button(ButtonState::Pressed),
                WindowPointerIntent::Foreground,
                true,
                false,
            ),
            ForegroundPreparation::Verify
        );
        assert_eq!(
            pointer_foreground_preparation(
                PointerInputKind::Button(ButtonState::Released),
                WindowPointerIntent::Foreground,
                true,
                false,
            ),
            ForegroundPreparation::None
        );
        assert_eq!(
            pointer_foreground_preparation(
                PointerInputKind::Wheel,
                WindowPointerIntent::Foreground,
                false,
                false,
            ),
            ForegroundPreparation::Establish
        );
    }

    #[test]
    fn launcher_drop_and_background_wheel_never_force_foreground() {
        for (kind, intent) in [
            (
                PointerInputKind::Button(ButtonState::Pressed),
                WindowPointerIntent::ActivationClick,
            ),
            (
                PointerInputKind::Button(ButtonState::Released),
                WindowPointerIntent::DropRelease,
            ),
            (
                PointerInputKind::Wheel,
                WindowPointerIntent::BackgroundWheel,
            ),
        ] {
            assert_eq!(
                pointer_foreground_preparation(kind, intent, false, false),
                ForegroundPreparation::None
            );
        }
    }

    #[test]
    fn legacy_transient_surface_is_never_restored_or_forced_to_foreground() {
        assert_eq!(
            pointer_foreground_preparation(
                PointerInputKind::Button(ButtonState::Pressed),
                WindowPointerIntent::Foreground,
                false,
                true,
            ),
            ForegroundPreparation::None
        );
    }

    #[test]
    fn wait_notice_explains_the_action_that_can_make_each_candidate_state_continue() {
        let mut combo = target(4);
        combo.window_class = "ComboLBox".to_string();
        combo.title.clear();
        assert!(playback_wait_message(&combo, &CandidateChoice::Missing).contains("展开"));

        let incompatible = playback_wait_message(
            &target(5),
            &CandidateChoice::Incompatible {
                candidate_count: 1,
                size_mismatch_count: 1,
                dpi_mismatch_count: 0,
            },
        );
        assert!(incompatible.contains("800×600"));
        assert!(incompatible.contains("显示缩放"));

        assert!(playback_wait_message(
            &target(6),
            &CandidateChoice::AlreadyAssigned { assigned_count: 1 },
        )
        .contains("再打开一个"));
    }

    #[test]
    fn passive_motion_only_uses_an_already_visible_candidate() {
        assert!(passive_pointer_candidate(CandidateChoice::Missing).is_none());

        let mut hidden = snapshot(11);
        hidden.visible = false;
        assert!(passive_pointer_candidate(CandidateChoice::Unique(hidden)).is_none());

        let mut minimized = snapshot(12);
        minimized.minimized = true;
        assert!(passive_pointer_candidate(CandidateChoice::Unique(minimized)).is_none());

        let visible = snapshot(13);
        assert_eq!(
            passive_pointer_candidate(CandidateChoice::Unique(visible.clone())),
            Some(visible)
        );
    }

    #[test]
    fn passive_motion_cache_does_not_reserve_a_window_for_an_effective_operation() {
        let mut session = PlaybackSession::default();
        let candidate = snapshot(21);
        let binding = session.cache_passive_binding(
            TargetWindowId(7),
            &candidate,
            input::window_lifetime_token(candidate.handle.raw()),
        );

        assert_eq!(binding.handle, candidate.handle);
        assert!(session.binding(TargetWindowId(7)).is_none());
        assert_eq!(
            session
                .passive_binding(TargetWindowId(7))
                .map(|cached| cached.handle),
            Some(candidate.handle)
        );
        assert!(session.assigned_handles.is_empty());

        session
            .bind(
                TargetWindowId(7),
                &candidate,
                input::window_lifetime_token(candidate.handle.raw()),
            )
            .expect("effective operation promotes the target to a reserved binding");
        assert!(session.passive_binding(TargetWindowId(7)).is_none());
        assert_eq!(
            session.assigned_handles.get(&candidate.handle),
            Some(&TargetWindowId(7))
        );
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
        assert_eq!(validate_bound_dpi(&target, &binding, resized.dpi), Ok(()));

        resized.dpi = 144;
        assert!(validate_bound_dpi(&target, &binding, resized.dpi)
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
