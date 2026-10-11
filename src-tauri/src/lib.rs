mod storage;
use tauri::Manager;

#[tauri::command]
async fn data_request(
    app: tauri::AppHandle,
    request: storage::Request,
) -> Result<storage::Snapshot, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Unable to locate your application data directory.")?;
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&directory)
            .map_err(|_| "Unable to access your application data directory.")?;
        let mut db = storage::open(&directory.join("workspace.sqlite3"))?;
        storage::apply(&mut db, request)
    })
    .await
    .map_err(|_| "The local database worker could not finish.")?
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![data_request])
        .run(tauri::generate_context!())
        .expect("Failed to launch Meeting Intelligence Workspace");
}
