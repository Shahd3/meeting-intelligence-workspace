//! One native window observes the existing recorder; it never owns capture.
use crate::{
    detection::{self, Observation, Policy},
    recorder,
    recording::{RecordingState, Status},
    storage,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{LogicalSize, Manager, PhysicalPosition};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Hidden,
    Suggestion,
    Recording,
    Failed,
}
#[derive(Clone, Serialize)]
pub struct Suggestion {
    pub application: &'static str,
    pub explanation: &'static str,
}
#[derive(Clone, Serialize)]
pub struct View {
    pub mode: Mode,
    pub recording: Status,
    pub suggestion: Option<Suggestion>,
    pub detection_enabled: bool,
    pub detection_error: Option<String>,
    pub widget_error: Option<String>,
    pub setup_requested: bool,
}
struct Inner {
    enabled: bool,
    policy: Policy,
    suggestion: Option<Observation>,
    detection_error: Option<String>,
    widget_error: Option<String>,
    acknowledged_failure: Option<String>,
    positioned: bool,
}
pub struct DesktopWidget {
    inner: Mutex<Inner>,
    database: PathBuf,
    pub shutdown: AtomicBool,
    setup: AtomicBool,
}
pub fn mode(status: &Status, suggestion: bool, acknowledged: bool) -> Mode {
    match status.state {
        RecordingState::Recording | RecordingState::Paused | RecordingState::Stopping => {
            Mode::Recording
        }
        RecordingState::Starting => Mode::Hidden,
        RecordingState::Failed if status.id.is_some() && !acknowledged => Mode::Failed,
        _ if suggestion => Mode::Suggestion,
        _ => Mode::Hidden,
    }
}
impl DesktopWidget {
    pub fn new(database: PathBuf) -> Self {
        let preference = storage::open(&database).and_then(|db| storage::detection_enabled(&db));
        let (enabled, detection_error) = match preference {
            Ok(enabled) => (enabled, None),
            Err(error) => (false, Some(error)),
        };
        Self {
            database,
            inner: Mutex::new(Inner {
                enabled,
                policy: Policy::default(),
                suggestion: None,
                detection_error,
                widget_error: None,
                acknowledged_failure: None,
                positioned: false,
            }),
            shutdown: AtomicBool::new(false),
            setup: AtomicBool::new(false),
        }
    }
    pub fn view(&self, recording: Status) -> Result<View, String> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| "Desktop widget is unavailable.")?;
        Ok(View {
            mode: mode(
                &recording,
                inner.suggestion.is_some(),
                recording.id.is_some() && recording.id == inner.acknowledged_failure,
            ),
            recording,
            suggestion: inner.suggestion.as_ref().map(|item| Suggestion {
                application: item.application.label(),
                explanation: item.explanation(),
            }),
            detection_enabled: inner.enabled,
            detection_error: inner.detection_error.clone(),
            widget_error: inner.widget_error.clone(),
            setup_requested: self.setup.load(Ordering::Acquire),
        })
    }
    pub fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        // Serialize preference changes with detection state; persist before applying.
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Desktop preferences are unavailable.")?;
        let db = storage::open(&self.database)?;
        storage::set_detection_enabled(&db, enabled)?;
        inner.enabled = enabled;
        inner.policy.clear();
        inner.suggestion = None;
        inner.detection_error = None;
        Ok(())
    }
    pub fn dismiss(&self) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Desktop widget is unavailable.")?;
        if let Some(suggestion) = inner.suggestion.take() {
            inner.policy.dismiss(suggestion.key);
        }
        Ok(())
    }
    pub fn open_main(&self, app: &tauri::AppHandle, setup: bool) -> Result<(), String> {
        if setup {
            self.dismiss()?;
            self.setup.store(true, Ordering::Release);
        }
        let main = app
            .get_webview_window("main")
            .ok_or("Main window is unavailable.")?;
        main.unminimize()
            .and_then(|_| main.show())
            .and_then(|_| main.set_focus())
            .map_err(|_| "Unable to open the main window.".into())
    }
    pub fn take_setup(&self) -> bool {
        self.setup.swap(false, Ordering::AcqRel)
    }
    pub fn acknowledge_failure(&self, status: &Status) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Desktop widget is unavailable.")?;
        inner.acknowledged_failure = status.id.clone();
        Ok(())
    }
    pub fn run(self: Arc<Self>, app: tauri::AppHandle) {
        let worker = self.clone();
        if std::thread::Builder::new()
            .name("desktop-recording-widget".into())
            .spawn(move || worker.monitor(app))
            .is_err()
        {
            if let Ok(mut inner) = self.inner.lock() {
                inner.widget_error=Some("The floating widget could not start. Main recording controls remain available.".into());
            }
        }
    }
    fn monitor(&self, app: tauri::AppHandle) {
        let mut next_scan = Instant::now();
        let mut previous = Mode::Hidden;
        let mut suppress_after_capture = false;
        while !self.shutdown.load(Ordering::Acquire) {
            let status = recorder(&app).and_then(|recorder| recorder.status());
            if let Ok(status) = status {
                let enabled = self
                    .inner
                    .lock()
                    .map(|inner| inner.enabled)
                    .unwrap_or(false);
                if !enabled {
                    suppress_after_capture = false;
                }
                if status.state.active() {
                    suppress_after_capture = enabled;
                    if let Ok(mut inner) = self.inner.lock() {
                        inner.policy.suppress_present();
                        if let Some(suggestion) = inner.suggestion.take() {
                            inner.policy.dismiss(suggestion.key);
                        }
                    }
                } else if enabled && Instant::now() >= next_scan {
                    next_scan = Instant::now() + Duration::from_secs(5);
                    let observations = detection::scan();
                    if let Ok(mut inner) = self.inner.lock() {
                        // Opt-out during an in-flight scan discards its result.
                        if inner.enabled {
                            match observations {
                                Ok(windows) => {
                                    if suppress_after_capture {
                                        for window in
                                            windows.iter().filter(|window| window.likely())
                                        {
                                            inner.policy.dismiss(window.key);
                                        }
                                        suppress_after_capture = false;
                                    }
                                    inner.detection_error = None;
                                    if let Some(current) = &inner.suggestion {
                                        if !windows.iter().any(|window| {
                                            window.key == current.key && window.likely()
                                        }) {
                                            inner.suggestion = None;
                                        }
                                    }
                                    let suggestion = inner.policy.observe(&windows);
                                    if inner.suggestion.is_none() {
                                        inner.suggestion = suggestion;
                                    }
                                }
                                Err(error) => {
                                    inner.detection_error = Some(error);
                                    inner.suggestion = None;
                                }
                            }
                        }
                    }
                }
                if let Ok(view) = self.view(status) {
                    if let Some(window) = app.get_webview_window("recording-widget") {
                        let changed = view.mode != previous;
                        let visible = view.mode != Mode::Hidden;
                        if changed && visible {
                            let size = if view.mode == Mode::Recording {
                                LogicalSize::new(280.0, 130.0)
                            } else {
                                LogicalSize::new(340.0, 240.0)
                            };
                            let _ = window.set_size(size);
                        }
                        if changed || (visible && !window.is_visible().unwrap_or(false)) {
                            let outcome = if visible {
                                // Physical work-area coordinates are derived from monitor scaling,
                                // while window sizing is in logical pixels. User dragging is retained.
                                self.position(&window);
                                window.show()
                            } else {
                                window.hide()
                            };
                            if let Ok(mut inner) = self.inner.lock() {
                                inner.widget_error=outcome.err().map(|_|"The floating widget could not be shown. Use the main recording controls.".into());
                            }
                        }
                        previous = view.mode;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    fn position(&self, window: &tauri::WebviewWindow) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if inner.positioned {
            return;
        }
        if let Ok(Some(monitor)) = window.current_monitor() {
            let area = monitor.work_area();
            let scale = monitor.scale_factor();
            let size = window.outer_size().unwrap_or_default();
            let x = area.position.x + area.size.width as i32
                - size.width as i32
                - (20.0 * scale) as i32;
            let y = area.position.y + area.size.height as i32
                - size.height as i32
                - (30.0 * scale) as i32;
            if window
                .set_position(PhysicalPosition::new(
                    x.max(area.position.x),
                    y.max(area.position.y),
                ))
                .is_ok()
            {
                inner.positioned = true;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_wins_over_suggestion_and_save_hides_widget() {
        for state in [
            RecordingState::Recording,
            RecordingState::Paused,
            RecordingState::Stopping,
        ] {
            assert_eq!(
                mode(
                    &Status {
                        state,
                        ..Status::default()
                    },
                    true,
                    false
                ),
                Mode::Recording
            );
        }
        assert_eq!(
            mode(
                &Status {
                    state: RecordingState::Starting,
                    ..Status::default()
                },
                true,
                false
            ),
            Mode::Hidden
        );
        assert_eq!(
            mode(
                &Status {
                    state: RecordingState::Saved,
                    ..Status::default()
                },
                false,
                false
            ),
            Mode::Hidden
        );
    }
    #[test]
    fn failed_capture_keeps_recovery_access() {
        let failed = Status {
            state: RecordingState::Failed,
            id: Some("test".into()),
            ..Status::default()
        };
        assert_eq!(mode(&failed, true, false), Mode::Failed);
        assert_eq!(mode(&failed, false, true), Mode::Hidden);
    }
    #[test]
    fn preference_is_opt_in_and_survives_restart() {
        let directory = std::env::temp_dir().join(format!("miw-widget-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("test.sqlite3");
        let service = DesktopWidget::new(path.clone());
        assert!(!service.view(Status::default()).unwrap().detection_enabled);
        service.set_enabled(true).unwrap();
        drop(service);
        let restarted = DesktopWidget::new(path.clone());
        assert!(restarted.view(Status::default()).unwrap().detection_enabled);
        restarted.set_enabled(false).unwrap();
        assert!(!restarted.view(Status::default()).unwrap().detection_enabled);
        drop(restarted);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
