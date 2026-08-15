use crate::{
    model::{ControlBounds, PointerSemanticAction},
    window_target::{ScreenPoint, WindowHandle},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedComboOption {
    pub owner: WindowHandle,
    pub action: PointerSemanticAction,
}

pub fn stable_owner_for_popup(
    popup: WindowHandle,
    owner_hint: Option<WindowHandle>,
) -> Option<WindowHandle> {
    platform::stable_owner_for_popup(popup, owner_hint)
}

pub fn option_at_point(
    popup: WindowHandle,
    owner_hint: Option<WindowHandle>,
    point: ScreenPoint,
) -> Result<Option<CapturedComboOption>, String> {
    platform::option_at_point(popup, owner_hint, point)
}

pub fn select_option(
    owner: WindowHandle,
    control_id: i32,
    control_bounds: ControlBounds,
    option_name: &str,
) -> Result<(), String> {
    platform::select_option(owner, control_id, control_bounds, option_name)
}

fn choose_option_index(options: &[String], requested: &str) -> Result<usize, String> {
    let exact = options
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (value == requested).then_some(index))
        .collect::<Vec<_>>();
    match exact.as_slice() {
        [index] => return Ok(*index),
        [_, _, ..] => {
            return Err(format!(
                "下拉框中存在多个名为“{requested}”的选项，无法安全确定应选择哪一个。"
            ))
        }
        [] => {}
    }

    let requested_folded = requested.to_lowercase();
    let folded = options
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (value.to_lowercase() == requested_folded).then_some(index))
        .collect::<Vec<_>>();
    match folded.as_slice() {
        [index] => Ok(*index),
        [_, _, ..] => Err(format!(
            "下拉框中存在多个名称仅大小写不同的“{requested}”选项，无法安全确定应选择哪一个。"
        )),
        [] => Err(format!("下拉框中没有名为“{requested}”的选项。")),
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::{ffi::c_void, mem::size_of};
    use windows::{
        core::Error as WindowsError,
        Win32::{
            Foundation::{BOOL, HWND, LPARAM, POINT, RECT, WPARAM},
            Graphics::Gdi::{ClientToScreen, ScreenToClient},
            UI::{
                Controls::{GetComboBoxInfo, COMBOBOXINFO},
                WindowsAndMessaging::{
                    EnumChildWindows, EnumWindows, GetAncestor, GetClassNameW, GetDlgCtrlID,
                    GetForegroundWindow, GetParent, GetWindow, GetWindowRect,
                    GetWindowThreadProcessId, IsWindow, IsWindowVisible, SendMessageTimeoutW,
                    CBN_SELCHANGE, CBN_SELENDOK, CB_ERR, CB_GETCOUNT, CB_GETCURSEL, CB_GETLBTEXT,
                    CB_GETLBTEXTLEN, CB_SETCURSEL, CB_SHOWDROPDOWN, GA_ROOT, GA_ROOTOWNER,
                    GW_OWNER, LB_GETTEXT, LB_GETTEXTLEN, LB_ITEMFROMPOINT, SMTO_ABORTIFHUNG,
                    WM_COMMAND,
                },
            },
        },
    };

    const COMBO_CLASS: &str = "ComboBox";
    const COMBO_POPUP_CLASS: &str = "ComboLBox";
    const MESSAGE_TIMEOUT_MS: u32 = 1_000;
    const MAX_OPTION_TEXT_CHARS: usize = 32_768;

    #[derive(Debug, Clone, Copy)]
    struct ComboAssociation {
        owner: HWND,
        combo: HWND,
    }

    pub(super) fn stable_owner_for_popup(
        popup: WindowHandle,
        owner_hint: Option<WindowHandle>,
    ) -> Option<WindowHandle> {
        association_for_list(hwnd(popup), owner_hint.map(hwnd))
            .map(|association| WindowHandle::from_raw(association.owner.0 as usize))
    }

    pub(super) fn option_at_point(
        popup: WindowHandle,
        owner_hint: Option<WindowHandle>,
        point: ScreenPoint,
    ) -> Result<Option<CapturedComboOption>, String> {
        let popup = hwnd(popup);
        if read_class(popup).as_deref() != Some(COMBO_POPUP_CLASS) {
            return Ok(None);
        }
        let Some(association) = association_for_list(popup, owner_hint.map(hwnd)) else {
            return Ok(None);
        };
        let Some(option_name) = list_option_at_point(popup, point)? else {
            return Ok(None);
        };
        let control_bounds = control_bounds(association.owner, association.combo)?;
        let control_id = unsafe { GetDlgCtrlID(association.combo) };
        Ok(Some(CapturedComboOption {
            owner: WindowHandle::from_raw(association.owner.0 as usize),
            action: PointerSemanticAction::SelectComboOption {
                control_id,
                control_bounds,
                option_name,
            },
        }))
    }

    pub(super) fn select_option(
        owner: WindowHandle,
        control_id: i32,
        control_bounds: ControlBounds,
        option_name: &str,
    ) -> Result<(), String> {
        let owner = hwnd(owner);
        if !unsafe { IsWindow(owner).as_bool() } {
            return Err("下拉框所属窗口已关闭。".to_string());
        }
        let combo = find_combo_by_locator(owner, control_id, control_bounds).ok_or_else(|| {
            format!(
                "未找到录制时位于 ({}, {})、尺寸为 {}×{} 的下拉框。",
                control_bounds.x, control_bounds.y, control_bounds.width, control_bounds.height
            )
        })?;
        let options = read_combo_options(combo)?;
        let index = choose_option_index(&options, option_name)?;
        send_message(combo, CB_SHOWDROPDOWN, 0, 0)?;
        let selected = send_message(combo, CB_SETCURSEL, index, 0)?;
        if selected == CB_ERR as isize || usize::try_from(selected).ok() != Some(index) {
            return Err(format!("Windows 拒绝选择下拉选项“{option_name}”。"));
        }

        let parent = unsafe { GetParent(combo) }
            .map_err(|error| format!("无法读取下拉框所属控件：{error}"))?;
        let current_control_id = unsafe { GetDlgCtrlID(combo) };
        let command = command_wparam(current_control_id, CBN_SELCHANGE);
        send_message(parent, WM_COMMAND, command, combo.0 as isize)?;
        let accepted = command_wparam(current_control_id, CBN_SELENDOK);
        send_message(parent, WM_COMMAND, accepted, combo.0 as isize)?;

        let current = send_message(combo, CB_GETCURSEL, 0, 0)?;
        if usize::try_from(current).ok() != Some(index) {
            return Err(format!("选择“{option_name}”后验证当前选项失败。"));
        }
        Ok(())
    }

    fn association_for_list(popup: HWND, owner_hint: Option<HWND>) -> Option<ComboAssociation> {
        if !unsafe { IsWindow(popup).as_bool() }
            || read_class(popup).as_deref() != Some(COMBO_POPUP_CLASS)
        {
            return None;
        }

        let mut candidates = Vec::with_capacity(4);
        if let Some(owner_hint) = owner_hint {
            push_root_candidate(&mut candidates, owner_hint);
        }
        push_root_candidate(&mut candidates, unsafe { GetAncestor(popup, GA_ROOTOWNER) });
        if let Ok(owner) = unsafe { GetWindow(popup, GW_OWNER) } {
            push_root_candidate(&mut candidates, owner);
        }
        push_root_candidate(&mut candidates, unsafe { GetForegroundWindow() });

        for owner in candidates {
            if let Some(combo) = find_combo_for_list(owner, popup) {
                return Some(ComboAssociation { owner, combo });
            }
        }
        find_association_in_process(popup)
    }

    fn push_root_candidate(candidates: &mut Vec<HWND>, candidate: HWND) {
        if candidate.0.is_null() {
            return;
        }
        let root = unsafe { GetAncestor(candidate, GA_ROOT) };
        let root = if root.0.is_null() { candidate } else { root };
        if !candidates.iter().any(|existing| existing.0 == root.0) {
            candidates.push(root);
        }
    }

    fn find_combo_for_list(owner: HWND, list: HWND) -> Option<HWND> {
        struct Context {
            list: HWND,
            combo: HWND,
        }

        unsafe extern "system" fn visit(raw: HWND, state: LPARAM) -> BOOL {
            let context = &mut *(state.0 as *mut Context);
            if read_class(raw).as_deref() != Some(COMBO_CLASS) {
                return BOOL(1);
            }
            let mut info = COMBOBOXINFO {
                cbSize: size_of::<COMBOBOXINFO>() as u32,
                ..Default::default()
            };
            if GetComboBoxInfo(raw, &mut info).is_ok() && info.hwndList.0 == context.list.0 {
                context.combo = raw;
                BOOL(0)
            } else {
                BOOL(1)
            }
        }

        if owner.0.is_null() || owner.0 == list.0 {
            return None;
        }
        let mut context = Context {
            list,
            combo: HWND::default(),
        };
        unsafe {
            let _ = EnumChildWindows(
                owner,
                Some(visit),
                LPARAM((&mut context as *mut Context) as isize),
            );
        }
        (!context.combo.0.is_null()).then_some(context.combo)
    }

    fn find_association_in_process(list: HWND) -> Option<ComboAssociation> {
        struct Context {
            list: HWND,
            process_id: u32,
            association: Option<ComboAssociation>,
        }

        unsafe extern "system" fn visit(raw: HWND, state: LPARAM) -> BOOL {
            let context = &mut *(state.0 as *mut Context);
            let mut process_id = 0;
            if GetWindowThreadProcessId(raw, Some(&mut process_id)) == 0
                || process_id != context.process_id
            {
                return BOOL(1);
            }
            if let Some(combo) = find_combo_for_list(raw, context.list) {
                context.association = Some(ComboAssociation { owner: raw, combo });
                BOOL(0)
            } else {
                BOOL(1)
            }
        }

        let mut process_id = 0;
        if unsafe { GetWindowThreadProcessId(list, Some(&mut process_id)) } == 0 || process_id == 0
        {
            return None;
        }
        let mut context = Context {
            list,
            process_id,
            association: None,
        };
        unsafe {
            let _ = EnumWindows(Some(visit), LPARAM((&mut context as *mut Context) as isize));
        }
        context.association
    }

    fn list_option_at_point(list: HWND, point: ScreenPoint) -> Result<Option<String>, String> {
        if !unsafe { IsWindowVisible(list).as_bool() } {
            return Ok(None);
        }
        let mut client = POINT {
            x: point.x,
            y: point.y,
        };
        if !unsafe { ScreenToClient(list, &mut client).as_bool() } {
            return Err(format!(
                "无法把鼠标位置转换为下拉列表坐标：{}",
                WindowsError::from_win32()
            ));
        }
        let (Ok(x), Ok(y)) = (i16::try_from(client.x), i16::try_from(client.y)) else {
            return Ok(None);
        };
        let packed = u32::from(x as u16) | (u32::from(y as u16) << 16);
        let hit = send_message(list, LB_ITEMFROMPOINT, 0, packed as isize)? as usize;
        if (hit >> 16) != 0 {
            return Ok(None);
        }
        let index = hit & 0xffff;
        read_list_option(list, index).map(Some)
    }

    fn read_list_option(list: HWND, index: usize) -> Result<String, String> {
        let length = send_message(list, LB_GETTEXTLEN, index, 0)?;
        if length < 0 {
            return Err("无法读取鼠标指向的下拉选项长度。".to_string());
        }
        let length =
            usize::try_from(length).map_err(|_| "下拉选项长度超出支持范围。".to_string())?;
        if length > MAX_OPTION_TEXT_CHARS {
            return Err("下拉选项文本过长，无法安全录制。".to_string());
        }
        let mut buffer = vec![0u16; length.saturating_add(1)];
        let copied = send_message(list, LB_GETTEXT, index, buffer.as_mut_ptr() as isize)?;
        if copied < 0 {
            return Err("无法读取鼠标指向的下拉选项文本。".to_string());
        }
        let copied = usize::try_from(copied).unwrap_or(0).min(length);
        Ok(String::from_utf16_lossy(&buffer[..copied]))
    }

    fn control_bounds(owner: HWND, combo: HWND) -> Result<ControlBounds, String> {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(combo, &mut rect) }
            .map_err(|error| format!("无法读取下拉框位置：{error}"))?;
        let mut origin = POINT { x: 0, y: 0 };
        if !unsafe { ClientToScreen(owner, &mut origin).as_bool() } {
            return Err(format!(
                "无法读取下拉框所属窗口的客户区位置：{}",
                WindowsError::from_win32()
            ));
        }
        let width = rect.right.saturating_sub(rect.left);
        let height = rect.bottom.saturating_sub(rect.top);
        let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
            return Err("下拉框返回了无效尺寸。".to_string());
        };
        if width == 0 || height == 0 {
            return Err("下拉框返回了零尺寸。".to_string());
        }
        Ok(ControlBounds {
            x: rect.left.saturating_sub(origin.x),
            y: rect.top.saturating_sub(origin.y),
            width,
            height,
        })
    }

    fn find_combo_by_locator(
        owner: HWND,
        control_id: i32,
        recorded_bounds: ControlBounds,
    ) -> Option<HWND> {
        #[derive(Clone, Copy)]
        struct Candidate {
            hwnd: HWND,
            control_id: i32,
            bounds: ControlBounds,
        }
        struct Context {
            owner: HWND,
            candidates: Vec<Candidate>,
        }

        unsafe extern "system" fn visit(raw: HWND, state: LPARAM) -> BOOL {
            let context = &mut *(state.0 as *mut Context);
            if read_class(raw).as_deref() == Some(COMBO_CLASS) {
                if let Ok(bounds) = control_bounds(context.owner, raw) {
                    context.candidates.push(Candidate {
                        hwnd: raw,
                        control_id: GetDlgCtrlID(raw),
                        bounds,
                    });
                }
            }
            BOOL(1)
        }

        let mut context = Context {
            owner,
            candidates: Vec::new(),
        };
        unsafe {
            let _ = EnumChildWindows(
                owner,
                Some(visit),
                LPARAM((&mut context as *mut Context) as isize),
            );
        }

        let by_id = context
            .candidates
            .iter()
            .copied()
            .filter(|candidate| control_id != 0 && candidate.control_id == control_id)
            .collect::<Vec<_>>();
        if let [candidate] = by_id.as_slice() {
            return Some(candidate.hwnd);
        }
        if let Some(candidate) = by_id
            .iter()
            .find(|candidate| candidate.bounds == recorded_bounds)
        {
            return Some(candidate.hwnd);
        }

        let by_bounds = context
            .candidates
            .iter()
            .filter(|candidate| candidate.bounds == recorded_bounds)
            .collect::<Vec<_>>();
        match by_bounds.as_slice() {
            [candidate] => Some(candidate.hwnd),
            _ => None,
        }
    }

    fn read_combo_options(combo: HWND) -> Result<Vec<String>, String> {
        let count = send_message(combo, CB_GETCOUNT, 0, 0)?;
        if count == CB_ERR as isize || count < 0 {
            return Err("无法读取下拉框选项数量。".to_string());
        }
        let count =
            usize::try_from(count).map_err(|_| "下拉框选项数量超出支持范围。".to_string())?;
        let mut options = Vec::with_capacity(count);
        for index in 0..count {
            let length = send_message(combo, CB_GETLBTEXTLEN, index, 0)?;
            if length == CB_ERR as isize || length < 0 {
                return Err(format!("无法读取下拉框第 {} 项的文本长度。", index + 1));
            }
            let length =
                usize::try_from(length).map_err(|_| "下拉选项长度超出支持范围。".to_string())?;
            if length > MAX_OPTION_TEXT_CHARS {
                return Err(format!("下拉框第 {} 项文本过长。", index + 1));
            }
            let mut buffer = vec![0u16; length.saturating_add(1)];
            let copied = send_message(combo, CB_GETLBTEXT, index, buffer.as_mut_ptr() as isize)?;
            if copied == CB_ERR as isize || copied < 0 {
                return Err(format!("无法读取下拉框第 {} 项的文本。", index + 1));
            }
            let copied = usize::try_from(copied).unwrap_or(0).min(length);
            options.push(String::from_utf16_lossy(&buffer[..copied]));
        }
        Ok(options)
    }

    fn read_class(hwnd: HWND) -> Option<String> {
        if hwnd.0.is_null() {
            return None;
        }
        let mut buffer = [0u16; 256];
        let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
        (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
    }

    fn send_message(
        hwnd: HWND,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> Result<isize, String> {
        let mut result = 0usize;
        let sent = unsafe {
            SendMessageTimeoutW(
                hwnd,
                message,
                WPARAM(wparam),
                LPARAM(lparam),
                SMTO_ABORTIFHUNG,
                MESSAGE_TIMEOUT_MS,
                Some(&mut result),
            )
        };
        if sent.0 == 0 {
            Err(format!(
                "目标下拉框没有在 {} 毫秒内响应 Windows 消息：{}",
                MESSAGE_TIMEOUT_MS,
                WindowsError::from_win32()
            ))
        } else {
            Ok(result as isize)
        }
    }

    fn command_wparam(control_id: i32, notification: u32) -> usize {
        ((control_id as u32 & 0xffff) | ((notification & 0xffff) << 16)) as usize
    }

    fn hwnd(handle: WindowHandle) -> HWND {
        HWND(handle.raw() as *mut c_void)
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    pub(super) fn stable_owner_for_popup(
        _popup: WindowHandle,
        _owner_hint: Option<WindowHandle>,
    ) -> Option<WindowHandle> {
        None
    }

    pub(super) fn option_at_point(
        _popup: WindowHandle,
        _owner_hint: Option<WindowHandle>,
        _point: ScreenPoint,
    ) -> Result<Option<CapturedComboOption>, String> {
        Ok(None)
    }

    pub(super) fn select_option(
        _owner: WindowHandle,
        _control_id: i32,
        _control_bounds: ControlBounds,
        _option_name: &str,
    ) -> Result<(), String> {
        Err("标准 Windows 下拉框语义回放仅支持 Windows。".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::choose_option_index;

    #[test]
    fn exact_option_name_wins_over_case_insensitive_matches() {
        let options = vec!["mihomo".to_string(), "Mihomo".to_string()];
        assert_eq!(choose_option_index(&options, "Mihomo").unwrap(), 1);
    }

    #[test]
    fn unique_case_insensitive_option_is_accepted() {
        let options = vec!["以太网".to_string(), "Mihomo".to_string()];
        assert_eq!(choose_option_index(&options, "mihomo").unwrap(), 1);
    }

    #[test]
    fn missing_or_ambiguous_option_is_rejected() {
        let options = vec!["Mihomo".to_string(), "MIHOMO".to_string()];
        assert!(choose_option_index(&options, "mihomo").is_err());
        assert!(choose_option_index(&options, "以太网").is_err());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn captures_and_selects_an_actual_standard_windows_combo_box_by_name() {
        use super::{
            option_at_point, select_option, PointerSemanticAction, ScreenPoint, WindowHandle,
        };
        use std::{ffi::c_void, mem::size_of};
        use windows::{
            core::w,
            Win32::{
                Foundation::{HINSTANCE, HWND, LPARAM, POINT, RECT, WPARAM},
                Graphics::Gdi::ClientToScreen,
                UI::{
                    Controls::{GetComboBoxInfo, COMBOBOXINFO},
                    WindowsAndMessaging::{
                        CreateWindowExW, DestroyWindow, IsWindowVisible, SendMessageW,
                        CBS_DROPDOWNLIST, CB_ADDSTRING, CB_GETCURSEL, CB_SHOWDROPDOWN, HMENU,
                        LB_ERR, LB_GETITEMRECT, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD,
                        WS_EX_NOACTIVATE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
                    },
                },
            },
        };

        struct WindowGuard(HWND);
        impl Drop for WindowGuard {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }

        let owner = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("Remember combo test"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                -32_000,
                -32_000,
                320,
                200,
                HWND::default(),
                HMENU::default(),
                HINSTANCE::default(),
                None,
            )
        }
        .expect("create hidden owner window");
        let owner_guard = WindowGuard(owner);
        let control_id = 71;
        let combo = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("COMBOBOX"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                20,
                30,
                180,
                120,
                owner,
                HMENU(control_id as usize as *mut c_void),
                HINSTANCE::default(),
                None,
            )
        }
        .expect("create standard combo box");
        for option in [w!("以太网"), w!("Mihomo")] {
            let result = unsafe {
                SendMessageW(
                    combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(option.as_ptr() as isize),
                )
            };
            assert!(result.0 >= 0, "add a combo option");
        }

        unsafe {
            SendMessageW(combo, CB_SHOWDROPDOWN, WPARAM(1), LPARAM(0));
        }
        let mut info = COMBOBOXINFO {
            cbSize: size_of::<COMBOBOXINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetComboBoxInfo(combo, &mut info) }.expect("read combo popup handle");
        assert!(unsafe { IsWindowVisible(info.hwndList).as_bool() });
        let mut item_rect = RECT::default();
        let item_result = unsafe {
            SendMessageW(
                info.hwndList,
                LB_GETITEMRECT,
                WPARAM(1),
                LPARAM((&mut item_rect as *mut RECT) as isize),
            )
        };
        assert_ne!(
            item_result.0, LB_ERR as isize,
            "read the Mihomo list-item bounds"
        );
        let mut option_point = POINT {
            x: item_rect.left + 1,
            y: item_rect.top + 1,
        };
        assert!(unsafe { ClientToScreen(info.hwndList, &mut option_point).as_bool() });
        let captured = option_at_point(
            WindowHandle::from_raw(info.hwndList.0 as usize),
            Some(WindowHandle::from_raw(owner.0 as usize)),
            ScreenPoint {
                x: option_point.x,
                y: option_point.y,
            },
        )
        .expect("capture standard combo option")
        .expect("the pointer should hit Mihomo");
        assert_eq!(captured.owner, WindowHandle::from_raw(owner.0 as usize));
        let PointerSemanticAction::SelectComboOption {
            control_id: captured_id,
            control_bounds,
            option_name,
        } = captured.action;
        assert_eq!(captured_id, control_id);
        assert_eq!(option_name, "Mihomo");

        select_option(captured.owner, captured_id, control_bounds, &option_name)
            .expect("select standard combo option by name");
        let selected = unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)) };
        assert_eq!(selected.0, 1);

        drop(owner_guard);
    }
}
