use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt, fs,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const FLOW_DIR_NAME: &str = "flows";
const FLOW_FILE_SUFFIX: &str = ".remember.json";
const CLICK_ACTIONS: &[&str] = &["左键单击", "右键单击", "双击"];
const DRAG_ACTIONS: &[&str] = &["左键拖拽", "右键拖拽"];
const MAX_DRAG_PATH_POINTS: usize = 512;
const MAX_STEP_TIMING_MS: u64 = 300_000;
const RESERVED_WINDOWS_FILE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidFileName(String),
    InvalidFlow(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "storage io error: {error}"),
            Self::Json(error) => write!(formatter, "flow json error: {error}"),
            Self::InvalidFileName(file_name) => {
                write!(formatter, "invalid flow file name: {file_name}")
            }
            Self::InvalidFlow(message) => write!(formatter, "invalid flow: {message}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<io::Error> for StorageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub version: u8,
    pub name: String,
    pub display_name: String,
    pub target_window: TargetWindow,
    pub steps: Vec<FlowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TargetWindow {
    pub title: String,
    pub process: String,
    pub size: String,
    pub matched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DragPathPoint {
    pub x: i32,
    pub y: i32,
    #[serde(rename = "elapsedMs")]
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FlowStep {
    #[serde(rename = "click")]
    Click {
        id: u32,
        action: String,
        target: String,
        x: i32,
        y: i32,
        #[serde(rename = "delayMs")]
        delay_ms: u64,
        note: String,
    },
    #[serde(rename = "drag")]
    Drag {
        id: u32,
        action: String,
        target: String,
        #[serde(rename = "startX")]
        start_x: i32,
        #[serde(rename = "startY")]
        start_y: i32,
        #[serde(rename = "endX")]
        end_x: i32,
        #[serde(rename = "endY")]
        end_y: i32,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
        #[serde(rename = "delayMs")]
        delay_ms: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path: Vec<DragPathPoint>,
        note: String,
    },
    #[serde(rename = "type")]
    Type {
        id: u32,
        action: String,
        text: String,
        #[serde(rename = "delayMs")]
        delay_ms: u64,
        note: String,
    },
    #[serde(rename = "key")]
    Key {
        id: u32,
        action: String,
        key: String,
        #[serde(rename = "delayMs")]
        delay_ms: u64,
        note: String,
    },
    #[serde(rename = "wait")]
    Wait {
        id: u32,
        action: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
        #[serde(rename = "delayMs")]
        delay_ms: u64,
        note: String,
    },
    #[serde(rename = "hotkey")]
    Hotkey {
        id: u32,
        action: String,
        keys: Vec<String>,
        #[serde(rename = "delayMs")]
        delay_ms: u64,
        note: String,
    },
    #[serde(rename = "scroll")]
    Scroll {
        id: u32,
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<i32>,
        #[serde(rename = "deltaX")]
        delta_x: i32,
        #[serde(rename = "deltaY")]
        delta_y: i32,
        #[serde(rename = "delayMs")]
        delay_ms: u64,
        note: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFlow {
    pub file_name: String,
    pub saved_at: u64,
    pub flow: Flow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSummary {
    pub file_name: String,
    pub name: String,
    pub display_name: String,
    pub step_count: usize,
    pub saved_at: u64,
    pub is_valid: bool,
    pub error: Option<String>,
}

pub fn sample_flow() -> Flow {
    Flow {
        version: 1,
        name: "daily-report".to_string(),
        display_name: "Daily Report 自动化".to_string(),
        target_window: TargetWindow {
            title: "Sales Report - Excel".to_string(),
            process: "EXCEL.EXE".to_string(),
            size: "1920 x 1080".to_string(),
            matched: true,
        },
        steps: vec![
            FlowStep::Click {
                id: 1,
                action: "左键单击".to_string(),
                target: "(120, 240) [屏幕绝对]".to_string(),
                x: 120,
                y: 240,
                delay_ms: 200,
                note: "打开菜单".to_string(),
            },
            FlowStep::Type {
                id: 2,
                action: "文本输入".to_string(),
                text: "Daily Report".to_string(),
                delay_ms: 300,
                note: "输入标题".to_string(),
            },
            FlowStep::Wait {
                id: 3,
                action: "等待".to_string(),
                duration_ms: 2000,
                delay_ms: 2000,
                note: "等待页面加载".to_string(),
            },
            FlowStep::Click {
                id: 4,
                action: "左键单击".to_string(),
                target: "(540, 320) [导出按钮]".to_string(),
                x: 540,
                y: 320,
                delay_ms: 200,
                note: "点击导出".to_string(),
            },
            FlowStep::Type {
                id: 5,
                action: "文本输入".to_string(),
                text: "=TODAY(yyyy-mm-dd)".to_string(),
                delay_ms: 300,
                note: "文件名".to_string(),
            },
            FlowStep::Wait {
                id: 6,
                action: "等待".to_string(),
                duration_ms: 1000,
                delay_ms: 1000,
                note: "等待保存完成".to_string(),
            },
            FlowStep::Hotkey {
                id: 7,
                action: "快捷键".to_string(),
                keys: vec!["Ctrl".to_string(), "S".to_string()],
                delay_ms: 100,
                note: "保存文件".to_string(),
            },
            FlowStep::Wait {
                id: 8,
                action: "等待".to_string(),
                duration_ms: 500,
                delay_ms: 500,
                note: "短暂等待".to_string(),
            },
        ],
    }
}

pub fn empty_flow() -> Flow {
    Flow {
        version: 1,
        name: "untitled-flow".to_string(),
        display_name: "未命名流程".to_string(),
        target_window: TargetWindow {
            title: "尚未捕获活动窗口".to_string(),
            process: "N/A".to_string(),
            size: "N/A".to_string(),
            matched: false,
        },
        steps: Vec::new(),
    }
}

pub fn initial_flow_in_dir(root: &Path) -> StorageResult<SavedFlow> {
    remove_legacy_default_seed_files(root)?;

    if let Some(summary) = list_flow_summaries_in_dir(root)?
        .into_iter()
        .find(|summary| summary.is_valid)
    {
        return load_flow_file(root, &summary.file_name);
    }

    Ok(SavedFlow {
        file_name: "untitled-flow.remember.json".to_string(),
        saved_at: 0,
        flow: empty_flow(),
    })
}

pub fn ensure_default_flow_in_dir(root: &Path) -> StorageResult<SavedFlow> {
    fs::create_dir_all(flows_dir(root))?;

    if let Some(summary) = list_flow_summaries_in_dir(root)?
        .into_iter()
        .find(|summary| summary.is_valid)
    {
        return load_flow_file(root, &summary.file_name);
    }

    save_flow_to_dir(root, &sample_flow())
}

fn remove_legacy_default_seed_files(root: &Path) -> StorageResult<()> {
    let flows_dir = flows_dir(root);
    if !flows_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(flows_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !is_remember_flow_path(&path) {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(flow) = load_flow_from_path(&path) else {
            continue;
        };

        if is_legacy_default_seed_flow(file_name, &flow) {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

pub fn save_flow_as_to_dir(
    root: &Path,
    source_flow: &Flow,
    display_name: &str,
) -> StorageResult<SavedFlow> {
    let base_display_name = normalize_display_name(display_name);
    let base_name = slugify_flow_name(&base_display_name);
    let flows_dir = flows_dir(root);
    fs::create_dir_all(&flows_dir)?;

    let mut candidate_index = 1;
    loop {
        let candidate_name = if candidate_index == 1 {
            base_name.clone()
        } else {
            format!("{base_name}-{candidate_index}")
        };
        let candidate_file_name = format!("{candidate_name}{FLOW_FILE_SUFFIX}");
        let candidate_path = flows_dir.join(&candidate_file_name);

        if !candidate_path.exists() {
            let mut copied_flow = source_flow.clone();
            copied_flow.name = candidate_name;
            copied_flow.display_name = if candidate_index == 1 {
                base_display_name
            } else {
                format!("{base_display_name} {candidate_index}")
            };
            validate_flow(&copied_flow)?;
            write_flow_atomically(&candidate_path, &copied_flow)?;
            return Ok(saved_flow(candidate_file_name, candidate_path, copied_flow));
        }

        candidate_index += 1;
    }
}

pub fn save_flow_to_dir(root: &Path, flow: &Flow) -> StorageResult<SavedFlow> {
    let file_name = flow_file_name(flow);
    save_flow_file_to_dir(root, &file_name, flow)
}

pub fn save_flow_file_to_dir(
    root: &Path,
    file_name: &str,
    flow: &Flow,
) -> StorageResult<SavedFlow> {
    validate_flow(flow)?;
    let flows_dir = flows_dir(root);
    fs::create_dir_all(&flows_dir)?;

    let path = resolve_flow_file(root, file_name)?;
    write_flow_atomically(&path, flow)?;

    Ok(saved_flow(file_name.to_string(), path, flow.clone()))
}

pub fn load_flow_file(root: &Path, file_name: &str) -> StorageResult<SavedFlow> {
    let path = resolve_flow_file(root, file_name)?;
    let flow = load_flow_from_path(&path)?;
    Ok(saved_flow(file_name.to_string(), path, flow))
}

pub fn load_flow_from_path(path: &Path) -> StorageResult<Flow> {
    let raw = fs::read_to_string(path)?;
    let flow: Flow = serde_json::from_str(&raw)?;
    validate_flow(&flow)?;
    Ok(flow)
}

pub fn list_flow_summaries_in_dir(root: &Path) -> StorageResult<Vec<FlowSummary>> {
    let flows_dir = flows_dir(root);
    if !flows_dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();
    for entry in fs::read_dir(flows_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !is_remember_flow_path(&path) {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        match load_flow_from_path(&path) {
            Ok(flow) => summaries.push(FlowSummary {
                file_name: file_name.to_string(),
                name: flow.name,
                display_name: flow.display_name,
                step_count: flow.steps.len(),
                saved_at: file_modified_unix_seconds(&path),
                is_valid: true,
                error: None,
            }),
            Err(error) => summaries.push(FlowSummary {
                file_name: file_name.to_string(),
                name: display_name_from_file_name(file_name),
                display_name: format!("{} (无法读取)", display_name_from_file_name(file_name)),
                step_count: 0,
                saved_at: file_modified_unix_seconds(&path),
                is_valid: false,
                error: Some(error.to_string()),
            }),
        }
    }

    summaries.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(summaries)
}

pub fn validate_flow(flow: &Flow) -> StorageResult<()> {
    if flow.version != 1 {
        return Err(StorageError::InvalidFlow(format!(
            "unsupported version {}",
            flow.version
        )));
    }
    if flow.name.trim().is_empty() {
        return Err(StorageError::InvalidFlow("name is required".to_string()));
    }
    if flow.display_name.trim().is_empty() {
        return Err(StorageError::InvalidFlow(
            "displayName is required".to_string(),
        ));
    }
    validate_steps(&flow.steps)?;
    Ok(())
}

fn validate_steps(steps: &[FlowStep]) -> StorageResult<()> {
    let mut seen_step_ids = HashSet::new();
    for step in steps {
        let step_id = flow_step_id(step);
        if !seen_step_ids.insert(step_id) {
            return Err(StorageError::InvalidFlow(format!(
                "duplicate step id {step_id}"
            )));
        }

        if let FlowStep::Click { id, action, .. } = step {
            if !CLICK_ACTIONS.contains(&action.as_str()) {
                return Err(StorageError::InvalidFlow(format!(
                    "click step {id} has an unsupported action"
                )));
            }
        }

        if let FlowStep::Drag { id, action, .. } = step {
            if !DRAG_ACTIONS.contains(&action.as_str()) {
                return Err(StorageError::InvalidFlow(format!(
                    "drag step {id} has an unsupported action"
                )));
            }
        }

        validate_step_metadata(step_id, step)?;
        validate_step_timing(step_id, step)?;

        if let FlowStep::Wait {
            id,
            duration_ms,
            delay_ms,
            ..
        } = step
        {
            if duration_ms != delay_ms {
                return Err(StorageError::InvalidFlow(format!(
                    "wait step {id} has mismatched durationMs and delayMs"
                )));
            }
        }

        if let FlowStep::Key { id, key, .. } = step {
            if key.trim().is_empty() {
                return Err(StorageError::InvalidFlow(format!(
                    "key step {id} key is required"
                )));
            }
            if !key_is_allowed(key) {
                return Err(StorageError::InvalidFlow(format!(
                    "key step {id} is not allowed because it can trigger global system behavior"
                )));
            }
        }

        if let FlowStep::Type { id, text, .. } = step {
            if type_text_looks_sensitive(text) {
                return Err(StorageError::InvalidFlow(format!(
                    "type step {id} contains sensitive text and must be removed or redacted before saving"
                )));
            }
        }

        if let FlowStep::Hotkey { id, keys, .. } = step {
            if keys.is_empty() {
                return Err(StorageError::InvalidFlow(format!(
                    "hotkey step {id} needs at least one key"
                )));
            }
            if keys.iter().any(|key| key.trim().is_empty()) {
                return Err(StorageError::InvalidFlow(format!(
                    "hotkey step {id} contains an empty key"
                )));
            }
            if !hotkey_is_allowed(keys) {
                return Err(StorageError::InvalidFlow(format!(
                    "hotkey step {id} is not allowed because it can trigger global system behavior"
                )));
            }
        }
    }

    Ok(())
}

fn validate_step_metadata(step_id: u32, step: &FlowStep) -> StorageResult<()> {
    match step {
        FlowStep::Click { target, note, .. } | FlowStep::Drag { target, note, .. } => {
            validate_non_sensitive_text(&format!("step {step_id} target"), target)?;
            validate_non_sensitive_text(&format!("step {step_id} note"), note)?;
        }
        FlowStep::Type { note, .. }
        | FlowStep::Key { note, .. }
        | FlowStep::Wait { note, .. }
        | FlowStep::Hotkey { note, .. }
        | FlowStep::Scroll { note, .. } => {
            validate_non_sensitive_text(&format!("step {step_id} note"), note)?;
        }
    }
    Ok(())
}

fn validate_step_timing(step_id: u32, step: &FlowStep) -> StorageResult<()> {
    match step {
        FlowStep::Click { delay_ms, .. }
        | FlowStep::Type { delay_ms, .. }
        | FlowStep::Key { delay_ms, .. }
        | FlowStep::Hotkey { delay_ms, .. }
        | FlowStep::Scroll { delay_ms, .. } => {
            validate_max_timing(step_id, step_kind(step), "delayMs", *delay_ms)?;
        }
        FlowStep::Wait {
            duration_ms,
            delay_ms,
            ..
        } => {
            validate_max_timing(step_id, "wait", "durationMs", *duration_ms)?;
            validate_max_timing(step_id, "wait", "delayMs", *delay_ms)?;
        }
        FlowStep::Drag {
            duration_ms,
            delay_ms,
            path,
            ..
        } => {
            validate_max_timing(step_id, "drag", "durationMs", *duration_ms)?;
            validate_max_timing(step_id, "drag", "delayMs", *delay_ms)?;
            validate_drag_path(step_id, *duration_ms, path)?;
        }
    }
    Ok(())
}

fn validate_drag_path(step_id: u32, duration_ms: u64, path: &[DragPathPoint]) -> StorageResult<()> {
    if path.is_empty() {
        return Ok(());
    }
    if path.len() < 2 {
        return Err(StorageError::InvalidFlow(format!(
            "drag step {step_id} path needs at least two points"
        )));
    }
    if path.len() > MAX_DRAG_PATH_POINTS {
        return Err(StorageError::InvalidFlow(format!(
            "drag step {step_id} path exceeds {MAX_DRAG_PATH_POINTS} points"
        )));
    }

    let mut previous_elapsed_ms = 0;
    for (index, point) in path.iter().enumerate() {
        if point.elapsed_ms > duration_ms {
            return Err(StorageError::InvalidFlow(format!(
                "drag step {step_id} path point {} exceeds durationMs",
                index + 1
            )));
        }
        if index > 0 && point.elapsed_ms < previous_elapsed_ms {
            return Err(StorageError::InvalidFlow(format!(
                "drag step {step_id} path timing must be ordered"
            )));
        }
        previous_elapsed_ms = point.elapsed_ms;
    }

    Ok(())
}

fn validate_max_timing(
    step_id: u32,
    step_kind: &str,
    field_name: &str,
    value: u64,
) -> StorageResult<()> {
    if value > MAX_STEP_TIMING_MS {
        return Err(StorageError::InvalidFlow(format!(
            "{step_kind} step {step_id} {field_name} exceeds {MAX_STEP_TIMING_MS}ms"
        )));
    }
    Ok(())
}

fn step_kind(step: &FlowStep) -> &'static str {
    match step {
        FlowStep::Click { .. } => "click",
        FlowStep::Drag { .. } => "drag",
        FlowStep::Type { .. } => "type",
        FlowStep::Key { .. } => "key",
        FlowStep::Wait { .. } => "wait",
        FlowStep::Hotkey { .. } => "hotkey",
        FlowStep::Scroll { .. } => "scroll",
    }
}

fn validate_non_sensitive_text(field_name: &str, value: &str) -> StorageResult<()> {
    if type_text_looks_sensitive(value) {
        return Err(StorageError::InvalidFlow(format!(
            "{field_name} contains sensitive text"
        )));
    }
    Ok(())
}

pub fn hotkey_is_allowed(keys: &[String]) -> bool {
    let normalized = keys
        .iter()
        .map(|key| normalize_key_name(key))
        .collect::<HashSet<_>>();

    if normalized
        .iter()
        .any(|key| key == "WIN" || key == "WINDOWS")
    {
        return false;
    }

    if normalized.contains("ALT") && (normalized.contains("F4") || normalized.contains("TAB")) {
        return false;
    }

    if normalized.contains("ALT") && normalized.contains("ESC") {
        return false;
    }

    if normalized.contains("CTRL") && normalized.contains("ESC") {
        return false;
    }

    if normalized.contains("CTRL") && normalized.contains("ALT") && normalized.contains("DELETE") {
        return false;
    }

    if normalized.contains("CTRL") && normalized.contains("ALT") && normalized.contains("S") {
        return false;
    }

    true
}

fn key_is_allowed(key: &str) -> bool {
    !matches!(
        normalize_key_name(key).as_str(),
        "WIN" | "WINDOWS" | "CTRL" | "ALT" | "SHIFT"
    )
}

fn type_text_looks_sensitive(text: &str) -> bool {
    let normalized = text.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    if [
        "password",
        "passcode",
        "verification code",
        "one-time code",
        "otp",
        "2fa",
        "credit card",
        "card number",
        "security code",
        "cvv",
        "cvc",
        "api key",
        "access token",
        "secret",
        "private key",
        "密码",
        "口令",
        "验证码",
        "动态码",
        "银行卡",
        "信用卡",
        "支付密码",
        "密钥",
        "令牌",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return true;
    }

    let digits = normalized
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    if (13..=19).contains(&digits.len()) && luhn_checksum_is_valid(&digits) {
        return true;
    }

    false
}

fn luhn_checksum_is_valid(digits: &str) -> bool {
    let mut sum = 0;
    let mut double = false;
    for digit in digits.bytes().rev() {
        let mut value = (digit - b'0') as u32;
        if double {
            value *= 2;
            if value > 9 {
                value -= 9;
            }
        }
        sum += value;
        double = !double;
    }
    sum % 10 == 0
}

fn normalize_key_name(key: &str) -> String {
    match key.trim().to_ascii_uppercase().as_str() {
        "CONTROL" => "CTRL".to_string(),
        "ESCAPE" => "ESC".to_string(),
        "DEL" => "DELETE".to_string(),
        value => value.to_string(),
    }
}

fn flow_step_id(step: &FlowStep) -> u32 {
    match step {
        FlowStep::Click { id, .. }
        | FlowStep::Drag { id, .. }
        | FlowStep::Type { id, .. }
        | FlowStep::Key { id, .. }
        | FlowStep::Wait { id, .. }
        | FlowStep::Hotkey { id, .. }
        | FlowStep::Scroll { id, .. } => *id,
    }
}

fn saved_flow(file_name: String, path: PathBuf, flow: Flow) -> SavedFlow {
    let saved_at = file_modified_unix_seconds(&path);
    SavedFlow {
        file_name,
        saved_at,
        flow,
    }
}

fn resolve_flow_file(root: &Path, file_name: &str) -> StorageResult<PathBuf> {
    if !flow_file_name_is_allowed(file_name) {
        return Err(StorageError::InvalidFileName(file_name.to_string()));
    }
    Ok(flows_dir(root).join(file_name))
}

fn flow_file_name_is_allowed(file_name: &str) -> bool {
    let Some(stem) = file_name.strip_suffix(FLOW_FILE_SUFFIX) else {
        return false;
    };
    if stem.is_empty()
        || RESERVED_WINDOWS_FILE_NAMES
            .iter()
            .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return false;
    }

    stem.chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
}

fn flow_file_name(flow: &Flow) -> String {
    format!("{}{}", slugify_flow_name(&flow.name), FLOW_FILE_SUFFIX)
}

fn display_name_from_file_name(file_name: &str) -> String {
    file_name
        .strip_suffix(FLOW_FILE_SUFFIX)
        .unwrap_or(file_name)
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn slugify_flow_name(value: &str) -> String {
    let slug = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else if character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if slug.is_empty() {
        "untitled-flow".to_string()
    } else {
        slug
    }
}

fn normalize_display_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "Untitled Flow".to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_legacy_default_seed_flow(file_name: &str, flow: &Flow) -> bool {
    if file_name != "daily-report.remember.json" || flow.name != "daily-report" {
        return false;
    }

    let has_seed_display_name =
        flow.display_name == "Daily Report 自动化" || flow.display_name.starts_with("Phase1 ");
    let has_seed_target = flow.target_window.title == "Sales Report - Excel"
        && flow.target_window.process.eq_ignore_ascii_case("EXCEL.EXE");

    has_seed_display_name && has_seed_target && (flow.steps.is_empty() || *flow == sample_flow())
}

fn flows_dir(root: &Path) -> PathBuf {
    root.join(FLOW_DIR_NAME)
}

fn is_remember_flow_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(FLOW_FILE_SUFFIX))
}

fn file_modified_unix_seconds(path: &Path) -> u64 {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .unwrap_or_else(|_| SystemTime::now())
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn write_flow_atomically(path: &Path, flow: &Flow) -> StorageResult<()> {
    let json = serde_json::to_string_pretty(flow)?;
    let temp_path = temporary_flow_path(path)?;

    let write_result = (|| -> io::Result<()> {
        let mut file = File::create_new(&temp_path)?;
        file.write_all(format!("{json}\n").as_bytes())?;
        file.sync_all()?;
        replace_file(&temp_path, path)
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(StorageError::Io(error));
    }

    Ok(())
}

fn temporary_flow_path(path: &Path) -> StorageResult<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| StorageError::InvalidFileName(path.to_string_lossy().to_string()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| StorageError::InvalidFileName(path.to_string_lossy().to_string()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), stamp)))
}

#[cfg(target_os = "windows")]
fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    fn wide_path(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let temp = wide_path(temp_path);
    let destination = wide_path(path);
    let moved = unsafe {
        MoveFileExW(
            temp.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    fs::rename(temp_path, path)
}
