pub mod model;
pub mod player;
pub mod storage;

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
