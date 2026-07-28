use crate::{model::Recording, recorder::MAX_RECORDING_STEPS};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const LIBRARY_SUFFIX: &str = ".remember.json";
const MAX_CACHED_RECORDING_LIBRARIES: usize = 8;
pub const MAX_RECORDING_FILE_BYTES: u64 = 64 * 1024 * 1024;
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static RECORDING_LIST_CACHE: OnceLock<Mutex<RecordingListCache>> = OnceLock::new();

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RecordingCacheKey {
    path: PathBuf,
    file_size: u64,
    modified_at: SystemTime,
}

#[derive(Default)]
struct LibraryRecordingCache {
    entries: HashMap<RecordingCacheKey, RecordingFile>,
}

impl LibraryRecordingCache {
    fn resolve<F>(&mut self, key: Option<RecordingCacheKey>, load: F) -> RecordingFile
    where
        F: FnOnce() -> RecordingFile,
    {
        if let Some(key) = key.as_ref() {
            if let Some(file) = self.entries.get(key) {
                return file.clone();
            }
        }

        let file = load();
        if let Some(key) = key {
            self.entries.insert(key, file.clone());
        }
        file
    }

    fn retain_seen(&mut self, seen: &HashSet<RecordingCacheKey>) {
        self.entries.retain(|key, _| seen.contains(key));
    }
}

#[derive(Default)]
struct RecordingListCache {
    libraries: HashMap<PathBuf, LibraryRecordingCache>,
    access_order: VecDeque<PathBuf>,
}

impl RecordingListCache {
    fn library_mut(&mut self, library_path: &Path) -> &mut LibraryRecordingCache {
        self.access_order.retain(|path| path != library_path);
        self.access_order.push_back(library_path.to_path_buf());
        self.libraries
            .entry(library_path.to_path_buf())
            .or_default();

        while self.libraries.len() > MAX_CACHED_RECORDING_LIBRARIES {
            let Some(oldest) = self.access_order.pop_front() else {
                break;
            };
            self.libraries.remove(&oldest);
        }

        self.libraries
            .get_mut(library_path)
            .expect("the current recording library cache must be retained")
    }

    fn remove_library(&mut self, library_path: &Path) {
        self.access_order.retain(|path| path != library_path);
        self.libraries.remove(library_path);
    }

    fn remove_path(&mut self, path: &Path) {
        for library in self.libraries.values_mut() {
            library.entries.retain(|key, _| key.path != path);
        }
    }
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
    validate_recording_for_storage(recording)?;
    let json = serde_json::to_string_pretty(recording).map_err(StorageError::InvalidJson)?;
    ensure_json_size(json.len() as u64)?;
    Ok(json)
}

pub fn recording_from_json(json: &str) -> Result<Recording, StorageError> {
    ensure_json_size(json.len() as u64)?;
    let recording: Recording = serde_json::from_str(json).map_err(StorageError::InvalidJson)?;
    validate_recording_for_storage(&recording)?;
    Ok(recording)
}

pub fn save_recording(path: &Path, recording: &Recording) -> Result<(), StorageError> {
    let temp_path = write_recording_temp(path, recording)?;
    match atomic_replace(&temp_path, path) {
        Ok(()) => {
            invalidate_recording_cache_path(path);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error.into())
        }
    }
}

pub(crate) fn save_recording_without_overwrite(
    path: &Path,
    recording: &Recording,
) -> Result<(), StorageError> {
    let temp_path = write_recording_temp(path, recording)?;
    match atomic_install_without_overwrite(&temp_path, path) {
        Ok(()) => {
            invalidate_recording_cache_path(path);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error.into())
        }
    }
}

pub fn load_recording(path: &Path) -> Result<Recording, StorageError> {
    let file = File::open(path)?;
    ensure_json_size(file.metadata()?.len())?;
    let recording: Recording =
        serde_json::from_reader(BufReader::new(file)).map_err(json_stream_error)?;
    validate_recording_for_storage(&recording)?;
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
            Ok(()) => {
                invalidate_recording_cache_path(&path);
                return Ok(path);
            }
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
    let library_cache_path = complete_cache_path(library_dir);
    if !library_dir.exists() {
        let mut cache = lock_recording_list_cache();
        cache.remove_library(&library_cache_path);
        return Ok(Vec::new());
    }

    let mut cache = lock_recording_list_cache();
    let library_cache = cache.library_mut(&library_cache_path);
    let mut seen_cache_keys = HashSet::new();
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

        let metadata = entry.metadata().ok();
        let modified_at = metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok());
        let updated_at_ms = modified_at.and_then(system_time_ms).unwrap_or(0);
        let cache_key = metadata.as_ref().and_then(|metadata| {
            modified_at.map(|modified_at| RecordingCacheKey {
                path: library_cache_path.join(entry.file_name()),
                file_size: metadata.len(),
                modified_at,
            })
        });
        if let Some(cache_key) = cache_key.as_ref() {
            seen_cache_keys.insert(cache_key.clone());
        }

        let mut file = library_cache.resolve(cache_key, || {
            recording_file_from_path(&path, file_name, updated_at_ms)
        });
        file.path = path.to_string_lossy().to_string();
        files.push(file);
    }
    library_cache.retain_seen(&seen_cache_keys);

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
    invalidate_recording_cache_path(&path);
    Ok(renamed_path)
}

pub fn delete_recording_from_library(library_dir: &Path, path: &Path) -> Result<(), StorageError> {
    let (_, path) = validated_library_recording_path(library_dir, path)?;
    fs::remove_file(&path)?;
    invalidate_recording_cache_path(&path);
    Ok(())
}

fn recording_file_from_path(path: &Path, file_name: &str, updated_at_ms: u64) -> RecordingFile {
    match load_recording(path) {
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
    }
}

fn recording_list_cache() -> &'static Mutex<RecordingListCache> {
    RECORDING_LIST_CACHE.get_or_init(|| Mutex::new(RecordingListCache::default()))
}

fn lock_recording_list_cache() -> std::sync::MutexGuard<'static, RecordingListCache> {
    recording_list_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn complete_cache_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|current_dir| current_dir.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

fn invalidate_recording_cache_path(path: &Path) {
    let Some(cache) = RECORDING_LIST_CACHE.get() else {
        return;
    };
    let path = complete_cache_path(path);
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove_path(&path);
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
    validate_recording_for_storage(recording)?;
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
            let mut writer = SizeLimitedWriter::new(BufWriter::new(file), MAX_RECORDING_FILE_BYTES);
            serde_json::to_writer_pretty(&mut writer, recording).map_err(json_stream_error)?;
            writer.flush()?;
            writer.get_ref().get_ref().sync_all()?;
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        return Ok(temp_path);
    }
}

fn validate_recording_for_storage(recording: &Recording) -> Result<(), StorageError> {
    if recording.steps.len() > MAX_RECORDING_STEPS {
        return Err(StorageError::InvalidRecording(format!(
            "recording exceeds the maximum of {MAX_RECORDING_STEPS} steps"
        )));
    }
    recording.validate().map_err(StorageError::InvalidRecording)
}

fn ensure_json_size(byte_count: u64) -> Result<(), StorageError> {
    if byte_count > MAX_RECORDING_FILE_BYTES {
        return Err(StorageError::InvalidRecording(format!(
            "recording JSON exceeds the maximum size of {MAX_RECORDING_FILE_BYTES} bytes"
        )));
    }
    Ok(())
}

struct SizeLimitedWriter<W> {
    inner: W,
    bytes_written: u64,
    max_bytes: u64,
}

impl<W> SizeLimitedWriter<W> {
    fn new(inner: W, max_bytes: u64) -> Self {
        Self {
            inner,
            bytes_written: 0,
            max_bytes,
        }
    }

    fn get_ref(&self) -> &W {
        &self.inner
    }
}

impl<W: Write> Write for SizeLimitedWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }

        let remaining = self.max_bytes.saturating_sub(self.bytes_written);
        if remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "recording JSON exceeds the maximum size of {} bytes",
                    self.max_bytes
                ),
            ));
        }

        let allowed = buffer.len().min(remaining as usize);
        let written = self.inner.write(&buffer[..allowed])?;
        self.bytes_written = self.bytes_written.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
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
pub(crate) fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    move_file_windows(source, destination, true)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn cached_file(name: &str) -> RecordingFile {
        RecordingFile {
            name: name.to_string(),
            path: "recording.remember.json".to_string(),
            step_count: 0,
            duration_ms: 0,
            created_at: String::new(),
            updated_at_ms: 0,
            load_error: None,
        }
    }

    #[test]
    fn unchanged_signature_reuses_metadata_and_changed_signature_reloads_it() {
        let path = PathBuf::from("C:\\recordings\\recording.remember.json");
        let unchanged_key = RecordingCacheKey {
            path: path.clone(),
            file_size: 100,
            modified_at: UNIX_EPOCH + Duration::from_secs(1),
        };
        let changed_key = RecordingCacheKey {
            path,
            file_size: 101,
            modified_at: UNIX_EPOCH + Duration::from_secs(2),
        };
        let mut cache = LibraryRecordingCache::default();
        let mut loads = 0;

        let first = cache.resolve(Some(unchanged_key.clone()), || {
            loads += 1;
            cached_file("first")
        });
        let cached = cache.resolve(Some(unchanged_key), || {
            loads += 1;
            cached_file("unexpected reload")
        });
        let changed = cache.resolve(Some(changed_key.clone()), || {
            loads += 1;
            cached_file("changed")
        });
        cache.retain_seen(&HashSet::from([changed_key]));

        assert_eq!(first.name, "first");
        assert_eq!(cached.name, "first");
        assert_eq!(changed.name, "changed");
        assert_eq!(loads, 2);
        assert_eq!(cache.entries.len(), 1);
    }
}
