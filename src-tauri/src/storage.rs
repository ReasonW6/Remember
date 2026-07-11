use crate::model::Recording;
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const LIBRARY_SUFFIX: &str = ".remember.json";
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordingFile {
    pub name: String,
    pub path: String,
    pub step_count: usize,
    pub duration_ms: u64,
    pub created_at: String,
    pub updated_at_ms: u64,
    pub load_error: Option<String>,
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
    let temp_path = write_recording_temp(path, recording)?;
    match atomic_replace(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error.into())
        }
    }
}

pub fn load_recording(path: &Path) -> Result<Recording, StorageError> {
    let file = File::open(path)?;
    let recording: Recording =
        serde_json::from_reader(BufReader::new(file)).map_err(json_stream_error)?;
    recording
        .validate()
        .map_err(StorageError::InvalidRecording)?;
    Ok(recording)
}

pub fn save_recording_to_library(
    library_dir: &Path,
    recording: &Recording,
) -> Result<PathBuf, StorageError> {
    fs::create_dir_all(library_dir)?;
    let base_name = sanitize_recording_name(&recording.name);
    let first_path = library_path(library_dir, &base_name, 0);
    let temp_path = write_recording_temp(&first_path, recording)?;
    let mut index = 0;

    loop {
        let path = library_path(library_dir, &base_name, index);
        match atomic_install_without_overwrite(&temp_path, &path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                index += 1;
            }
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                return Err(error.into());
            }
        }
    }
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

        let updated_at_ms = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(system_time_ms)
            .unwrap_or(0);

        let file = match load_recording(&path) {
            Ok(recording) => RecordingFile {
                name: recording.name,
                path: path.to_string_lossy().to_string(),
                step_count: recording.steps.len(),
                duration_ms: recording.duration_ms,
                created_at: recording.created_at,
                updated_at_ms,
                load_error: None,
            },
            Err(error) => RecordingFile {
                name: file_name
                    .strip_suffix(LIBRARY_SUFFIX)
                    .unwrap_or(file_name)
                    .to_string(),
                path: path.to_string_lossy().to_string(),
                step_count: 0,
                duration_ms: 0,
                created_at: String::new(),
                updated_at_ms,
                load_error: Some(error.to_string()),
            },
        };
        files.push(file);
    }

    files.sort_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(files)
}

pub fn rename_recording_in_library(
    library_dir: &Path,
    path: &Path,
    new_name: &str,
) -> Result<PathBuf, StorageError> {
    let destination_library_dir = library_dir.to_path_buf();
    let (_, path) = validated_library_recording_path(library_dir, path)?;
    let source_result_path = destination_library_dir.join(path.file_name().ok_or_else(|| {
        StorageError::InvalidRecording("recording path is outside the library".to_string())
    })?);
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err(StorageError::InvalidRecording(
            "recording name cannot be empty".to_string(),
        ));
    }

    let mut recording = load_recording(&path)?;
    if recording.name == new_name {
        return Ok(source_result_path);
    }
    recording.name = new_name.to_string();

    let current_base = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(LIBRARY_SUFFIX))
        .unwrap_or_default();
    let new_base = sanitize_recording_name(new_name);
    if current_base.eq_ignore_ascii_case(&new_base) {
        save_recording(&path, &recording)?;
        return Ok(source_result_path);
    }

    let renamed_path = save_recording_to_library(&destination_library_dir, &recording)?;
    if let Err(error) = fs::remove_file(&path) {
        let _ = fs::remove_file(&renamed_path);
        return Err(error.into());
    }
    Ok(renamed_path)
}

pub fn delete_recording_from_library(library_dir: &Path, path: &Path) -> Result<(), StorageError> {
    let (_, path) = validated_library_recording_path(library_dir, path)?;
    fs::remove_file(path)?;
    Ok(())
}

fn validated_library_recording_path(
    library_dir: &Path,
    path: &Path,
) -> Result<(PathBuf, PathBuf), StorageError> {
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

    Ok((library_dir, path))
}

fn write_recording_temp(path: &Path, recording: &Recording) -> Result<PathBuf, StorageError> {
    recording
        .validate()
        .map_err(StorageError::InvalidRecording)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let destination_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("recording");

    loop {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{destination_name}.{}.{}.tmp",
            process::id(),
            sequence
        ));
        let file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let write_result = (|| -> Result<(), StorageError> {
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, recording).map_err(json_stream_error)?;
            writer.flush()?;
            writer.get_ref().sync_all()?;
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        return Ok(temp_path);
    }
}

fn json_stream_error(error: serde_json::Error) -> StorageError {
    match error.io_error_kind() {
        Some(kind) => StorageError::File(io::Error::new(kind, error)),
        None => StorageError::InvalidJson(error),
    }
}

fn library_path(library_dir: &Path, base_name: &str, index: usize) -> PathBuf {
    let file_name = if index == 0 {
        format!("{base_name}{LIBRARY_SUFFIX}")
    } else {
        format!("{base_name}-{index}{LIBRARY_SUFFIX}")
    };
    library_dir.join(file_name)
}

#[cfg(target_os = "windows")]
#[link(name = "Kernel32")]
unsafe extern "system" {
    #[link_name = "MoveFileExW"]
    fn move_file_ex_w(existing_file_name: *const u16, new_file_name: *const u16, flags: u32)
        -> i32;
}

#[cfg(target_os = "windows")]
fn move_file_windows(source: &Path, destination: &Path, replace: bool) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    // SAFETY: Both paths are owned, NUL-terminated UTF-16 buffers that live for the call.
    let result = unsafe { move_file_ex_w(source.as_ptr(), destination.as_ptr(), flags) };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    move_file_windows(source, destination, true)
}

#[cfg(not(target_os = "windows"))]
fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(target_os = "windows")]
fn atomic_install_without_overwrite(source: &Path, destination: &Path) -> std::io::Result<()> {
    move_file_windows(source, destination, false)
}

#[cfg(not(target_os = "windows"))]
fn atomic_install_without_overwrite(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::hard_link(source, destination)?;
    if let Err(error) = fs::remove_file(source) {
        let _ = fs::remove_file(destination);
        return Err(error);
    }
    Ok(())
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
