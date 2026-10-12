mod desktop_widget;
mod detection;
mod intelligence;
mod processing;
mod recording;
mod storage;
use std::sync::Arc;
use tauri::Manager;
// Debug-only isolation for desktop validation; release builds always use per-user paths.
fn data_directory(app: &tauri::AppHandle, local: bool) -> Result<std::path::PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(directory) = std::env::var_os("MIW_DEV_DATA_DIR") {
        let path = std::path::PathBuf::from(directory);
        if !path.is_absolute() {
            return Err("MIW_DEV_DATA_DIR must be an absolute test directory.".into());
        }
        return Ok(path.join(if local { "local" } else { "data" }));
    }
    (if local {
        app.path().app_local_data_dir()
    } else {
        app.path().app_data_dir()
    })
    .map_err(|_| "Unable to locate application data storage.".into())
}
struct RecorderState(Result<Arc<recording::Recorder>, String>);
#[tauri::command]
async fn ai_settings_save(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    request: processing::settings::Save,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("AI settings belong to the main window.".into());
    }
    let worker = app.state::<Arc<processing::Pipeline>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        processing::settings::save(
            &mut storage::open(&worker.database)?,
            request,
            &processing::credentials::WindowsVault,
        )
    })
    .await
    .map_err(|_| "Unable to save AI settings.")?
}
#[tauri::command]
async fn ai_connection_test(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    kind: String,
    revision: String,
    authorized: bool,
) -> Result<String, String> {
    if window.label() != "main"
        || !authorized
        || !["transcription", "summarization"].contains(&kind.as_str())
    {
        return Err("Explicit approval of this saved connection test is required.".into());
    }
    let worker = app.state::<Arc<processing::Pipeline>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let engine = processing::settings::load(&storage::open(&worker.database)?, &kind)?
            .ok_or("Save these settings before testing.")?;
        if engine.revision() != revision {
            return Err("Settings changed. Review the saved destination before testing.".into());
        }
        if kind == "summarization" {
            intelligence::connection_test(&intelligence::config(&storage::open(&worker.database)?)?)
        } else {
            processing::test_connection(&engine, &kind)
        }
    })
    .await
    .map_err(|_| "Connection test could not finish.")?
}
#[tauri::command]
async fn processing_rebind(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
    revision: String,
    authorize: bool,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Upload approval belongs to the main window.".into());
    }
    let worker = app.state::<Arc<processing::Pipeline>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        processing::settings::rebind(
            &mut storage::open(&worker.database)?,
            &id,
            &revision,
            authorize,
        )
    })
    .await
    .map_err(|_| "Unable to apply job settings.")?
}
#[tauri::command]
async fn intelligence_export(
    window: tauri::WebviewWindow,
    contents: String,
) -> Result<bool, String> {
    if window.label() != "main" {
        return Err("Minutes export belongs to the main window.".into());
    }
    tauri::async_runtime::spawn_blocking(move || intelligence::export_markdown(contents))
        .await
        .map_err(|_| "Minutes export could not finish.")?
}
#[tauri::command]
async fn intelligence_request(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    request: intelligence::Request,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Intelligence changes belong to the main window.".into());
    }
    let worker = app.state::<Arc<intelligence::Pipeline>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        intelligence::mutate(&mut storage::open(&worker.database)?, request)
    })
    .await
    .map_err(|_| "Unable to update meeting intelligence.")?
}
#[tauri::command]
async fn processing_status(
    app: tauri::AppHandle,
    meeting_id: Option<i64>,
) -> Result<processing::View, String> {
    let worker = app.state::<Arc<processing::Pipeline>>().inner().clone();
    let generation = app.state::<Arc<intelligence::Pipeline>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut view = worker.status(meeting_id)?;
        view.intelligence.worker_error = generation.error();
        Ok(view)
    })
    .await
    .map_err(|_| "Unable to read processing status.")?
}
#[tauri::command]
async fn processing_configure(
    app: tauri::AppHandle,
    config: processing::Config,
) -> Result<(), String> {
    let worker = app.state::<Arc<processing::Pipeline>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        processing::set_config(&mut storage::open(&worker.database)?, config)
    })
    .await
    .map_err(|_| "Unable to configure transcription.")?
}
#[tauri::command]
async fn processing_retry(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let worker = app.state::<Arc<processing::Pipeline>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        processing::retry(&storage::open(&worker.database)?, &id)
    })
    .await
    .map_err(|_| "Unable to retry transcription.")?
}
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
    let directory = data_directory(&app, false)?;
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
                let database_directory = data_directory(app.handle(), false)?;
                std::fs::create_dir_all(&database_directory)
                    .map_err(|_| "Unable to access local database storage.")?;
                let local_directory = data_directory(app.handle(), true)?;
                recording::Recorder::new(
                    local_directory.join("recordings"),
                    database_directory.join("workspace.sqlite3"),
                )
                .map(Arc::new)
            })();
            app.manage(RecorderState(initialized));
            let database = data_directory(app.handle(), false)?.join("workspace.sqlite3");
            let pipeline = processing::Pipeline::new(
                database.clone(),
                data_directory(app.handle(), true)?.join("recordings"),
            );
            app.manage(pipeline.clone());
            pipeline.start();
            let generation = intelligence::Pipeline::new(database.clone());
            app.manage(generation.clone());
            generation.start();
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
            intelligence_export,
            intelligence_request,
            ai_settings_save,
            ai_connection_test,
            processing_rebind,
            processing_status,
            processing_configure,
            processing_retry,
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
                app.state::<Arc<intelligence::Pipeline>>()
                    .shutdown
                    .store(true, std::sync::atomic::Ordering::Release);
                app.state::<Arc<processing::Pipeline>>()
                    .shutdown
                    .store(true, std::sync::atomic::Ordering::Release);
            }
        });
}
