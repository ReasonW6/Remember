use remember_lib::model::{KeyState, MacroStep, Recording};
use remember_lib::storage::{
    delete_recording_from_library, list_recordings, load_recording, recording_from_json,
    recording_to_json, rename_recording_in_library, save_recording, save_recording_to_library,
};
use std::{
    env, fs, process,
    sync::{Arc, Barrier},
    thread,
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
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 120,
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
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
            extended: false,
            state: KeyState::Pressed,
        },
        MacroStep::Key {
            elapsed_ms: 50,
            vk_code: 0x41,
            scan_code: 0x1E,
            extended: false,
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

    fs::write(&path, "incomplete previous contents").expect("seed existing recording path");
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
    assert_eq!(files[0].load_error, None);
}

#[test]
fn renames_library_file_and_embedded_recording_name() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-rename",
        process::id(),
    ));
    let path =
        save_recording_to_library(&library_dir, &sample_recording()).expect("save to library");

    let renamed_path =
        rename_recording_in_library(&library_dir, &path, "renamed recording").expect("rename");
    let renamed = load_recording(&renamed_path).expect("load renamed recording");
    let files = list_recordings(&library_dir).expect("list renamed recording");

    assert!(!path.exists());
    assert!(renamed_path.ends_with("renamed-recording.remember.json"));
    assert_eq!(renamed.name, "renamed recording");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "renamed recording");
    assert_eq!(files[0].path, renamed_path.to_string_lossy());

    fs::remove_file(&renamed_path).expect("clean up renamed recording");
    fs::remove_dir(&library_dir).expect("clean up library");
}

#[test]
fn rename_uses_a_unique_path_without_overwriting_an_existing_recording() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-rename-collision",
        process::id(),
    ));
    let mut target = sample_recording();
    target.name = "target".to_string();
    let target_path =
        save_recording_to_library(&library_dir, &target).expect("save target recording");
    let mut source = sample_recording();
    source.name = "source".to_string();
    let source_path =
        save_recording_to_library(&library_dir, &source).expect("save source recording");

    let renamed_path =
        rename_recording_in_library(&library_dir, &source_path, "target").expect("rename");

    assert!(target_path.exists());
    assert!(!source_path.exists());
    assert_ne!(renamed_path, target_path);
    assert_eq!(
        load_recording(&target_path).expect("load original target"),
        target
    );
    assert_eq!(
        load_recording(&renamed_path)
            .expect("load renamed source")
            .name,
        "target"
    );

    fs::remove_file(&target_path).expect("clean up target recording");
    fs::remove_file(&renamed_path).expect("clean up renamed recording");
    fs::remove_dir(&library_dir).expect("clean up library");
}

#[test]
fn lists_corrupt_recording_files_with_a_load_error() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-corrupt",
        process::id(),
    ));
    fs::create_dir_all(&library_dir).expect("create library");
    let path = library_dir.join("broken.remember.json");
    fs::write(&path, "not valid json").expect("write corrupt recording");

    let files = list_recordings(&library_dir).expect("list recordings");

    fs::remove_file(&path).expect("clean up corrupt recording");
    fs::remove_dir(&library_dir).expect("clean up library");

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "broken");
    assert_eq!(files[0].path, path.to_string_lossy());
    assert_eq!(files[0].step_count, 0);
    assert!(files[0]
        .load_error
        .as_deref()
        .is_some_and(|error| error.contains("invalid recording json")));
}

#[test]
fn concurrent_library_saves_use_distinct_paths_without_overwriting() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-concurrent",
        process::id(),
    ));
    let barrier = Arc::new(Barrier::new(2));

    let save = |barrier: Arc<Barrier>| {
        let library_dir = library_dir.clone();
        thread::spawn(move || {
            barrier.wait();
            save_recording_to_library(&library_dir, &sample_recording())
                .expect("save recording concurrently")
        })
    };
    let first = save(barrier.clone());
    let second = save(barrier);
    let first_path = first.join().expect("join first save");
    let second_path = second.join().expect("join second save");

    assert_ne!(first_path, second_path);
    assert_eq!(
        load_recording(&first_path).expect("load first"),
        sample_recording()
    );
    assert_eq!(
        load_recording(&second_path).expect("load second"),
        sample_recording()
    );
    let library_entries = fs::read_dir(&library_dir)
        .expect("read library")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect library entries");
    assert_eq!(library_entries.len(), 2);
    assert!(library_entries.iter().all(|entry| entry
        .file_name()
        .to_string_lossy()
        .ends_with(".remember.json")));

    fs::remove_file(&first_path).expect("clean up first recording");
    fs::remove_file(&second_path).expect("clean up second recording");
    fs::remove_dir(&library_dir).expect("clean up library");
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
