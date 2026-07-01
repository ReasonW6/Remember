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
