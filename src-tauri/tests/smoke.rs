#[test]
fn exposes_product_name() {
    assert_eq!(remember_lib::product_name(), "Remember");
}

#[test]
fn release_entry_uses_windows_gui_subsystem() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let main_rs = std::fs::read_to_string(format!("{manifest_dir}/src/main.rs"))
        .expect("read binary entrypoint");

    assert!(
        main_rs.contains("windows_subsystem = \"windows\""),
        "release Windows binary should use the GUI subsystem so it does not open a console window"
    );
}

#[test]
fn tauri_config_uses_uploaded_icon_assets() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let config = std::fs::read_to_string(format!("{manifest_dir}/tauri.conf.json"))
        .expect("read tauri config");
    let icon = std::fs::metadata(format!("{manifest_dir}/icons/icon.ico")).expect("icon.ico");
    let svg = std::fs::metadata(format!("{manifest_dir}/icons/remember-icon.svg"))
        .expect("remember-icon.svg");

    assert!(config.contains("\"icons/icon.ico\""));
    assert!(
        icon.len() > 1_000,
        "icon.ico should not be the placeholder ico"
    );
    assert!(svg.len() > 1_000, "remember-icon.svg should be present");
}

#[test]
fn main_window_uses_custom_titlebar() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let config = std::fs::read_to_string(format!("{manifest_dir}/tauri.conf.json"))
        .expect("read tauri config");

    assert!(
        config.contains("\"decorations\": false"),
        "main window should disable native decorations for the custom titlebar"
    );
}

#[test]
fn production_webview_uses_a_restrictive_csp() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let config = std::fs::read_to_string(format!("{manifest_dir}/tauri.conf.json"))
        .expect("read tauri config");
    let config: serde_json::Value = serde_json::from_str(&config).expect("parse tauri config");
    let csp = config["app"]["security"]["csp"]
        .as_str()
        .expect("CSP should be configured as a string");
    let dev_csp = config["app"]["security"]["devCsp"]
        .as_str()
        .expect("development CSP should be configured as a string");

    assert!(csp.contains("default-src 'self'"));
    assert!(csp.contains("connect-src ipc: http://ipc.localhost"));
    assert!(csp.contains("object-src 'none'"));
    assert!(csp.contains("frame-ancestors 'none'"));
    assert!(csp.contains("form-action 'none'"));
    assert!(!csp.contains("localhost:1420"));
    assert!(!csp.contains("asset:"));
    assert!(!csp.contains("'unsafe-inline'"));
    assert!(!csp.contains("'unsafe-eval'"));

    assert!(dev_csp.contains("http://localhost:1420"));
    assert!(dev_csp.contains("ws://localhost:1420"));
    assert!(dev_csp.contains("style-src 'self' 'unsafe-inline'"));
}

#[test]
fn main_window_capability_uses_only_required_events_windows_and_dialogs() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let capability = std::fs::read_to_string(format!("{manifest_dir}/capabilities/default.json"))
        .expect("read default capability");
    let capability: serde_json::Value =
        serde_json::from_str(&capability).expect("parse default capability");
    let permissions = capability["permissions"]
        .as_array()
        .expect("permissions should be an array");
    let has = |permission: &str| permissions.iter().any(|value| value == permission);

    assert!(!has("core:default"));
    assert!(!has("dialog:default"));
    for required in [
        "core:event:allow-listen",
        "core:event:allow-unlisten",
        "core:window:allow-start-dragging",
        "core:window:allow-minimize",
        "core:window:allow-close",
        "dialog:allow-open",
        "dialog:allow-save",
        "dialog:allow-ask",
    ] {
        assert!(has(required), "missing {required}");
    }
    assert_eq!(permissions.len(), 8);
}
