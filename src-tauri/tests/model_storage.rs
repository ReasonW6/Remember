use remember_lib::model::{
    ButtonState, ClientSize, KeyState, MacroStep, MouseButton, PointerPosition, Recording,
    TargetWindowAvailability, TargetWindowId, WindowPointerIntent, WindowTarget,
    RECORDING_VERSION_V1, RECORDING_VERSION_V2,
};
use remember_lib::recorder::MAX_RECORDING_STEPS;
use remember_lib::storage::{
    delete_recording_from_library, list_recordings, load_recording, recording_from_json,
    recording_to_json, rename_recording_in_library, save_recording, save_recording_to_library,
    MAX_RECORDING_FILE_BYTES,
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
        targets: Vec::new(),
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

fn sample_window_relative_recording() -> Recording {
    let editor_id = TargetWindowId(1);
    let dialog_id = TargetWindowId(2);
    Recording::new_window_relative(
        "window aware smoke",
        "2026-08-09T00:00:00Z",
        vec![
            WindowTarget {
                id: editor_id,
                executable_path: r"C:\Windows\System32\notepad.exe".to_string(),
                window_class: "Notepad".to_string(),
                title: "notes.txt - Notepad".to_string(),
                client_size: ClientSize {
                    width: 800,
                    height: 600,
                },
                dpi: 96,
                availability: TargetWindowAvailability::Initial,
            },
            WindowTarget {
                id: dialog_id,
                executable_path: r"C:\Windows\explorer.exe".to_string(),
                window_class: "#32770".to_string(),
                title: "Save As".to_string(),
                client_size: ClientSize {
                    width: 640,
                    height: 480,
                },
                dpi: 144,
                availability: TargetWindowAvailability::Deferred,
            },
        ],
        vec![
            MacroStep::PointerMove {
                elapsed_ms: 0,
                position: PointerPosition::ScreenRelative { x: -20, y: 40 },
            },
            MacroStep::PointerButton {
                elapsed_ms: 20,
                position: PointerPosition::WindowRelative {
                    target_id: editor_id,
                    x: 120,
                    y: 80,
                    intent: WindowPointerIntent::ActivationClick,
                },
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            },
            MacroStep::TargetedKey {
                elapsed_ms: 40,
                target_id: Some(editor_id),
                vk_code: 0x41,
                scan_code: 0x1E,
                extended: false,
                state: KeyState::Pressed,
            },
            MacroStep::PointerWheel {
                elapsed_ms: 120,
                position: PointerPosition::WindowRelative {
                    target_id: dialog_id,
                    x: 320,
                    y: 240,
                    intent: WindowPointerIntent::BackgroundWheel,
                },
                delta: -120,
            },
        ],
    )
}

fn targeted_key(
    elapsed_ms: u64,
    target_id: Option<TargetWindowId>,
    vk_code: u16,
    state: KeyState,
) -> MacroStep {
    MacroStep::TargetedKey {
        elapsed_ms,
        target_id,
        vk_code,
        scan_code: vk_code,
        extended: false,
        state,
    }
}

#[test]
fn serializes_recording_with_stable_version() {
    let json = recording_to_json(&sample_recording()).expect("serialize");

    assert!(json.contains("\"version\": 1"));
    assert!(json.contains("\"kind\": \"key\""));
    assert!(!json.contains("\"targets\""));
}

#[test]
fn loads_and_rewrites_the_exact_legacy_v1_shape_without_upgrading_it() {
    let legacy_json = r#"{
      "version": 1,
      "name": "legacy",
      "created_at": "2026-06-29T00:00:00Z",
      "duration_ms": 25,
      "steps": [
        { "kind": "mouse_move", "elapsed_ms": 25, "x": 10, "y": -5 }
      ]
    }"#;

    let recording = recording_from_json(legacy_json).expect("load legacy v1");
    let rewritten = recording_to_json(&recording).expect("rewrite legacy v1");
    let value: serde_json::Value = serde_json::from_str(&rewritten).expect("parse rewritten v1");

    assert_eq!(recording.version, RECORDING_VERSION_V1);
    assert!(recording.targets.is_empty());
    assert_eq!(
        value.get("version").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert!(value.get("targets").is_none());
    assert_eq!(value["steps"][0]["kind"].as_str(), Some("mouse_move"));
    assert_eq!(value["steps"][0]["x"].as_i64(), Some(10));
    assert_eq!(value["steps"][0]["y"].as_i64(), Some(-5));
}

#[test]
fn serializes_and_deserializes_v2_targets_and_explicit_coordinate_spaces() {
    let original = sample_window_relative_recording();
    let json = recording_to_json(&original).expect("serialize v2");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parse v2 json");
    let loaded = recording_from_json(&json).expect("deserialize v2");

    assert_eq!(loaded, original);
    assert_eq!(value["version"].as_u64(), Some(2));
    assert_eq!(value["targets"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["targets"][0]["id"].as_u64(), Some(1));
    assert_eq!(
        value["targets"][0]["availability"].as_str(),
        Some("initial")
    );
    assert_eq!(
        value["targets"][1]["availability"].as_str(),
        Some("deferred")
    );
    assert_eq!(
        value["steps"][0]["position"]["coordinate_space"].as_str(),
        Some("screen_relative")
    );
    assert_eq!(
        value["steps"][1]["position"]["coordinate_space"].as_str(),
        Some("window_relative")
    );
    assert_eq!(value["steps"][1]["position"]["target_id"].as_u64(), Some(1));
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

    assert!(error.to_string().contains("不支持录制文件版本"));
}

#[test]
fn rejects_v1_content_that_would_change_the_legacy_format() {
    let mut targets_in_v1 = sample_window_relative_recording();
    targets_in_v1.version = RECORDING_VERSION_V1;
    targets_in_v1.steps = vec![MacroStep::Wait { elapsed_ms: 0 }];

    let target_error =
        recording_to_json(&targets_in_v1).expect_err("v1 targets must not serialize");
    assert!(target_error
        .to_string()
        .contains("版本 1 录制文件不能包含目标窗口"));

    let mut v2_step_in_v1 = sample_recording();
    v2_step_in_v1.steps = vec![MacroStep::PointerMove {
        elapsed_ms: 0,
        position: PointerPosition::ScreenRelative { x: 10, y: 20 },
    }];
    let step_error =
        recording_to_json(&v2_step_in_v1).expect_err("v2 steps must not serialize as v1");
    assert!(step_error
        .to_string()
        .contains("版本 1 录制文件不能包含版本 2 步骤"));
}

#[test]
fn rejects_v2_legacy_input_steps_without_an_explicit_coordinate_model() {
    let mut recording = sample_window_relative_recording();
    recording.steps = vec![MacroStep::MouseMove {
        elapsed_ms: 0,
        x: 10,
        y: 20,
    }];

    let error = recording_to_json(&recording).expect_err("legacy pointer step must fail in v2");

    assert!(error
        .to_string()
        .contains("版本 2 录制文件必须使用明确的版本 2 输入步骤"));
}

#[test]
fn rejects_duplicate_and_unknown_v2_target_ids() {
    let mut duplicate = sample_window_relative_recording();
    duplicate.targets[1].id = duplicate.targets[0].id;
    let duplicate_error =
        recording_to_json(&duplicate).expect_err("duplicate target IDs must fail");
    assert!(duplicate_error.to_string().contains("目标窗口 ID 1 重复"));

    let mut unknown = sample_window_relative_recording();
    unknown.steps.push(MacroStep::PointerMove {
        elapsed_ms: 125,
        position: PointerPosition::WindowRelative {
            target_id: TargetWindowId(99),
            x: 0,
            y: 0,
            intent: WindowPointerIntent::Foreground,
        },
    });
    unknown.duration_ms = 125;
    let unknown_error =
        recording_to_json(&unknown).expect_err("unknown target reference must fail");
    assert!(unknown_error.to_string().contains("未知的目标窗口 ID 99"));
}

#[test]
fn rejects_pointer_button_intents_with_the_wrong_button_edge() {
    for (intent, state, expected) in [
        (
            WindowPointerIntent::ActivationClick,
            ButtonState::Released,
            "窗口相对激活点击意图只能用于鼠标按钮按下",
        ),
        (
            WindowPointerIntent::DropRelease,
            ButtonState::Pressed,
            "窗口相对拖放释放意图只能用于鼠标按钮松开",
        ),
    ] {
        let mut recording = sample_window_relative_recording();
        recording.steps = vec![MacroStep::PointerButton {
            elapsed_ms: 0,
            position: PointerPosition::WindowRelative {
                target_id: TargetWindowId(1),
                x: 10,
                y: 20,
                intent,
            },
            button: MouseButton::Left,
            state,
        }];
        recording.duration_ms = 0;

        let error = recording_to_json(&recording).expect_err("mismatched edge must fail");
        assert!(error.to_string().contains(expected));
    }

    let mut valid_edges = sample_window_relative_recording();
    valid_edges.steps = vec![
        MacroStep::PointerButton {
            elapsed_ms: 0,
            position: PointerPosition::WindowRelative {
                target_id: TargetWindowId(1),
                x: 10,
                y: 20,
                intent: WindowPointerIntent::ActivationClick,
            },
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        },
        MacroStep::PointerButton {
            elapsed_ms: 1,
            position: PointerPosition::WindowRelative {
                target_id: TargetWindowId(1),
                x: 10,
                y: 20,
                intent: WindowPointerIntent::DropRelease,
            },
            button: MouseButton::Left,
            state: ButtonState::Released,
        },
    ];
    valid_edges.duration_ms = 1;
    recording_to_json(&valid_edges).expect("matching activation and drop edges remain valid");
}

#[test]
fn targeted_keyboard_sequence_rejects_an_initial_release_and_target_changes() {
    let mut initial_release = sample_window_relative_recording();
    initial_release.steps = vec![targeted_key(
        0,
        Some(TargetWindowId(1)),
        0x41,
        KeyState::Released,
    )];
    initial_release.duration_ms = 0;
    let release_error = recording_to_json(&initial_release).expect_err("initial release must fail");
    assert!(release_error
        .to_string()
        .contains("目标键盘序列必须从按键按下开始"));

    let mut changed_target = sample_window_relative_recording();
    changed_target.steps = vec![
        targeted_key(0, Some(TargetWindowId(1)), 0x41, KeyState::Pressed),
        targeted_key(1, Some(TargetWindowId(2)), 0x42, KeyState::Pressed),
    ];
    changed_target.duration_ms = 1;
    let target_error =
        recording_to_json(&changed_target).expect_err("held sequence target must stay locked");
    assert!(target_error
        .to_string()
        .contains("所有按键松开前必须保持同一目标窗口"));

    let mut changed_from_screen = sample_window_relative_recording();
    changed_from_screen.steps = vec![
        targeted_key(0, None, 0x41, KeyState::Pressed),
        targeted_key(1, Some(TargetWindowId(1)), 0x41, KeyState::Released),
    ];
    changed_from_screen.duration_ms = 1;
    assert!(recording_to_json(&changed_from_screen)
        .expect_err("None is also a locked sequence target")
        .to_string()
        .contains("保持同一目标窗口"));
}

#[test]
fn targeted_keyboard_state_matches_player_repeat_and_unmatched_release_semantics() {
    let mut recording = sample_window_relative_recording();
    recording.steps = vec![
        targeted_key(0, None, 0x41, KeyState::Pressed),
        targeted_key(1, None, 0x41, KeyState::Pressed),
        targeted_key(2, None, 0x42, KeyState::Released),
        targeted_key(3, None, 0x41, KeyState::Released),
        targeted_key(4, Some(TargetWindowId(2)), 0x42, KeyState::Pressed),
        targeted_key(5, Some(TargetWindowId(2)), 0x42, KeyState::Released),
    ];
    recording.duration_ms = 5;

    recording_to_json(&recording)
        .expect("repeat press is idempotent and unmatched release is a no-op while held");
}

#[test]
fn rejects_unusable_v2_target_identity_metadata() {
    let mut missing_path = sample_window_relative_recording();
    missing_path.targets[0].executable_path = "  ".to_string();
    let path_error = recording_to_json(&missing_path).expect_err("empty path must fail");
    assert!(path_error.to_string().contains("可执行文件路径不能为空"));

    let mut missing_class = sample_window_relative_recording();
    missing_class.targets[0].window_class.clear();
    let class_error = recording_to_json(&missing_class).expect_err("empty class must fail");
    assert!(class_error.to_string().contains("窗口类名不能为空"));

    let mut zero_dpi = sample_window_relative_recording();
    zero_dpi.targets[0].dpi = 0;
    let dpi_error = recording_to_json(&zero_dpi).expect_err("zero DPI must fail");
    assert!(dpi_error.to_string().contains("DPI 必须大于零"));

    let mut zero_width = sample_window_relative_recording();
    zero_width.targets[0].client_size.width = 0;
    let size_error = recording_to_json(&zero_width).expect_err("zero client size must fail");
    assert!(size_error
        .to_string()
        .contains("客户区宽度和高度必须大于零"));
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

    assert!(error.to_string().contains("步骤时间戳不得倒退"));
}

#[test]
fn rejects_recordings_over_the_step_limit_on_import_and_export() {
    let recording = Recording {
        version: 1,
        name: "too many steps".to_string(),
        created_at: "2026-06-29T00:00:00Z".to_string(),
        duration_ms: 0,
        targets: Vec::new(),
        steps: vec![MacroStep::Wait { elapsed_ms: 0 }; MAX_RECORDING_STEPS + 1],
    };

    let export_error =
        recording_to_json(&recording).expect_err("oversized recording must not serialize");
    assert!(export_error
        .to_string()
        .contains(&MAX_RECORDING_STEPS.to_string()));

    let json = serde_json::to_string(&recording).expect("construct oversized import fixture");
    assert!(
        json.len() as u64 <= MAX_RECORDING_FILE_BYTES,
        "step-limit fixture must remain below the independent file-size limit"
    );
    let import_error =
        recording_from_json(&json).expect_err("oversized recording must not deserialize");
    assert!(import_error
        .to_string()
        .contains(&MAX_RECORDING_STEPS.to_string()));
}

#[test]
fn rejects_recording_file_over_the_byte_limit_before_parsing() {
    let path = env::temp_dir().join(format!(
        "remember-model-storage-{}-oversized.json",
        process::id()
    ));
    let file = fs::File::create(&path).expect("create oversized fixture");
    file.set_len(MAX_RECORDING_FILE_BYTES + 1)
        .expect("set oversized fixture length");
    drop(file);

    let error = load_recording(&path).expect_err("oversized file must fail");

    fs::remove_file(&path).expect("clean up oversized fixture");

    assert!(error
        .to_string()
        .contains(&MAX_RECORDING_FILE_BYTES.to_string()));
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
fn saves_loads_and_lists_window_relative_v2_recordings() {
    let recording = sample_window_relative_recording();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-v2-library",
        process::id(),
    ));
    let path = save_recording_to_library(&library_dir, &recording).expect("save v2 to library");

    let loaded = load_recording(&path).expect("load v2 recording");
    let files = list_recordings(&library_dir).expect("list v2 recording");

    assert_eq!(loaded, recording);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].version, Some(RECORDING_VERSION_V2));
    assert_eq!(files[0].step_count, recording.steps.len());

    fs::remove_file(&path).expect("clean up v2 recording");
    fs::remove_dir(&library_dir).expect("clean up v2 library");
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
    assert_eq!(files[0].version, Some(RECORDING_VERSION_V1));
    assert_eq!(files[0].step_count, recording.steps.len());
    assert_eq!(files[0].duration_ms, recording.duration_ms);
    assert_eq!(files[0].load_error, None);
}

#[test]
fn recording_list_cache_refreshes_changed_corrupt_and_removed_files() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-cache-refresh",
        process::id(),
    ));
    let original = sample_recording();
    let path = save_recording_to_library(&library_dir, &original).expect("save to library");

    let initial = list_recordings(&library_dir).expect("prime recording list cache");
    assert_eq!(initial[0].name, original.name);

    let original_size = fs::metadata(&path).expect("read original metadata").len();
    let mut changed = original;
    changed.name = "externally updated recording with a longer name".to_string();
    changed.duration_ms = 450;
    let changed_json = recording_to_json(&changed).expect("serialize changed recording");
    assert_ne!(
        changed_json.len() as u64,
        original_size,
        "fixture must change the cache file-size key"
    );
    fs::write(&path, changed_json).expect("replace recording outside storage API");

    let refreshed = list_recordings(&library_dir).expect("refresh changed recording");
    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].name, changed.name);
    assert_eq!(refreshed[0].duration_ms, changed.duration_ms);
    assert_eq!(refreshed[0].load_error, None);

    fs::write(&path, "not valid json after a cached valid recording")
        .expect("corrupt cached recording externally");
    let corrupt = list_recordings(&library_dir).expect("refresh corrupt recording");
    assert_eq!(corrupt.len(), 1);
    assert_eq!(corrupt[0].name, "notepad-smoke");
    assert!(corrupt[0]
        .load_error
        .as_deref()
        .is_some_and(|error| error.contains("invalid recording json")));

    fs::remove_file(&path).expect("remove cached recording externally");
    let removed = list_recordings(&library_dir).expect("prune removed recording");
    fs::remove_dir(&library_dir).expect("clean up library");

    assert!(removed.is_empty());
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
    assert_eq!(renamed.version, RECORDING_VERSION_V1);
    assert!(renamed.targets.is_empty());
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "renamed recording");
    assert_eq!(files[0].path, renamed_path.to_string_lossy());

    let renamed_json = fs::read_to_string(&renamed_path).expect("read renamed recording JSON");
    assert!(!renamed_json.contains("\"targets\""));

    fs::remove_file(&renamed_path).expect("clean up renamed recording");
    fs::remove_dir(&library_dir).expect("clean up library");
}

#[test]
fn renames_v2_without_losing_target_metadata_or_coordinate_spaces() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let library_dir = env::temp_dir().join(format!(
        "remember-model-storage-{}-{unique}-v2-rename",
        process::id(),
    ));
    let original = sample_window_relative_recording();
    let path = save_recording_to_library(&library_dir, &original).expect("save v2 recording");

    let renamed_path = rename_recording_in_library(&library_dir, &path, "renamed window recording")
        .expect("rename v2 recording");
    let renamed = load_recording(&renamed_path).expect("load renamed v2 recording");

    assert_eq!(renamed.version, RECORDING_VERSION_V2);
    assert_eq!(renamed.name, "renamed window recording");
    assert_eq!(renamed.targets, original.targets);
    assert_eq!(renamed.steps, original.steps);

    fs::remove_file(&renamed_path).expect("clean up renamed v2 recording");
    fs::remove_dir(&library_dir).expect("clean up v2 rename library");
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
    assert_eq!(files[0].version, None);
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
