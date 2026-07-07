use remember_lib::model::{KeyState, MacroStep, Recording};
use remember_lib::storage::{
    delete_recording_from_library, list_recordings, load_recording, recording_from_json,
    recording_to_json, save_recording, save_recording_to_library,
};
use std::{
    env, fs, process,
    time::{SystemTime, UNIX_EPOCH},
};

fn sample_recording() -> Recording {
    Recording {
        version: 1,
        name: "notepad smoke".to_string(),
        created_at: "2026-06-29T00:00:00Z".to_string(),
        duration_ms: 120,
        steps: vec![
            MacroStep::Key {
                elapsed_ms: 0,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 120,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Released,
            },
        ],
    }
}

#[test]
fn serializes_recording_with_stable_version() {
    let json = recording_to_json(&sample_recording()).expect("serialize");

    assert!(json.contains("\"version\": 1"));
    assert!(json.contains("\"kind\": \"key\""));
}

#[test]
fn deserializes_round_trip_recording() {
    let original = sample_recording();
    let json = recording_to_json(&original).expect("serialize");
    let loaded = recording_from_json(&json).expect("deserialize");

    assert_eq!(loaded, original);
}

#[test]
fn rejects_unsupported_version() {
    let json = r#"{
      "version": 99,
      "name": "bad",
      "created_at": "2026-06-29T00:00:00Z",
      "duration_ms": 0,
      "steps": []
    }"#;

    let error = recording_from_json(json).expect_err("unsupported version must fail");

    assert!(error.to_string().contains("unsupported recording version"));
}

#[test]
fn rejects_missing_required_fields() {
    let error = recording_from_json(r#"{"version":1}"#).expect_err("missing fields must fail");

    assert!(error.to_string().contains("invalid recording json"));
}

#[test]
fn rejects_step_timestamps_that_move_backward() {
    let mut recording = sample_recording();
    recording.duration_ms = 100;
    recording.steps = vec![
        MacroStep::Key {
            elapsed_ms: 100,
            vk_code: 0x41,
            scan_code: 0x1E,
            state: KeyState::Pressed,
        },
        MacroStep::Key {
            elapsed_ms: 50,
            vk_code: 0x41,
            scan_code: 0x1E,
            state: KeyState::Released,
        },
    ];

    let error = recording_to_json(&recording).expect_err("non-monotonic steps must fail");

    assert!(error
        .to_string()
        .contains("step timestamps must be monotonic"));
}

#[test]
fn saves_and_loads_recording_from_file() {
    let recording = sample_recording();
    let path = env::temp_dir().join(format!(
        "remember-model-storage-{}-save-load.json",
        process::id()
    ));

    save_recording(&path, &recording).expect("save recording");
    let loaded = load_recording(&path).expect("load recording");

    fs::remove_file(&path).expect("clean up temp recording");

    assert_eq!(loaded, recording);
}

#[test]
fn saves_recording_to_library_and_lists_it() {
    let recording = sample_recording();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-library",
        process::id(),
    ));
    let path = save_recording_to_library(&library_dir, &recording).expect("save to library");

    let files = list_recordings(&library_dir).expect("list recordings");

    fs::remove_file(&path).expect("clean up recording");
    fs::remove_dir(&library_dir).expect("clean up library");

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, recording.name);
    assert_eq!(files[0].path, path.to_string_lossy());
    assert_eq!(files[0].step_count, recording.steps.len());
    assert_eq!(files[0].duration_ms, recording.duration_ms);
}

#[test]
fn deletes_recording_from_library() {
    let recording = sample_recording();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-delete",
        process::id(),
    ));
    let path = save_recording_to_library(&library_dir, &recording).expect("save to library");

    delete_recording_from_library(&library_dir, &path).expect("delete from library");
    let files = list_recordings(&library_dir).expect("list recordings");

    fs::remove_dir(&library_dir).expect("clean up library");

    assert!(files.is_empty());
    assert!(!path.exists());
}
