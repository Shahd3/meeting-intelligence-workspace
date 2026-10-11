mod recording;
mod storage;
use std::sync::Arc;
use tauri::Manager;
struct RecorderState(Result<Arc<recording::Recorder>, String>);
fn recorder(app: &tauri::AppHandle) -> Result<Arc<recording::Recorder>, String> {
    app.state::<RecorderState>().0.clone()
}

#[tauri::command]
fn recording_status(app: tauri::AppHandle) -> Result<recording::Status, String> {
    recorder(&app)?.status()
}
#[tauri::command]
async fn recording_devices(app: tauri::AppHandle) -> Result<recording::Devices, String> {
    let recorder = recorder(&app)?;
    tauri::async_runtime::spawn_blocking(move || recorder.devices())
        .await
        .map_err(|_| "Audio device discovery failed.")?
}
#[tauri::command]
async fn recording_start(
    app: tauri::AppHandle,
    selection: recording::Selection,
) -> Result<recording::Status, String> {
    let recorder = recorder(&app)?;
    tauri::async_runtime::spawn_blocking(move || recorder.start(selection))
        .await
        .map_err(|_| "Recording initialization failed.")?
}
#[tauri::command]
async fn recording_control(
    app: tauri::AppHandle,
    action: recording::Action,
) -> Result<recording::Status, String> {
    let recorder = recorder(&app)?;
    tauri::async_runtime::spawn_blocking(move || recorder.control(action))
        .await
        .map_err(|_| "Recording control failed.")?
}
#[tauri::command]
async fn recording_recover(app: tauri::AppHandle) -> Result<(), String> {
    let recorder = recorder(&app)?;
    tauri::async_runtime::spawn_blocking(move || recorder.recover())
        .await
        .map_err(|_| "Recording recovery failed.")?
}

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
        .setup(|app| {
            let initialized = (|| -> Result<_, String> {
                let database_directory = app
                    .path()
                    .app_data_dir()
                    .map_err(|_| "Unable to locate local database storage.")?;
                std::fs::create_dir_all(&database_directory)
                    .map_err(|_| "Unable to access local database storage.")?;
                let local_directory = app
                    .path()
                    .app_local_data_dir()
                    .map_err(|_| "Unable to locate local audio storage.")?;
                recording::Recorder::new(
                    local_directory.join("recordings"),
                    database_directory.join("workspace.sqlite3"),
                )
                .map(Arc::new)
            })();
            app.manage(RecorderState(initialized));
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("recording", |context, request, responder| {
            match recorder(context.app_handle()) {
                Ok(recorder) => {
                    tauri::async_runtime::spawn_blocking(move || {
                        responder.respond(recording::response(
                            &recorder.root,
                            &recorder.db_path,
                            request,
                        ))
                    });
                }
                Err(_) => responder.respond(
                    tauri::http::Response::builder()
                        .status(503)
                        .body(b"Recording storage is unavailable.".to_vec())
                        .unwrap(),
                ),
            }
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Ok(recorder) = recorder(window.app_handle()) {
                    if recorder.active() {
                        api.prevent_close();
                        recorder.close_warning();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            data_request,
            recording_status,
            recording_devices,
            recording_start,
            recording_control,
            recording_recover
        ])
        .run(tauri::generate_context!())
        .expect("Failed to launch Meeting Intelligence Workspace");
}
