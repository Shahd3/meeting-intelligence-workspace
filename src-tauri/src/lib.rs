pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("Failed to launch Meeting Intelligence Workspace");
}
