use crate::model::Recording;
use std::{fs, path::Path};
use thiserror::Error;

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
    fs::write(path, json)?;
    Ok(())
}

pub fn load_recording(path: &Path) -> Result<Recording, StorageError> {
    let json = fs::read_to_string(path)?;
    recording_from_json(&json)
}
