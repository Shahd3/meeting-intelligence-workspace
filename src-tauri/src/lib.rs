mod desktop_widget;
mod detection;
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

#[tauri::command]
fn desktop_widget_status(app: tauri::AppHandle) -> Result<desktop_widget::View, String> {
    app.state::<Arc<desktop_widget::DesktopWidget>>()
        .view(recorder(&app)?.status()?)
}
#[tauri::command]
async fn desktop_detection_set(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let widget = app
        .state::<Arc<desktop_widget::DesktopWidget>>()
        .inner()
        .clone();
    tauri::async_runtime::spawn_blocking(move || widget.set_enabled(enabled))
        .await
        .map_err(|_| "Unable to save recording preferences.")?
}
#[tauri::command]
fn desktop_widget_dismiss(app: tauri::AppHandle) -> Result<(), String> {
    app.state::<Arc<desktop_widget::DesktopWidget>>().dismiss()
}
#[tauri::command]
fn desktop_widget_open(app: tauri::AppHandle, setup: bool) -> Result<(), String> {
    app.state::<Arc<desktop_widget::DesktopWidget>>()
        .open_main(&app, setup)
}
#[tauri::command]
fn desktop_widget_take_setup(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<bool, String> {
    if window.label() != "main" {
        return Err("Recording setup belongs to the main window.".into());
    }
    Ok(app
        .state::<Arc<desktop_widget::DesktopWidget>>()
        .take_setup())
}
#[tauri::command]
fn desktop_widget_drag(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "recording-widget" {
        return Err("Only the floating widget can be dragged here.".into());
    }
    window
        .start_dragging()
        .map_err(|_| "Unable to move the recording widget.".into())
}
#[tauri::command]
fn desktop_widget_acknowledge(app: tauri::AppHandle) -> Result<(), String> {
    app.state::<Arc<desktop_widget::DesktopWidget>>()
        .acknowledge_failure(&recorder(&app)?.status()?)
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
            let database = app.path().app_data_dir()?.join("workspace.sqlite3");
            let widget = Arc::new(desktop_widget::DesktopWidget::new(database));
            app.manage(widget.clone());
            widget.run(app.handle().clone());
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
                if window.label() == "recording-widget" {
                    api.prevent_close();
                    let widget = window
                        .app_handle()
                        .state::<Arc<desktop_widget::DesktopWidget>>();
                    let _ = widget.dismiss();
                    if let Ok(recorder) = recorder(window.app_handle()) {
                        if recorder.active()
                            || recorder
                                .status()
                                .map(|status| status.state == recording::RecordingState::Failed)
                                .unwrap_or(false)
                        {
                            let _ = widget.open_main(window.app_handle(), false);
                        }
                    }
                    let _ = window.hide();
                    return;
                }
                if let Ok(recorder) = recorder(window.app_handle()) {
                    if recorder.active() {
                        api.prevent_close();
                        recorder.close_warning();
                    } else {
                        window.app_handle().exit(0);
                    }
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            data_request,
            recording_status,
            recording_devices,
            recording_start,
            recording_control,
            recording_recover,
            desktop_widget_status,
            desktop_detection_set,
            desktop_widget_dismiss,
            desktop_widget_open,
            desktop_widget_take_setup,
            desktop_widget_drag,
            desktop_widget_acknowledge
        ])
        .build(tauri::generate_context!())
        .expect("Failed to launch Meeting Intelligence Workspace")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if recorder(app)
                    .map(|recorder| recorder.active())
                    .unwrap_or(false)
                {
                    api.prevent_exit();
                    return;
                }
                app.state::<Arc<desktop_widget::DesktopWidget>>()
                    .shutdown
                    .store(true, std::sync::atomic::Ordering::Release);
            }
        });
}
