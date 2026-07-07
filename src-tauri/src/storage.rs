use crate::model::Recording;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const LIBRARY_SUFFIX: &str = ".remember.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordingFile {
    pub name: String,
    pub path: String,
    pub step_count: usize,
    pub duration_ms: u64,
    pub created_at: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("invalid recording json: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("{0}")]
    InvalidRecording(String),
    #[error("file error: {0}")]
    File(#[from] std::io::Error),
}

pub fn recording_to_json(recording: &Recording) -> Result<String, StorageError> {
    recording
        .validate()
        .map_err(StorageError::InvalidRecording)?;
    serde_json::to_string_pretty(recording).map_err(StorageError::InvalidJson)
}

pub fn recording_from_json(json: &str) -> Result<Recording, StorageError> {
    let recording: Recording = serde_json::from_str(json).map_err(StorageError::InvalidJson)?;
    recording
        .validate()
        .map_err(StorageError::InvalidRecording)?;
    Ok(recording)
}

pub fn save_recording(path: &Path, recording: &Recording) -> Result<(), StorageError> {
    let json = recording_to_json(recording)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(path, json)?;
    Ok(())
}

pub fn load_recording(path: &Path) -> Result<Recording, StorageError> {
    let json = fs::read_to_string(path)?;
    recording_from_json(&json)
}

pub fn save_recording_to_library(
    library_dir: &Path,
    recording: &Recording,
) -> Result<PathBuf, StorageError> {
    fs::create_dir_all(library_dir)?;
    let path = unique_library_path(library_dir, &sanitize_recording_name(&recording.name));
    save_recording(&path, recording)?;
    Ok(path)
}

pub fn list_recordings(library_dir: &Path) -> Result<Vec<RecordingFile>, StorageError> {
    if !library_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(library_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(LIBRARY_SUFFIX) {
            continue;
        }

        let Ok(recording) = load_recording(&path) else {
            continue;
        };
        let updated_at_ms = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(system_time_ms)
            .unwrap_or(0);

        files.push(RecordingFile {
            name: recording.name,
            path: path.to_string_lossy().to_string(),
            step_count: recording.steps.len(),
            duration_ms: recording.duration_ms,
            created_at: recording.created_at,
            updated_at_ms,
        });
    }

    files.sort_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(files)
}

pub fn delete_recording_from_library(library_dir: &Path, path: &Path) -> Result<(), StorageError> {
    let library_dir = fs::canonicalize(library_dir)?;
    let path = fs::canonicalize(path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if !path.starts_with(&library_dir) || !file_name.ends_with(LIBRARY_SUFFIX) {
        return Err(StorageError::InvalidRecording(
            "recording path is outside the library".to_string(),
        ));
    }

    fs::remove_file(path)?;
    Ok(())
}

fn unique_library_path(library_dir: &Path, base_name: &str) -> PathBuf {
    let mut index = 0;
    loop {
        let file_name = if index == 0 {
            format!("{base_name}{LIBRARY_SUFFIX}")
        } else {
            format!("{base_name}-{index}{LIBRARY_SUFFIX}")
        };
        let path = library_dir.join(file_name);
        if !path.exists() {
            return path;
        }
        index += 1;
    }
}

fn sanitize_recording_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "recording".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn system_time_ms(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}
