use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const FLOW_DIR_NAME: &str = "flows";
const FLOW_FILE_SUFFIX: &str = ".remember.json";
const DEFAULT_FLOW_FILE_NAME: &str = "daily-report.remember.json";

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
    #[serde(rename = "type")]
    Type {
        id: u32,
        action: String,
        text: String,
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
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSummary {
    pub file_name: String,
    pub name: String,
    pub display_name: String,
    pub step_count: usize,
    pub saved_at: u64,
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

pub fn empty_flow(display_name: &str) -> Flow {
    let normalized_name = slugify_flow_name(display_name);
    Flow {
        version: 1,
        name: normalized_name,
        display_name: display_name.trim().to_string(),
        target_window: TargetWindow {
            title: String::new(),
            process: String::new(),
            size: String::new(),
            matched: false,
        },
        steps: Vec::new(),
    }
}

pub fn ensure_default_flow_in_dir(root: &Path) -> StorageResult<SavedFlow> {
    fs::create_dir_all(flows_dir(root))?;

    if let Some(summary) = list_flow_summaries_in_dir(root)?.into_iter().next() {
        return load_flow_file(root, &summary.file_name);
    }

    save_flow_to_dir(root, &sample_flow())
}

pub fn create_flow_in_dir(root: &Path, display_name: &str) -> StorageResult<SavedFlow> {
    let flow = empty_flow(display_name);
    save_flow_to_dir(root, &flow)
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
            let json = serde_json::to_string_pretty(&copied_flow)?;
            fs::write(&candidate_path, format!("{json}\n"))?;
            return Ok(saved_flow(candidate_file_name, candidate_path, copied_flow));
        }

        candidate_index += 1;
    }
}

pub fn save_flow_to_dir(root: &Path, flow: &Flow) -> StorageResult<SavedFlow> {
    validate_flow(flow)?;
    let flows_dir = flows_dir(root);
    fs::create_dir_all(&flows_dir)?;

    let file_name = flow_file_name(flow);
    let path = flows_dir.join(&file_name);
    let json = serde_json::to_string_pretty(flow)?;
    fs::write(&path, format!("{json}\n"))?;

    Ok(saved_flow(file_name, path, flow.clone()))
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

        let Ok(flow) = load_flow_from_path(&path) else {
            continue;
        };

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        summaries.push(FlowSummary {
            file_name: file_name.to_string(),
            name: flow.name,
            display_name: flow.display_name,
            step_count: flow.steps.len(),
            saved_at: file_modified_unix_seconds(&path),
        });
    }

    summaries.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(summaries)
}

fn validate_flow(flow: &Flow) -> StorageResult<()> {
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
    }

    Ok(())
}

fn flow_step_id(step: &FlowStep) -> u32 {
    match step {
        FlowStep::Click { id, .. }
        | FlowStep::Type { id, .. }
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
        path,
    }
}

fn resolve_flow_file(root: &Path, file_name: &str) -> StorageResult<PathBuf> {
    if file_name.contains('/') || file_name.contains('\\') || !file_name.ends_with(FLOW_FILE_SUFFIX)
    {
        return Err(StorageError::InvalidFileName(file_name.to_string()));
    }
    Ok(flows_dir(root).join(file_name))
}

fn flow_file_name(flow: &Flow) -> String {
    format!("{}{}", slugify_flow_name(&flow.name), FLOW_FILE_SUFFIX)
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

#[allow(dead_code)]
pub fn default_flow_file_name() -> &'static str {
    DEFAULT_FLOW_FILE_NAME
}
