use remember_lib::storage::{
    ensure_default_flow_in_dir, initial_flow_in_dir, list_flow_summaries_in_dir,
    load_flow_from_path, sample_flow, save_flow_as_to_dir, save_flow_file_to_dir, save_flow_to_dir,
    FlowStep, StorageError,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_root(test_name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("remember-{test_name}-{stamp}"));
    fs::create_dir_all(&root).expect("temp root should be created");
    root
}

#[test]
fn save_flow_writes_remember_json_and_round_trips() {
    let root = temp_root("round-trip");
    let flow = sample_flow();

    let saved = save_flow_to_dir(&root, &flow).expect("flow should save");

    assert_eq!(saved.file_name, "daily-report.remember.json");
    let saved_path = root.join("flows").join(&saved.file_name);
    assert!(saved_path.ends_with("flows/daily-report.remember.json"));

    let loaded = load_flow_from_path(&saved_path).expect("saved flow should load");
    assert_eq!(loaded, flow);

    let raw = fs::read_to_string(saved_path).expect("saved file should be readable");
    assert!(raw.contains("\"version\": 1"));
    assert!(raw.contains("\"steps\""));
}

#[test]
fn save_flow_file_updates_the_selected_file_even_when_internal_name_differs() {
    let root = temp_root("save-selected-file");
    let mut flow = sample_flow();
    let original =
        save_flow_as_to_dir(&root, &flow, "Original Flow").expect("source flow should save");

    flow.name = "renamed-in-memory".to_string();
    flow.display_name = "Renamed In Memory".to_string();
    let saved = save_flow_file_to_dir(&root, &original.file_name, &flow)
        .expect("selected file should save");

    assert_eq!(saved.file_name, original.file_name);
    let saved_path = root.join("flows").join(&saved.file_name);
    assert!(saved_path.ends_with("flows/original-flow.remember.json"));
    assert!(!root
        .join("flows")
        .join("renamed-in-memory.remember.json")
        .exists());

    let loaded = load_flow_from_path(&saved_path).expect("saved file should load");
    assert_eq!(loaded.display_name, "Renamed In Memory");
}

#[test]
fn save_flow_file_rejects_non_slug_file_names() {
    let root = temp_root("invalid-file-name");
    let flow = sample_flow();

    for file_name in [
        "../bad.remember.json",
        "bad\\name.remember.json",
        "bad:name.remember.json",
        "bad name.remember.json",
        "CON.remember.json",
        "bad.json",
    ] {
        let error =
            save_flow_file_to_dir(&root, file_name, &flow).expect_err("file name should reject");

        assert!(matches!(
            error,
            StorageError::InvalidFileName(message) if message == file_name
        ));
    }
}

#[test]
fn first_run_default_flow_is_created_once() {
    let root = temp_root("first-run");

    let first = ensure_default_flow_in_dir(&root).expect("default flow should be created");
    assert_eq!(first.file_name, "daily-report.remember.json");

    let mut edited = first.flow;
    edited.display_name = "Changed Local Flow".to_string();
    save_flow_to_dir(&root, &edited).expect("edited default should save");

    let second = ensure_default_flow_in_dir(&root).expect("existing flow should load");
    assert_eq!(second.flow.display_name, "Changed Local Flow");
}

#[test]
fn initial_flow_starts_empty_without_creating_default_file() {
    let root = temp_root("initial-empty");

    let initial = initial_flow_in_dir(&root).expect("initial flow should load");

    assert_eq!(initial.file_name, "untitled-flow.remember.json");
    assert_eq!(initial.saved_at, 0);
    assert_eq!(initial.flow.name, "untitled-flow");
    assert_eq!(initial.flow.steps.len(), 0);
    assert_eq!(initial.flow.target_window.process, "N/A");
    assert!(!root.join("flows").exists());
}

#[test]
fn initial_flow_skips_invalid_files_and_loads_first_valid_flow() {
    let root = temp_root("first-valid");
    let flows_dir = root.join("flows");
    fs::create_dir_all(&flows_dir).expect("flows dir should be created");
    fs::write(flows_dir.join("aaa-broken.remember.json"), "{ not-json")
        .expect("broken file should be written");

    let mut flow = sample_flow();
    flow.display_name = "Valid Local Flow".to_string();
    save_flow_to_dir(&root, &flow).expect("valid flow should save");

    let loaded = ensure_default_flow_in_dir(&root).expect("valid flow should load");

    assert_eq!(loaded.file_name, "daily-report.remember.json");
    assert_eq!(loaded.flow.display_name, "Valid Local Flow");
}

#[test]
fn initial_flow_creates_default_when_only_invalid_files_exist() {
    let root = temp_root("invalid-only");
    let flows_dir = root.join("flows");
    fs::create_dir_all(&flows_dir).expect("flows dir should be created");
    fs::write(flows_dir.join("broken.remember.json"), "{ not-json")
        .expect("broken file should be written");

    let loaded = ensure_default_flow_in_dir(&root).expect("default flow should be created");
    let summaries = list_flow_summaries_in_dir(&root).expect("summaries should load");

    assert_eq!(loaded.file_name, "daily-report.remember.json");
    assert!(summaries
        .iter()
        .any(|summary| summary.file_name == "broken.remember.json" && !summary.is_valid));
    assert!(summaries
        .iter()
        .any(|summary| summary.file_name == "daily-report.remember.json" && summary.is_valid));
}

#[test]
fn initial_flow_does_not_create_default_when_only_invalid_files_exist() {
    let root = temp_root("initial-invalid-only");
    let flows_dir = root.join("flows");
    fs::create_dir_all(&flows_dir).expect("flows dir should be created");
    fs::write(flows_dir.join("broken.remember.json"), "{ not-json")
        .expect("broken file should be written");

    let initial = initial_flow_in_dir(&root).expect("initial flow should load");
    let summaries = list_flow_summaries_in_dir(&root).expect("summaries should load");

    assert_eq!(initial.file_name, "untitled-flow.remember.json");
    assert_eq!(initial.flow.steps.len(), 0);
    assert!(summaries
        .iter()
        .any(|summary| summary.file_name == "broken.remember.json" && !summary.is_valid));
    assert!(!summaries
        .iter()
        .any(|summary| summary.file_name == "daily-report.remember.json"));
}

#[test]
fn initial_flow_removes_legacy_default_seed_file() {
    let root = temp_root("legacy-default");
    let mut legacy = sample_flow();
    legacy.display_name = "Phase1 PersistedF'luo'w".to_string();
    legacy.steps = Vec::new();
    save_flow_file_to_dir(&root, "daily-report.remember.json", &legacy)
        .expect("legacy default should save");

    let legacy_path = root.join("flows").join("daily-report.remember.json");
    assert!(legacy_path.exists());

    let initial = initial_flow_in_dir(&root).expect("initial flow should load");
    let summaries = list_flow_summaries_in_dir(&root).expect("summaries should load");

    assert_eq!(initial.file_name, "untitled-flow.remember.json");
    assert_eq!(initial.flow.steps.len(), 0);
    assert!(!legacy_path.exists());
    assert!(!summaries
        .iter()
        .any(|summary| summary.file_name == "daily-report.remember.json"));
}

#[test]
fn malformed_or_wrong_version_flow_files_are_rejected() {
    let root = temp_root("invalid");
    let flows_dir = root.join("flows");
    fs::create_dir_all(&flows_dir).expect("flows dir should be created");

    let malformed = flows_dir.join("broken.remember.json");
    fs::write(&malformed, "{ not-json").expect("malformed file should be written");
    assert!(load_flow_from_path(&malformed).is_err());

    let unsupported = flows_dir.join("unsupported.remember.json");
    fs::write(
        &unsupported,
        r#"{"version":2,"name":"bad","displayName":"Bad","targetWindow":{"title":"","process":"","size":"","matched":false},"steps":[]}"#,
    )
    .expect("unsupported file should be written");
    assert!(load_flow_from_path(&unsupported).is_err());
}

#[test]
fn legacy_scroll_steps_without_coordinates_still_load() {
    let root = temp_root("legacy-scroll");
    let flows_dir = root.join("flows");
    fs::create_dir_all(&flows_dir).expect("flows dir should be created");
    let legacy = flows_dir.join("legacy-scroll.remember.json");
    fs::write(
        &legacy,
        r#"{"version":1,"name":"legacy-scroll","displayName":"Legacy Scroll","targetWindow":{"title":"Notepad","process":"notepad.exe","size":"800 x 600","matched":true},"steps":[{"type":"scroll","id":1,"action":"滚动","deltaX":0,"deltaY":-120,"delayMs":100,"note":"legacy scroll"}]}"#,
    )
    .expect("legacy file should be written");

    let loaded = load_flow_from_path(&legacy).expect("legacy scroll should load");

    match &loaded.steps[0] {
        FlowStep::Scroll { x, y, .. } => assert_eq!((*x, *y), (None, None)),
        step => panic!("expected scroll step, got {step:?}"),
    }
}

#[test]
fn validation_rejects_duplicate_step_ids() {
    let root = temp_root("duplicate-step-id");
    let mut flow = sample_flow();
    if let FlowStep::Type { id, .. } = &mut flow.steps[1] {
        *id = 1;
    }

    let error = save_flow_to_dir(&root, &flow).expect_err("duplicate ids should be rejected");

    assert!(matches!(
        error,
        StorageError::InvalidFlow(message)
            if message.contains("duplicate step id 1")
    ));
}

#[test]
fn validation_rejects_wait_steps_with_mismatched_duration_and_delay() {
    let root = temp_root("mismatched-wait");
    let mut flow = sample_flow();
    if let FlowStep::Wait { delay_ms, .. } = &mut flow.steps[2] {
        *delay_ms = 999;
    }

    let error = save_flow_to_dir(&root, &flow).expect_err("mismatched wait should be rejected");

    assert!(matches!(
        error,
        StorageError::InvalidFlow(message)
            if message.contains("wait step 3")
                && message.contains("durationMs")
                && message.contains("delayMs")
    ));
}

#[test]
fn validation_rejects_empty_key_and_hotkey_steps() {
    let root = temp_root("empty-key-hotkey");

    let mut empty_key = sample_flow();
    empty_key.steps.push(FlowStep::Key {
        id: 9,
        action: "按键".to_string(),
        key: " ".to_string(),
        delay_ms: 100,
        note: "empty key".to_string(),
    });
    let key_error = save_flow_to_dir(&root, &empty_key).expect_err("empty key should reject");
    assert!(matches!(
        key_error,
        StorageError::InvalidFlow(message)
            if message.contains("key step 9") && message.contains("key is required")
    ));

    let mut empty_hotkey = sample_flow();
    empty_hotkey.steps.push(FlowStep::Hotkey {
        id: 9,
        action: "快捷键".to_string(),
        keys: Vec::new(),
        delay_ms: 100,
        note: "empty hotkey".to_string(),
    });
    let hotkey_error =
        save_flow_to_dir(&root, &empty_hotkey).expect_err("empty hotkey should reject");
    assert!(matches!(
        hotkey_error,
        StorageError::InvalidFlow(message)
            if message.contains("hotkey step 9") && message.contains("at least one key")
    ));
}

#[test]
fn validation_rejects_unsupported_mouse_actions() {
    let root = temp_root("unsupported-mouse-action");

    let mut unknown_click = sample_flow();
    if let FlowStep::Click { action, .. } = &mut unknown_click.steps[0] {
        *action = "primary click".to_string();
    }
    let click_error =
        save_flow_to_dir(&root, &unknown_click).expect_err("unknown click action should reject");
    assert!(matches!(
        click_error,
        StorageError::InvalidFlow(message)
            if message.contains("click step 1") && message.contains("unsupported action")
    ));

    let mut unknown_drag = sample_flow();
    unknown_drag.steps.push(FlowStep::Drag {
        id: 9,
        action: "drag".to_string(),
        target: "(0, 0) -> (10, 10) [屏幕绝对]".to_string(),
        start_x: 0,
        start_y: 0,
        end_x: 10,
        end_y: 10,
        duration_ms: 100,
        delay_ms: 100,
        path: Vec::new(),
        note: "unknown drag".to_string(),
    });
    let drag_error =
        save_flow_to_dir(&root, &unknown_drag).expect_err("unknown drag action should reject");
    assert!(matches!(
        drag_error,
        StorageError::InvalidFlow(message)
            if message.contains("drag step 9") && message.contains("unsupported action")
    ));
}

#[test]
fn validation_rejects_single_system_modifier_key_steps() {
    for key in ["Win", "Windows", "Alt", "Ctrl", "Control", "Shift"] {
        let root = temp_root("unsafe-key");
        let mut flow = sample_flow();
        flow.steps.push(FlowStep::Key {
            id: 9,
            action: "按键".to_string(),
            key: key.to_string(),
            delay_ms: 100,
            note: "unsafe key".to_string(),
        });

        let error = save_flow_to_dir(&root, &flow).expect_err("unsafe key should reject");

        assert!(matches!(
            error,
            StorageError::InvalidFlow(message)
                if message.contains("key step 9") && message.contains("not allowed")
        ));
    }
}

#[test]
fn validation_rejects_sensitive_type_text_before_save() {
    for text in [
        "password: hunter2",
        "验证码 123456",
        "api key sk-local-secret",
        "4111 1111 1111 1111",
    ] {
        let root = temp_root("sensitive-type-text");
        let mut flow = sample_flow();
        flow.steps.push(FlowStep::Type {
            id: 9,
            action: "文本输入".to_string(),
            text: text.to_string(),
            delay_ms: 100,
            note: "sensitive text".to_string(),
        });

        let error = save_flow_to_dir(&root, &flow).expect_err("sensitive text should reject");

        assert!(matches!(
            error,
            StorageError::InvalidFlow(message)
                if message.contains("type step 9") && message.contains("sensitive text")
        ));
    }
}

#[test]
fn validation_allows_descriptive_flow_metadata_before_save() {
    let root = temp_root("descriptive-metadata");

    let mut display_name_flow = sample_flow();
    display_name_flow.display_name = "Password Reset 操作说明".to_string();
    save_flow_to_dir(&root, &display_name_flow).expect("descriptive display name should save");

    let mut title_flow = sample_flow();
    title_flow.target_window.title = "验证码 - Browser".to_string();
    save_flow_to_dir(&root, &title_flow).expect("descriptive target title should save");
}

#[test]
fn validation_rejects_sensitive_step_metadata_before_save() {
    let root = temp_root("sensitive-step-metadata");

    let mut note_flow = sample_flow();
    if let FlowStep::Click { note, .. } = &mut note_flow.steps[0] {
        *note = "click password field".to_string();
    }
    let note_error = save_flow_to_dir(&root, &note_flow).expect_err("sensitive note rejects");
    assert!(matches!(
        note_error,
        StorageError::InvalidFlow(message)
            if message.contains("step 1 note") && message.contains("sensitive")
    ));

    let mut target_flow = sample_flow();
    if let FlowStep::Click { target, .. } = &mut target_flow.steps[0] {
        *target = "password field".to_string();
    }
    let target_error = save_flow_to_dir(&root, &target_flow).expect_err("sensitive target rejects");
    assert!(matches!(
        target_error,
        StorageError::InvalidFlow(message)
            if message.contains("step 1 target") && message.contains("sensitive")
    ));
}

#[test]
fn validation_rejects_excessive_step_timing() {
    let root = temp_root("excessive-step-timing");

    let mut long_wait = sample_flow();
    if let FlowStep::Wait {
        duration_ms,
        delay_ms,
        ..
    } = &mut long_wait.steps[2]
    {
        *duration_ms = 300_001;
        *delay_ms = 300_001;
    }
    let wait_error = save_flow_to_dir(&root, &long_wait).expect_err("long wait rejects");
    assert!(matches!(
        wait_error,
        StorageError::InvalidFlow(message)
            if message.contains("wait step 3") && message.contains("exceeds")
    ));

    let mut long_drag = sample_flow();
    long_drag.steps.push(FlowStep::Drag {
        id: 9,
        action: "左键拖拽".to_string(),
        target: "(0, 0) -> (10, 10) [屏幕绝对]".to_string(),
        start_x: 0,
        start_y: 0,
        end_x: 10,
        end_y: 10,
        duration_ms: 300_001,
        delay_ms: 100,
        path: Vec::new(),
        note: "long drag".to_string(),
    });
    let drag_error = save_flow_to_dir(&root, &long_drag).expect_err("long drag rejects");
    assert!(matches!(
        drag_error,
        StorageError::InvalidFlow(message)
            if message.contains("drag step 9") && message.contains("durationMs")
    ));
}

#[test]
fn validation_rejects_high_risk_global_hotkeys() {
    for keys in [
        vec!["Win", "R"],
        vec!["Alt", "F4"],
        vec!["Alt", "Tab"],
        vec!["Alt", "Esc"],
        vec!["Ctrl", "Esc"],
        vec!["Ctrl", "Shift", "Esc"],
        vec!["Ctrl", "Alt", "Delete"],
        vec!["Ctrl", "Alt", "S"],
    ] {
        let root = temp_root("unsafe-hotkey");
        let mut flow = sample_flow();
        flow.steps.push(FlowStep::Hotkey {
            id: 9,
            action: "快捷键".to_string(),
            keys: keys.iter().map(|key| key.to_string()).collect(),
            delay_ms: 100,
            note: "unsafe hotkey".to_string(),
        });

        let error = save_flow_to_dir(&root, &flow).expect_err("unsafe hotkey should reject");

        assert!(matches!(
            error,
            StorageError::InvalidFlow(message)
                if message.contains("hotkey step 9") && message.contains("not allowed")
        ));
    }
}

#[test]
fn list_flow_summaries_includes_invalid_remember_files() {
    let root = temp_root("list-invalid");
    let flows_dir = root.join("flows");
    fs::create_dir_all(&flows_dir).expect("flows dir should be created");
    fs::write(flows_dir.join("broken.remember.json"), "{ not-json")
        .expect("broken file should be written");

    let summaries = list_flow_summaries_in_dir(&root).expect("summaries should load");

    let broken = summaries
        .iter()
        .find(|summary| summary.file_name == "broken.remember.json")
        .expect("broken flow should remain visible");
    assert!(!broken.is_valid);
    assert!(broken
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("flow json"));
}

#[test]
fn save_as_creates_a_distinct_named_copy_without_overwriting_source() {
    let root = temp_root("save-as");
    let original = save_flow_to_dir(&root, &sample_flow()).expect("source flow should save");

    let saved_copy =
        save_flow_as_to_dir(&root, &original.flow, "Daily Report Copy").expect("copy should save");

    assert_eq!(saved_copy.file_name, "daily-report-copy.remember.json");
    assert_eq!(saved_copy.flow.name, "daily-report-copy");
    assert_eq!(saved_copy.flow.display_name, "Daily Report Copy");
    assert_ne!(saved_copy.file_name, original.file_name);

    let original_path = root.join("flows").join(&original.file_name);
    let loaded_original = load_flow_from_path(&original_path).expect("source should still load");
    assert_eq!(loaded_original.display_name, "Daily Report 自动化");

    let summaries = list_flow_summaries_in_dir(&root).expect("summaries should list flows");
    assert_eq!(summaries.len(), 2);
    assert!(summaries
        .iter()
        .any(|summary| summary.file_name == "daily-report.remember.json"));
    assert!(summaries
        .iter()
        .any(|summary| summary.file_name == "daily-report-copy.remember.json"));
}

#[test]
fn save_as_adds_a_number_when_the_target_name_already_exists() {
    let root = temp_root("save-as-duplicate");
    let flow = sample_flow();

    save_flow_as_to_dir(&root, &flow, "Daily Report Copy").expect("first copy should save");
    let second =
        save_flow_as_to_dir(&root, &flow, "Daily Report Copy").expect("second copy should save");

    assert_eq!(second.file_name, "daily-report-copy-2.remember.json");
    assert_eq!(second.flow.name, "daily-report-copy-2");
    assert_eq!(second.flow.display_name, "Daily Report Copy 2");
}
