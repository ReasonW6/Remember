fn main() {
    println!("cargo:rerun-if-changed=permissions");
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().permissions_path_pattern("./permissions/*.json"),
    ))
    .expect("failed to build Remember");
}
