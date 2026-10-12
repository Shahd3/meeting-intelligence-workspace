use super::*;
use crate::storage::{self, Request};

struct Fixture {
    directory: std::path::PathBuf,
    root: std::path::PathBuf,
    db_path: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("miw-recording-test-{}", uuid::Uuid::new_v4()));
        let root = directory.join("recordings");
        std::fs::create_dir_all(&root).unwrap();
        let db_path = directory.join("test.sqlite3");
        Self {
            directory,
            root,
            db_path,
        }
    }
    fn pending(&self, db: &mut Connection) -> Recording {
        let record = Recording {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id: None,
            title: "Synthetic recording".into(),
            state: "starting".into(),
            elapsed_ms: 1000,
            recovered: false,
            complete: false,
            warning: None,
            created_at: String::new(),
            tracks: vec![Track {
                role: "microphone".into(),
                device_id: "test-input".into(),
                device_name: "Synthetic source".into(),
                filename: "microphone.wav.part".into(),
                frames: 0,
                sample_rate: wav::RATE,
                channels: 2,
            }],
        };
        register(db, &record).unwrap();
        std::fs::create_dir(self.root.join(&record.id)).unwrap();
        record
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this UUID-scoped synthetic fixture; never the application data directory.
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn finalization_is_idempotent_and_deleted_meetings_are_not_recreated() {
    let f = Fixture::new();
    let mut db = storage::open(&f.db_path).unwrap();
    let mut record = f.pending(&mut db);
    let directory = f.root.join(&record.id);
    let path = directory.join("microphone.wav");
    let mut writer = wav::Wav::create(&path).unwrap();
    writer.append_at(0, &[1, 0, 2, 0]).unwrap();
    record.tracks[0].frames = writer.finish(wav::RATE as u64).unwrap();
    record.tracks[0].filename = "microphone.wav".into();
    record.state = "saved".into();
    record.complete = true;
    write_marker(&directory, &record).unwrap();
    commit_result(&mut db, &mut record).unwrap();
    let id = record.meeting_id.unwrap();
    commit_result(&mut db, &mut record).unwrap();
    let data = storage::apply(&mut db, Request::Load).unwrap();
    assert_eq!(data.meetings.len(), 1);
    assert!(data.meetings[0].recorded);
    assert!(data.meetings[0].summary.is_empty());
    assert!(data.recordings[0].complete);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM processing_jobs WHERE recording_id=?1 AND state='queued'",
            [&record.id],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    storage::apply(&mut db, Request::DeleteMeeting { id }).unwrap();
    commit_result(&mut db, &mut record).unwrap();
    recover(&mut db, &f.root).unwrap();
    assert!(storage::apply(&mut db, Request::Load)
        .unwrap()
        .meetings
        .is_empty());
    assert!(list(&db).unwrap()[0].meeting_id.is_none());
    assert_eq!(
        db.query_row("SELECT count(*) FROM processing_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(path.exists());
}

#[test]
fn save_authorizes_only_new_recordings_and_duplicate_finalization_keeps_one_job() {
    let f = Fixture::new();
    let mut db = storage::open(&f.db_path).unwrap();
    let mut older = f.pending(&mut db);
    let engine = crate::processing::settings::Engine::Remote {
        remote: crate::processing::settings::Remote {
            base_url: "https://example.invalid/v1/".into(),
            model: "test-model".into(),
            response_format: "json".into(),
            language: "auto".into(),
        },
        revision: uuid::Uuid::new_v4().to_string(),
        credential_id: Some(uuid::Uuid::new_v4().to_string()),
    };
    db.execute(
        "INSERT INTO ai_settings(kind,config) VALUES ('transcription',?1)",
        [serde_json::to_string(&engine).unwrap()],
    )
    .unwrap();
    db.execute("UPDATE transcription_preferences SET automatic_revision=?1,after_recording_rowid=(SELECT max(rowid) FROM recordings) WHERE id=1",[engine.revision()]).unwrap();
    let mut fresh = f.pending(&mut db);
    // Match live WASAPI finalization with a nonfatal discontinuity warning.
    fresh.recovered = true;
    fresh.warning = Some("Windows reported an audio discontinuity.".into());
    let mut interrupted = f.pending(&mut db);
    interrupted.recovered = true;
    let interrupted_id = interrupted.id.clone();
    for record in [&mut older, &mut fresh, &mut interrupted] {
        let path = f.root.join(&record.id).join("microphone.wav");
        record.tracks[0].frames = wav::Wav::create(&path)
            .unwrap()
            .finish(wav::RATE as u64)
            .unwrap();
        record.tracks[0].filename = "microphone.wav".into();
        record.state = if record.id == interrupted_id {
            "failed"
        } else {
            "saved"
        }
        .into();
        record.complete = !record.recovered;
        commit_result(&mut db, record).unwrap();
        commit_result(&mut db, record).unwrap();
    }
    let (ready, approval): (bool, Option<String>) = db
        .query_row(
            "SELECT ready,upload_revision FROM processing_jobs WHERE recording_id=?1",
            [&fresh.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(ready);
    assert_eq!(approval.as_deref(), Some(engine.revision()));
    assert!(!db
        .query_row(
            "SELECT ready FROM processing_jobs WHERE recording_id=?1",
            [&interrupted.id],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    assert!(!db
        .query_row(
            "SELECT ready FROM processing_jobs WHERE recording_id=?1",
            [&older.id],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    assert_eq!(
        db.query_row("SELECT count(*) FROM processing_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert!(f.root.join(&fresh.id).join("microphone.wav").exists());
}

#[test]
fn interrupted_capture_recovers_original_payload_once_across_database_restarts() {
    let f = Fixture::new();
    let mut db = storage::open(&f.db_path).unwrap();
    let record = f.pending(&mut db);
    let directory = f.root.join(&record.id);
    let source = directory.join("microphone.wav.part");
    let mut bytes = wav::header(0).unwrap().to_vec();
    bytes.extend([1, 0, 2, 0, 3, 0, 4, 0, 5]);
    std::fs::write(&source, &bytes).unwrap();
    drop(db);
    let mut db = storage::open(&f.db_path).unwrap();
    recover(&mut db, &f.root).unwrap();
    recover(&mut db, &f.root).unwrap();
    let data = storage::apply(&mut db, Request::Load).unwrap();
    assert_eq!(data.meetings.len(), 1);
    assert!(data.recordings[0].recovered);
    assert!(!data.recordings[0].complete);
    assert_eq!(data.recordings[0].tracks[0].frames, 2);
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    assert_eq!(
        wav::inspect(&directory.join("microphone.recovered.wav")).unwrap(),
        2
    );
}

#[test]
fn durable_marker_closes_file_database_commit_gap_without_marking_success_incomplete() {
    let f = Fixture::new();
    let mut db = storage::open(&f.db_path).unwrap();
    let mut record = f.pending(&mut db);
    let directory = f.root.join(&record.id);
    record.tracks[0].filename = "microphone.wav".into();
    record.tracks[0].frames = wav::Wav::create(&directory.join("microphone.wav"))
        .unwrap()
        .finish(48)
        .unwrap();
    record.elapsed_ms = 1;
    record.complete = true;
    record.state = "saved".into();
    write_marker(&directory, &record).unwrap();
    drop(db);
    let mut db = storage::open(&f.db_path).unwrap();
    recover(&mut db, &f.root).unwrap();
    let saved = list(&db).unwrap().remove(0);
    assert!(saved.complete);
    assert!(!saved.recovered);
    assert!(saved.meeting_id.is_some());
}

#[test]
fn workspace_deletion_unassigns_recorded_meeting_and_preserves_tracks() {
    let f = Fixture::new();
    let mut db = storage::open(&f.db_path).unwrap();
    db.execute("INSERT INTO workspaces(name,description,color,symbol,overview,task,owner,due) VALUES ('Test','','sage','*','','','','')", []).unwrap();
    let workspace = db.last_insert_rowid();
    let mut record = f.pending(&mut db);
    let directory = f.root.join(&record.id);
    record.tracks[0].filename = "microphone.wav".into();
    let path = directory.join("microphone.wav");
    record.tracks[0].frames = wav::Wav::create(&path).unwrap().finish(48).unwrap();
    record.state = "saved".into();
    commit_result(&mut db, &mut record).unwrap();
    db.execute(
        "UPDATE meetings SET workspace_id=?1 WHERE id=?2",
        params![workspace, record.meeting_id],
    )
    .unwrap();
    let data = storage::apply(&mut db, Request::DeleteWorkspace { id: workspace }).unwrap();
    assert_eq!(data.meetings[0].project, 0);
    assert_eq!(data.recordings[0].meeting_id, record.meeting_id);
    assert!(path.exists());
}

#[test]
fn live_owner_blocks_another_recorder_and_invalid_controls_are_rejected() {
    let f = Fixture::new();
    let first = Recorder::new(f.root.clone(), f.db_path.clone()).unwrap();
    assert!(Recorder::new(f.root.clone(), f.db_path.clone()).is_err());
    assert!(first.control(Action::Resume).is_err());
    assert!(first.control(Action::Stop).is_err());
    drop(first);
    assert!(Recorder::new(f.root.clone(), f.db_path.clone()).is_ok());
}

#[test]
fn playback_rejects_traversal_foreign_origins_and_writes() {
    let f = Fixture::new();
    let request = tauri::http::Request::builder()
        .uri("http://recording.localhost/../microphone")
        .body(vec![])
        .unwrap();
    assert_eq!(response(&f.root, &f.db_path, request).status(), 404);
    let request = tauri::http::Request::builder()
        .uri("http://recording.localhost/x/system")
        .header("Origin", "https://example.com")
        .body(vec![])
        .unwrap();
    assert_eq!(response(&f.root, &f.db_path, request).status(), 403);
    let request = tauri::http::Request::builder()
        .method("POST")
        .uri("http://recording.localhost/x/system")
        .body(vec![])
        .unwrap();
    assert_eq!(response(&f.root, &f.db_path, request).status(), 405);
}

#[test]
fn unsupported_or_missing_audio_is_retained_as_failed_without_an_empty_meeting() {
    let f = Fixture::new();
    let mut db = storage::open(&f.db_path).unwrap();
    let record = f.pending(&mut db);
    recover(&mut db, &f.root).unwrap();
    let data = storage::apply(&mut db, Request::Load).unwrap();
    assert!(data.meetings.is_empty());
    assert_eq!(data.recordings[0].state, "failed");
    assert!(f.root.join(&record.id).exists());
}

#[test]
fn playback_serves_header_and_bounded_ranges_for_finalized_audio() {
    let f = Fixture::new();
    let mut db = storage::open(&f.db_path).unwrap();
    let mut record = f.pending(&mut db);
    let directory = f.root.join(&record.id);
    record.tracks[0].filename = "microphone.wav".into();
    record.tracks[0].frames = wav::Wav::create(&directory.join("microphone.wav"))
        .unwrap()
        .finish(800_000)
        .unwrap();
    record.elapsed_ms = 800_000 * 1000 / wav::RATE as u64;
    record.state = "saved".into();
    commit_result(&mut db, &mut record).unwrap();
    let uri = format!("http://recording.localhost/{}/microphone", record.id);
    let request = tauri::http::Request::builder()
        .uri(&uri)
        .header("Range", "bytes=0-")
        .body(vec![])
        .unwrap();
    let reply = response(&f.root, &f.db_path, request);
    assert_eq!(reply.status(), 206);
    assert_eq!(&reply.body()[..4], b"RIFF");
    assert_eq!(reply.body().len(), 2 * 1024 * 1024);
    assert_eq!(reply.headers()["Content-Range"], "bytes 0-2097151/3200044");
    let request = tauri::http::Request::builder()
        .method("HEAD")
        .uri(&uri)
        .body(vec![])
        .unwrap();
    let reply = response(&f.root, &f.db_path, request);
    assert_eq!(reply.status(), 200);
    assert!(reply.body().is_empty());
}

#[test]
fn recovery_restarts_an_interrupted_derived_copy_without_overwriting_original() {
    let f = Fixture::new();
    let source = f.root.join("test.wav.part");
    let output = f.root.join("test.recovered.wav");
    let mut original = wav::header(0).unwrap().to_vec();
    original.extend([1, 0, 2, 0, 3, 0, 4, 0]);
    std::fs::write(&source, &original).unwrap();
    std::fs::write(
        output.with_extension("wav.recovery.tmp"),
        b"interrupted scratch",
    )
    .unwrap();
    assert_eq!(wav::repair_copy(&source, &output).unwrap(), 2);
    assert_eq!(std::fs::read(source).unwrap(), original);
    assert_eq!(wav::inspect(&output).unwrap(), 2);
}

#[test]
fn unavailable_requested_loopback_fails_without_creating_a_microphone_recording() {
    let f = Fixture::new();
    let recorder = Recorder::new(f.root.clone(), f.db_path.clone()).unwrap();
    recorder
        .start(Selection {
            microphone: false,
            system: true,
            microphone_device: None,
            system_device: Some("miw-deliberately-nonexistent-output-endpoint".into()),
            title: "Unavailable source test".into(),
        })
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while recorder.active() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let status = recorder.status().unwrap();
    assert_eq!(status.state, service::State::Failed);
    assert!(status.error.is_some());
    assert!(status.meeting_id.is_none());
    let mut db = storage::open(&f.db_path).unwrap();
    let data = storage::apply(&mut db, Request::Load).unwrap();
    assert!(data.meetings.is_empty());
    assert!(data.recordings.is_empty());
}
