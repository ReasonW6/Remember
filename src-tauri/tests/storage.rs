use remember_lib::storage::{
    ensure_default_flow_in_dir, list_flow_summaries_in_dir, load_flow_from_path, sample_flow,
    save_flow_as_to_dir, save_flow_to_dir, FlowStep, StorageError,
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
    assert!(saved.path.ends_with("flows/daily-report.remember.json"));

    let loaded = load_flow_from_path(&saved.path).expect("saved flow should load");
    assert_eq!(loaded, flow);

    let raw = fs::read_to_string(saved.path).expect("saved file should be readable");
    assert!(raw.contains("\"version\": 1"));
    assert!(raw.contains("\"steps\""));
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
fn save_as_creates_a_distinct_named_copy_without_overwriting_source() {
    let root = temp_root("save-as");
    let original = save_flow_to_dir(&root, &sample_flow()).expect("source flow should save");

    let saved_copy =
        save_flow_as_to_dir(&root, &original.flow, "Daily Report Copy").expect("copy should save");

    assert_eq!(saved_copy.file_name, "daily-report-copy.remember.json");
    assert_eq!(saved_copy.flow.name, "daily-report-copy");
    assert_eq!(saved_copy.flow.display_name, "Daily Report Copy");
    assert_ne!(saved_copy.file_name, original.file_name);

    let loaded_original = load_flow_from_path(&original.path).expect("source should still load");
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
