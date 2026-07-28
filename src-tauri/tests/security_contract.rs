use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const ALL_APP_COMMANDS: [&str; 18] = [
    "get_state",
    "start_recording",
    "stop_recording",
    "list_recordings",
    "delete_recording",
    "rename_recording",
    "open_recording",
    "save_current_recording",
    "get_hotkeys",
    "get_advanced_settings",
    "get_settings_bundle",
    "set_settings_bundle",
    "show_advanced_settings",
    "get_privilege_state",
    "restart_as_administrator",
    "start_playback",
    "set_playback_settings",
    "stop_playback",
];

const MAIN_WINDOW_COMMANDS: [&str; 16] = [
    "get_state",
    "start_recording",
    "stop_recording",
    "list_recordings",
    "delete_recording",
    "rename_recording",
    "open_recording",
    "save_current_recording",
    "get_hotkeys",
    "get_advanced_settings",
    "show_advanced_settings",
    "get_privilege_state",
    "restart_as_administrator",
    "start_playback",
    "set_playback_settings",
    "stop_playback",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    serde_json::from_str(
        &fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("could not parse {}: {error}", path.display()))
}

fn string_set(values: &Value) -> BTreeSet<String> {
    values
        .as_array()
        .expect("expected an array")
        .iter()
        .map(|value| value.as_str().expect("expected a string").to_string())
        .collect()
}

fn permission_commands<'a>(permissions: &'a Value, identifier: &str) -> &'a Value {
    permissions["permission"]
        .as_array()
        .expect("permission list")
        .iter()
        .find(|permission| permission["identifier"] == identifier)
        .unwrap_or_else(|| panic!("missing permission {identifier}"))
        .get("commands")
        .and_then(|commands| commands.get("allow"))
        .expect("permission allow list")
}

fn capability_permissions(file_name: &str, expected_window: &str) -> BTreeSet<String> {
    let capability = read_json(manifest_dir().join("capabilities").join(file_name));
    assert_eq!(
        string_set(&capability["windows"]),
        BTreeSet::from([expected_window.to_string()])
    );
    string_set(&capability["permissions"])
}

#[test]
fn each_window_has_only_its_required_application_commands() {
    let permissions = read_json(manifest_dir().join("permissions/remember.json"));
    let main_commands = string_set(permission_commands(&permissions, "main-window-commands"));
    let advanced_settings_commands = string_set(permission_commands(
        &permissions,
        "advanced-settings-window-commands",
    ));
    let activity_indicator_commands = string_set(permission_commands(
        &permissions,
        "activity-indicator-window-commands",
    ));

    assert_eq!(
        main_commands,
        BTreeSet::from(MAIN_WINDOW_COMMANDS.map(str::to_string))
    );
    assert_eq!(
        advanced_settings_commands,
        BTreeSet::from([
            "get_settings_bundle".to_string(),
            "set_settings_bundle".to_string(),
        ])
    );
    assert_eq!(
        activity_indicator_commands,
        BTreeSet::from(["get_state".to_string()])
    );
    let declared_commands = permissions["permission"]
        .as_array()
        .expect("permission list")
        .iter()
        .flat_map(|permission| {
            permission["commands"]["allow"]
                .as_array()
                .expect("permission allow list")
        })
        .map(|command| command.as_str().expect("command string").to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        declared_commands,
        BTreeSet::from(ALL_APP_COMMANDS.map(str::to_string))
    );

    assert_eq!(
        capability_permissions("default.json", "main"),
        BTreeSet::from([
            "core:event:allow-listen".to_string(),
            "core:event:allow-unlisten".to_string(),
            "core:window:allow-start-dragging".to_string(),
            "core:window:allow-minimize".to_string(),
            "core:window:allow-unminimize".to_string(),
            "core:window:allow-set-focus".to_string(),
            "core:window:allow-close".to_string(),
            "dialog:allow-open".to_string(),
            "dialog:allow-save".to_string(),
            "dialog:allow-ask".to_string(),
            "main-window-commands".to_string(),
        ])
    );
    assert_eq!(
        capability_permissions("advanced-settings.json", "advanced-settings"),
        BTreeSet::from([
            "core:window:allow-start-dragging".to_string(),
            "core:window:allow-close".to_string(),
            "advanced-settings-window-commands".to_string(),
        ])
    );
    assert_eq!(
        capability_permissions("activity-indicator.json", "activity-indicator"),
        BTreeSet::from([
            "core:event:allow-listen".to_string(),
            "core:event:allow-unlisten".to_string(),
            "activity-indicator-window-commands".to_string(),
        ])
    );
}

#[test]
fn generated_acl_schema_contains_the_application_manifest() {
    let schema = read_json(manifest_dir().join("gen/schemas/acl-manifests.json"));
    let app_manifest = schema
        .get("__app-acl__")
        .expect("Tauri build must generate an application ACL manifest");
    let manifest_permissions = app_manifest["permissions"]
        .as_object()
        .expect("application permissions");

    for identifier in [
        "main-window-commands",
        "advanced-settings-window-commands",
        "activity-indicator-window-commands",
    ] {
        assert!(
            manifest_permissions.contains_key(identifier),
            "generated ACL schema is missing {identifier}"
        );
    }
}
