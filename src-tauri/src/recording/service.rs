use super::*;
use fs2::FileExt;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Idle,
    Starting,
    Recording,
    Paused,
    Stopping,
    Saved,
    Failed,
}
impl State {
    pub fn active(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Recording | Self::Paused | Self::Stopping
        )
    }
    fn allows(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Idle | Self::Saved | Self::Failed, Self::Starting)
                | (Self::Starting, Self::Recording | Self::Failed)
                | (
                    Self::Recording,
                    Self::Paused | Self::Stopping | Self::Failed
                )
                | (
                    Self::Paused,
                    Self::Recording | Self::Stopping | Self::Failed
                )
                | (Self::Stopping, Self::Saved | Self::Failed)
        )
    }
}
#[derive(Clone, Serialize)]
pub struct Status {
    pub state: State,
    pub id: Option<String>,
    pub elapsed_ms: u64,
    pub meeting_id: Option<i64>,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub peak: f32,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            state: State::Idle,
            id: None,
            elapsed_ms: 0,
            meeting_id: None,
            error: None,
            warning: None,
            peak: 0.0,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Pause,
    Resume,
    Stop,
}
struct Session {
    sender: SyncSender<Action>,
    thread: JoinHandle<()>,
}
pub struct Recorder {
    pub root: PathBuf,
    pub db_path: PathBuf,
    _ownership: File,
    status: Arc<Mutex<Status>>,
    session: Mutex<Option<Session>>,
    stop_requested: AtomicBool,
}
impl Recorder {
    pub fn new(root: PathBuf, db_path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root)
            .map_err(|_| "Unable to create the local recording directory.")?;
        let ownership = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("recorder.lock"))
            .map_err(|_| "Unable to acquire recording ownership.")?;
        ownership.try_lock_exclusive().map_err(|_| {
            "Another application instance owns recording. Close it before recording or recovery."
        })?;
        let mut db = crate::storage::open(&db_path)?;
        recover(&mut db, &root)?;
        Ok(Self {
            root,
            db_path,
            _ownership: ownership,
            status: Arc::new(Mutex::new(Status::default())),
            session: Mutex::new(None),
            stop_requested: AtomicBool::new(false),
        })
    }
    pub fn status(&self) -> Result<Status> {
        self.status
            .lock()
            .map(|s| s.clone())
            .map_err(|_| "Recording state is unavailable.".into())
    }
    pub fn active(&self) -> bool {
        self.status().map(|s| s.state.active()).unwrap_or(true)
    }
    pub fn devices(&self) -> Result<Devices> {
        #[cfg(windows)]
        {
            super::windows::devices()
        }
        #[cfg(not(windows))]
        {
            Err("Native capture currently requires Windows.".into())
        }
    }
    pub fn start(&self, selection: Selection) -> Result<Status> {
        selection.validate()?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| "Recording control is unavailable.")?;
        if self.active() {
            return Err("A recording session is already active.".into());
        }
        if let Some(old) = session.take() {
            old.thread
                .join()
                .map_err(|_| "Previous recording worker failed. Restart to recover its audio.")?;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let (mut initial, root, db_path) =
            (self.status()?, self.root.clone(), self.db_path.clone());
        if !initial.state.allows(State::Starting) {
            return Err("Cannot start recording from the current state.".into());
        }
        self.stop_requested.store(false, Ordering::Release);
        initial = Status {
            state: State::Starting,
            id: Some(id.clone()),
            ..Status::default()
        };
        *self
            .status
            .lock()
            .map_err(|_| "Recording state is unavailable.")? = initial.clone();
        let status = self.status.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let spawned=thread::Builder::new().name("meeting-audio-capture".into()).spawn(move||{
            let outcome=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||run(&root,&db_path,&id,selection,receiver,&status)));
            let error=match outcome {Ok(Ok(()))=>None,Ok(Err(error))=>Some(error),Err(_)=>Some("Recording worker failed unexpectedly. Restart or recover to preserve available audio.".into())};
            if let Some(error)=error {
                if let Ok(mut db)=crate::storage::open(&db_path){mark_failed(&mut db,&id,&error);}
                if let Ok(mut s)=status.lock(){s.state=State::Failed;s.error=Some(error);s.peak=0.0;}
            }
        });
        match spawned {
            Ok(thread) => {
                *session = Some(Session { sender, thread });
                Ok(initial)
            }
            Err(_) => {
                if let Ok(mut s) = self.status.lock() {
                    s.state = State::Failed;
                }
                Err("Unable to start the audio worker.".into())
            }
        }
    }
    pub fn control(&self, action: Action) -> Result<Status> {
        let session = self
            .session
            .lock()
            .map_err(|_| "Recording control is unavailable.")?;
        let s = self.status()?;
        if self.stop_requested.load(Ordering::Acquire) {
            return if action == Action::Stop {
                Ok(s)
            } else {
                Err("Saving is already in progress.".into())
            };
        }
        let allowed = matches!(
            (s.state, action),
            (State::Recording, Action::Pause | Action::Stop)
                | (State::Paused, Action::Resume | Action::Stop)
        );
        if !allowed {
            return Err("This recording action is not available in the current state.".into());
        }
        let worker = session.as_ref().ok_or("Recording worker is unavailable.")?;
        // The session mutex serializes controls across every webview. Latch Stop
        // before sending so a consumed queue slot cannot admit a second request.
        if action == Action::Stop {
            self.stop_requested.store(true, Ordering::Release);
        }
        if worker.sender.try_send(action).is_err() {
            if action == Action::Stop {
                self.stop_requested.store(false, Ordering::Release);
            }
            return Err("A recording action is already pending or the worker stopped.".into());
        }
        Ok(s)
    }
    pub fn recover(&self) -> Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| "Recording control is unavailable.")?;
        if self.active() {
            return Err("Stop active capture before recovery.".into());
        }
        if let Some(old) = session.take() {
            let _ = old.thread.join();
        }
        let mut db = crate::storage::open(&self.db_path)?;
        super::recover(&mut db, &self.root)
    }
    pub fn close_warning(&self) {
        if let Ok(mut s) = self.status.lock() {
            s.warning = Some(
                "Stop recording before closing the application. Capture is still active.".into(),
            );
        }
    }
}

#[derive(Default)]
struct Timeline {
    base: u64,
    epoch: Option<u64>,
}
impl Timeline {
    fn resume(&mut self, now: u64) {
        self.epoch = Some(now);
    }
    fn frames(&self, now: u64) -> u64 {
        self.base
            + self
                .epoch
                .map(|epoch| now.saturating_sub(epoch) * wav::RATE as u64 / 10_000_000)
                .unwrap_or(0)
    }
    fn pause(&mut self, now: u64) {
        self.base = self.frames(now);
        self.epoch = None;
    }
    fn packet(&self, timestamp: u64, pcm: &[u8]) -> Option<(u64, usize)> {
        let epoch = self.epoch?;
        let skip = if timestamp < epoch {
            (epoch - timestamp)
                .saturating_mul(wav::RATE as u64)
                .div_ceil(10_000_000) as usize
        } else {
            0
        };
        if skip >= pcm.len() / 4 {
            return None;
        }
        Some((
            self.base.saturating_add(
                timestamp
                    .saturating_sub(epoch)
                    .saturating_mul(wav::RATE as u64)
                    / 10_000_000,
            ),
            skip * 4,
        ))
    }
}
fn transition(status: &Arc<Mutex<Status>>, next: State) -> Result<()> {
    let mut s = status
        .lock()
        .map_err(|_| "Recording state is unavailable.")?;
    if !s.state.allows(next) {
        return Err("Invalid recording state transition.".into());
    }
    s.state = next;
    Ok(())
}

#[cfg(not(windows))]
fn run(
    _: &Path,
    _: &Path,
    _: &str,
    _: Selection,
    _: Receiver<Action>,
    _: &Arc<Mutex<Status>>,
) -> Result<()> {
    Err("Native audio capture currently requires Windows.".into())
}

#[cfg(windows)]
fn run(
    root: &Path,
    db_path: &Path,
    id: &str,
    selection: Selection,
    receiver: Receiver<Action>,
    status: &Arc<Mutex<Status>>,
) -> Result<()> {
    use super::windows::{self, Source};
    let _com = windows::Com::enter()?;
    // Open every requested endpoint before starting any capture.
    let sources = windows::open(&selection)?;
    let (title, title_source) = super::initial_title(&selection.title);
    let mut record = Recording {
        id: id.into(),
        meeting_id: None,
        title,
        title_source,
        state: "starting".into(),
        elapsed_ms: 0,
        recovered: false,
        complete: false,
        warning: None,
        created_at: String::new(),
        tracks: sources
            .iter()
            .map(|s| Track {
                role: s.role.clone(),
                device_id: s.device.id.clone(),
                device_name: s.device.name.clone(),
                filename: format!("{}.wav.part", s.role),
                frames: 0,
                sample_rate: wav::RATE,
                channels: 2,
            })
            .collect(),
    };
    let mut db = crate::storage::open(db_path)?;
    register(&mut db, &record)?;
    let directory = root.join(id);
    std::fs::create_dir(&directory).map_err(|_| "Unable to create recording files.")?;
    let mut writers = record
        .tracks
        .iter()
        .map(|t| wav::Wav::create(&directory.join(&t.filename)))
        .collect::<Result<Vec<_>>>()?;
    let mut timeline = Timeline::default();
    let mut seen = vec![false; sources.len()];
    fn drain(
        sources: &[Source],
        writers: &mut [wav::Wav],
        timeline: &Timeline,
        record: &mut Recording,
        seen: &mut [bool],
    ) -> Result<f32> {
        let mut peak = 0f32;
        for (index, source) in sources.iter().enumerate() {
            // Bound work per poll to avoid starvation of stop/pause commands.
            for _ in 0..100 {
                let Some(packet) = source.packet()? else {
                    break;
                };
                if packet.discontinuity && seen[index] {
                    record.warning=Some("Windows reported an audio discontinuity. Timestamp gaps are preserved as silence; review playback.".into());
                    record.recovered = true;
                }
                seen[index] = true;
                if let Some((target, skip)) = timeline.packet(packet.timestamp, &packet.pcm) {
                    if target
                        > timeline
                            .frames(windows::now()?)
                            .saturating_add(wav::RATE as u64)
                    {
                        return Err("Windows returned an audio timestamp outside the recording timeline. Available audio was retained.".into());
                    }
                    for sample in packet.pcm[skip..].chunks_exact(2) {
                        peak = peak.max(
                            i16::from_le_bytes([sample[0], sample[1]]).unsigned_abs() as f32
                                / 32768.0,
                        );
                    }
                    writers[index].append_at(target, &packet.pcm[skip..])?;
                }
            }
        }
        Ok(peak)
    }
    let mut checkpoint_at = Instant::now();
    let outcome = (|| {
        timeline.resume(windows::now()?);
        for source in &sources {
            source.start()?;
        }
        transition(status, State::Recording)?;
        record.state = "recording".into();
        checkpoint(&mut db, &record)?;
        loop {
            if let Ok(action) = receiver.try_recv() {
                match action {
                    Action::Pause => {
                        if timeline.epoch.is_none() {
                            continue;
                        }
                        for source in &sources {
                            source.pause()?;
                        }
                        drain(&sources, &mut writers, &timeline, &mut record, &mut seen)?;
                        timeline.pause(windows::now()?);
                        transition(status, State::Paused)?;
                        record.state = "paused".into();
                        record.elapsed_ms = timeline.base * 1000 / wav::RATE as u64;
                        for (index, w) in writers.iter_mut().enumerate() {
                            w.pad_to(timeline.base)?;
                            w.checkpoint()?;
                            record.tracks[index].frames = w.frames;
                        }
                        checkpoint(&mut db, &record)?;
                    }
                    Action::Resume => {
                        if timeline.epoch.is_some() {
                            continue;
                        }
                        for source in &sources {
                            source.reset()?;
                        }
                        timeline.resume(windows::now()?);
                        seen.fill(false);
                        for source in &sources {
                            source.start()?;
                        }
                        transition(status, State::Recording)?;
                        record.state = "recording".into();
                        checkpoint(&mut db, &record)?;
                    }
                    Action::Stop => {
                        transition(status, State::Stopping)?;
                        record.state = "stopping".into();
                        if timeline.epoch.is_some() {
                            for source in &sources {
                                source.pause()?;
                            }
                            drain(&sources, &mut writers, &timeline, &mut record, &mut seen)?;
                            timeline.pause(windows::now()?);
                        }
                        break;
                    }
                }
            }
            let now = windows::now()?;
            let frames = timeline.frames(now);
            if frames >= wav::MAX_FRAMES {
                for source in &sources {
                    source.pause()?;
                }
                timeline.pause(now);
                timeline.base = wav::MAX_FRAMES;
                transition(status, State::Stopping)?;
                record.state = "stopping".into();
                record.warning = Some(
                    "The four-hour WAV limit was reached; capture stopped and audio was preserved."
                        .into(),
                );
                break;
            }
            let peak = if timeline.epoch.is_some() {
                drain(&sources, &mut writers, &timeline, &mut record, &mut seen)?
            } else {
                0.0
            };
            record.elapsed_ms = frames * 1000 / wav::RATE as u64;
            if let Ok(mut s) = status.lock() {
                s.elapsed_ms = record.elapsed_ms;
                s.peak = peak;
                if record.warning.is_some() || s.warning.is_none() {
                    s.warning = record.warning.clone();
                }
            }
            if checkpoint_at.elapsed() >= Duration::from_secs(1) {
                for source in &sources {
                    source.alive()?;
                }
                for (index, w) in writers.iter_mut().enumerate() {
                    w.checkpoint()?;
                    record.tracks[index].frames = w.frames;
                }
                checkpoint(&mut db, &record)?;
                checkpoint_at = Instant::now();
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    })();
    let capture_error: Option<String> = outcome.err();
    for source in &sources {
        let _ = source.pause();
    }
    if timeline.epoch.is_some() {
        timeline.pause(windows::now()?);
    }
    let frames = timeline.base.min(wav::MAX_FRAMES);
    record.elapsed_ms = frames * 1000 / wav::RATE as u64;
    record.complete = capture_error.is_none() && !record.recovered;
    record.recovered |= capture_error.is_some();
    record.state = if capture_error.is_some() {
        "failed"
    } else {
        "saved"
    }
    .into();
    if let Some(error) = &capture_error {
        record.warning = Some(error.clone());
    }
    for (index, writer) in writers.into_iter().enumerate() {
        let frames = writer.finish(frames)?;
        let source = directory.join(&record.tracks[index].filename);
        let filename = format!("{}.wav", record.tracks[index].role);
        std::fs::rename(&source, directory.join(&filename)).map_err(|_| {
            "Audio is preserved, but safe file finalization failed. Retry recovery."
        })?;
        record.tracks[index].filename = filename;
        record.tracks[index].frames = frames;
    }
    write_marker(&directory, &record)?;
    commit_result(&mut db, &mut record)?;
    if let Ok(mut s) = status.lock() {
        s.state = if capture_error.is_some() {
            State::Failed
        } else {
            State::Saved
        };
        s.elapsed_ms = record.elapsed_ms;
        s.meeting_id = record.meeting_id;
        s.warning = record.warning;
        s.error = capture_error;
        s.peak = 0.0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_rejects_duplicate_start_and_invalid_resume() {
        assert!(State::Idle.allows(State::Starting));
        assert!(!State::Recording.allows(State::Starting));
        assert!(!State::Paused.allows(State::Starting));
        assert!(State::Paused.allows(State::Recording));
        assert!(State::Stopping.allows(State::Saved));
        assert!(!State::Idle.allows(State::Recording));
    }
    #[test]
    fn paused_time_is_excluded_from_duration_and_packet_alignment() {
        let mut clock = Timeline::default();
        clock.resume(10_000_000);
        assert_eq!(clock.frames(20_000_000), 48_000);
        clock.pause(20_000_000);
        assert_eq!(clock.frames(100_000_000), 48_000);
        clock.resume(100_000_000);
        assert_eq!(clock.frames(110_000_000), 96_000);
        assert_eq!(clock.packet(100_000_000, &[0; 4]), Some((48_000, 0)));
    }

    #[test]
    fn recorder_rejects_start_in_every_active_state_before_opening_devices() {
        let directory =
            std::env::temp_dir().join(format!("miw-state-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let recorder =
            Recorder::new(directory.join("recordings"), directory.join("test.sqlite3")).unwrap();
        for state in [
            State::Starting,
            State::Recording,
            State::Paused,
            State::Stopping,
        ] {
            recorder.status.lock().unwrap().state = state;
            let selection = Selection {
                microphone: true,
                system: false,
                microphone_device: None,
                system_device: None,
                title: "Duplicate guard test".into(),
            };
            assert!(recorder
                .start(selection)
                .err()
                .unwrap()
                .contains("already active"));
            assert!(recorder.session.lock().unwrap().is_none());
        }
        drop(recorder);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn stop_from_two_windows_is_idempotent_even_after_queue_consumption() {
        let directory =
            std::env::temp_dir().join(format!("miw-stop-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let recorder =
            Recorder::new(directory.join("recordings"), directory.join("test.sqlite3")).unwrap();
        recorder.status.lock().unwrap().state = State::Recording;
        let (sender, receiver) = mpsc::sync_channel(1);
        *recorder.session.lock().unwrap() = Some(Session {
            sender,
            thread: std::thread::spawn(|| {}),
        });
        recorder.control(Action::Stop).unwrap();
        assert!(matches!(receiver.try_recv(), Ok(Action::Stop)));
        recorder.control(Action::Stop).unwrap();
        assert!(receiver.try_recv().is_err());
        assert!(recorder.control(Action::Pause).is_err());
        recorder.status.lock().unwrap().state = State::Saved;
        recorder.control(Action::Stop).unwrap();
        assert!(receiver.try_recv().is_err());
        drop(recorder);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
