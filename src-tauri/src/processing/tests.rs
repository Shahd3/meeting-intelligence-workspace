use super::*;
use provider::{Audio, Provider, WhisperCli};
use std::path::Path;
struct Fixture {
    directory: PathBuf,
    db: Connection,
    recording: String,
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("miw-processing-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let db = storage::open(&directory.join("test.sqlite3")).unwrap();
        db.execute("INSERT INTO meetings(title,date,duration,summary,highlights,decision,actions,speakers) VALUES ('Queue fixture','','','','[]','','[]','[]')",[]).unwrap();
        let recording = uuid::Uuid::new_v4().to_string();
        db.execute("INSERT INTO recordings(id,meeting_id,title,state,finalized,complete) VALUES (?1,1,'Queue fixture','saved',1,1)",[&recording]).unwrap();
        Self {
            directory,
            db,
            recording,
        }
    }
}

fn enqueue_ready(db: &Connection, recording: &str) {
    enqueue(db, recording).unwrap();
    // Existing orchestration fixtures represent explicitly started work.
    db.execute(
        "UPDATE processing_jobs SET ready=1 WHERE recording_id=?1",
        [recording],
    )
    .unwrap();
}

fn segment(role: &str, text: &str, start: u64, end: u64) -> Segment {
    Segment {
        text: text.into(),
        start_ms: Some(start),
        end_ms: Some(end),
        sources: vec![Source {
            role: role.into(),
            start_ms: Some(start),
            end_ms: Some(end),
        }],
    }
}
#[test]
fn durable_enqueue_claim_restart_and_duplicate_prevention() {
    let mut f = Fixture::new();
    enqueue_ready(&f.db, &f.recording);
    enqueue_ready(&f.db, &f.recording);
    let (id, _, owner, attempt) = claim(&mut f.db).unwrap().unwrap();
    assert_eq!(attempt, 1);
    assert!(claim(&mut f.db).unwrap().is_none());
    checkpoint(
        &mut f.db,
        &id,
        &owner,
        "microphone",
        0,
        "digest",
        &[segment("microphone", "Hello", 0, 100)],
    )
    .unwrap();
    let reopened = storage::open(&f.directory.join("test.sqlite3")).unwrap();
    reconcile(&reopened).unwrap();
    let data = view(&reopened, None, Some(1)).unwrap();
    assert_eq!(data.jobs.len(), 1);
    assert_eq!(data.jobs[0].state, "retryable");
    assert_eq!(data.jobs[0].checkpoints, 1);
    assert!(owned(&reopened, &id, &owner).is_err());
    retry(&reopened, &id).unwrap();
    drop(reopened);
    drop(f.db);
    std::fs::remove_dir_all(f.directory).unwrap();
}
#[test]
fn result_and_completion_commit_atomically_and_survive_restart() {
    let mut f = Fixture::new();
    enqueue_ready(&f.db, &f.recording);
    let (id, _, owner, _) = claim(&mut f.db).unwrap().unwrap();
    complete(
        &mut f.db,
        &id,
        &owner,
        vec![segment("microphone", "Hello", 0, 100)],
        None,
    )
    .unwrap();
    assert!(complete(&mut f.db, &id, &owner, vec![], None).is_err());
    enqueue_ready(&f.db, &f.recording);
    let reopened = storage::open(&f.directory.join("test.sqlite3")).unwrap();
    let data = view(&reopened, None, Some(1)).unwrap();
    assert_eq!(data.jobs.len(), 1);
    assert_eq!(data.jobs[0].state, "completed");
    assert_eq!(data.jobs[0].segments.as_ref().unwrap()[0].text, "Hello");
    assert!(retry(&reopened, &id).is_err());
    reopened.execute("DELETE FROM workspaces", []).unwrap();
    assert_eq!(view(&reopened, None, Some(1)).unwrap().jobs.len(), 1);
    reopened
        .execute("DELETE FROM meetings WHERE id=1", [])
        .unwrap();
    assert!(view(&reopened, None, Some(1)).unwrap().jobs.is_empty());
    assert_eq!(
        reopened
            .query_row("SELECT count(*) FROM recordings", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(reopened);
    drop(f.db);
    std::fs::remove_dir_all(f.directory).unwrap();
}
#[test]
fn failed_missing_runtime_and_retry_budget_are_persisted() {
    let mut f = Fixture::new();
    enqueue_ready(&f.db, &f.recording);
    let (id, _, owner, _) = claim(&mut f.db).unwrap().unwrap();
    let worker = Pipeline::new(f.directory.join("test.sqlite3"), f.directory.join("audio"));
    let error = worker
        .process(&mut f.db, &id, &f.recording, &owner)
        .unwrap_err();
    assert_eq!(error.category, "configuration");
    assert!(!error.retryable);
    f.db.execute(
        "UPDATE processing_jobs SET attempts=5,state='failed',owner=NULL WHERE id=?1",
        [&id],
    )
    .unwrap();
    assert!(retry(&f.db, &id).is_err());
    assert!(claim(&mut f.db).unwrap().is_none());
    drop(f.db);
    std::fs::remove_dir_all(f.directory).unwrap();
}
#[test]
fn checkpoint_reuse_and_audio_silence_processing() {
    let f = Fixture::new();
    let audio = f.directory.join("audio");
    let dir = audio.join(&f.recording);
    std::fs::create_dir_all(&dir).unwrap();
    let writer = crate::recording::wav::Wav::create(&dir.join("microphone.wav")).unwrap();
    writer.finish(48000).unwrap();
    f.db.execute("INSERT INTO recording_tracks(recording_id,role,device_id,device_name,filename,frames) VALUES (?1,'microphone','','','microphone.wav',48000)",[&f.recording]).unwrap();
    let mut input = Audio::open(&dir.join("microphone.wav"), 48000).unwrap();
    let pcm = input.next_chunk().unwrap().unwrap();
    assert_eq!(pcm.len(), 16000);
    assert!(input.next_chunk().unwrap().is_none());
    let shutdown = AtomicBool::new(false);
    let config = Config {
        executable: String::new(),
        model: String::new(),
        language: "auto".into(),
        executable_sha256: String::new(),
        model_sha256: String::new(),
    };
    assert!(WhisperCli {
        config: &config,
        shutdown: &shutdown
    }
    .transcribe(&pcm, &dir, "microphone", 0)
    .unwrap()
    .is_empty());
    assert!(Audio::open(&dir.join("microphone.wav"), 48001).is_err());
    let mut corrupt = std::fs::read(dir.join("microphone.wav")).unwrap();
    corrupt[0] = 0;
    std::fs::write(dir.join("microphone.wav"), corrupt).unwrap();
    assert!(Audio::open(&dir.join("microphone.wav"), 48000).is_err());
    drop(f.db);
    std::fs::remove_dir_all(f.directory).unwrap();
}
#[test]
fn duplicate_echoes_retain_sources_but_distinct_overlaps_remain() {
    let merged = merge_echoes(vec![
        segment("microphone", "Hello there.", 100, 1000),
        segment("system", "hello there", 200, 1100),
        segment("system", "Another speaker", 250, 1200),
    ]);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].sources.len(), 2);
    assert_eq!(merged[0].sources[1].start_ms, Some(200));
    assert!(provider::validate(&[segment("system", "bad", 500, 100)], "system", 0, 1000).is_err());
}
#[test]
fn worker_lock_excludes_a_second_instance() {
    let f = Fixture::new();
    let a = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(f.directory.join("processing.lock"))
        .unwrap();
    a.try_lock_exclusive().unwrap();
    let b = OpenOptions::new()
        .read(true)
        .write(true)
        .open(f.directory.join("processing.lock"))
        .unwrap();
    assert!(b.try_lock_exclusive().is_err());
    drop(a);
    b.try_lock_exclusive().unwrap();
    drop(b);
    drop(f.db);
    std::fs::remove_dir_all(f.directory).unwrap();
}

#[test]
fn transient_retries_are_due_bounded_and_manual_retry_does_not_reset_attempts() {
    let mut f = Fixture::new();
    enqueue_ready(&f.db, &f.recording);
    let (id, _, owner, attempt) = claim(&mut f.db).unwrap().unwrap();
    record_failure(
        &f.db,
        &id,
        &owner,
        attempt,
        provider::Failure::provider("Temporary provider failure."),
    )
    .unwrap();
    let data = view(&f.db, None, None).unwrap();
    assert_eq!(data.jobs[0].state, "retryable");
    assert_eq!(data.jobs[0].attempts, 1);
    assert!(data.jobs[0].next_attempt_at.unwrap() > data.jobs[0].updated_at);
    assert!(claim(&mut f.db).unwrap().is_none());
    retry(&f.db, &id).unwrap();
    let (_, _, owner, attempt) = claim(&mut f.db).unwrap().unwrap();
    assert_eq!(attempt, 2);
    record_failure(
        &f.db,
        &id,
        &owner,
        attempt,
        provider::Failure::provider("Temporary provider failure."),
    )
    .unwrap();
    retry(&f.db, &id).unwrap();
    let (_, _, owner, attempt) = claim(&mut f.db).unwrap().unwrap();
    assert_eq!(attempt, 3);
    record_failure(
        &f.db,
        &id,
        &owner,
        attempt,
        provider::Failure::provider("Temporary provider failure."),
    )
    .unwrap();
    assert_eq!(view(&f.db, None, None).unwrap().jobs[0].state, "failed");
    assert!(claim(&mut f.db).unwrap().is_none());
    retry(&f.db, &id).unwrap();
    assert_eq!(claim(&mut f.db).unwrap().unwrap().3, 4);
    drop(f.db);
    std::fs::remove_dir_all(f.directory).unwrap();
}

#[test]
fn a_completion_write_failure_rolls_back_the_transcript_receipt() {
    let mut f = Fixture::new();
    enqueue_ready(&f.db, &f.recording);
    let (id, _, owner, _) = claim(&mut f.db).unwrap().unwrap();
    f.db.execute_batch("CREATE TRIGGER fail_completion BEFORE UPDATE OF state ON processing_jobs WHEN NEW.state='completed' BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    assert!(complete(
        &mut f.db,
        &id,
        &owner,
        vec![segment("microphone", "Hello", 0, 100)],
        None
    )
    .is_err());
    assert_eq!(
        f.db.query_row("SELECT count(*) FROM transcripts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(view(&f.db, None, None).unwrap().jobs[0].state, "processing");
    f.db.execute_batch("DROP TRIGGER fail_completion;").unwrap();
    complete(
        &mut f.db,
        &id,
        &owner,
        vec![segment("microphone", "Hello", 0, 100)],
        None,
    )
    .unwrap();
    drop(f.db);
    std::fs::remove_dir_all(f.directory).unwrap();
}
/// Explicit opt-in: public known-audio fixture and downloaded runtime stay outside Git.
#[test]
#[ignore = "requires MIW_TEST_CLI, MIW_TEST_MODEL, MIW_TEST_AUDIO canonical WAV"]
fn real_local_transcription_and_restart_checkpoint_reuse() {
    let mut f = Fixture::new();
    let root = f.directory.join("audio");
    let directory = root.join(&f.recording);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::copy(
        std::env::var("MIW_TEST_AUDIO").unwrap(),
        directory.join("microphone.wav"),
    )
    .unwrap();
    let frames = (std::fs::metadata(directory.join("microphone.wav"))
        .unwrap()
        .len()
        - 44)
        / 4;
    f.db.execute(
        "UPDATE recordings SET elapsed_ms=?1 WHERE id=?2",
        params![(frames * 1000 / 48000) as i64, &f.recording],
    )
    .unwrap();
    f.db.execute("INSERT INTO recording_tracks(recording_id,role,device_id,device_name,filename,frames) VALUES (?1,'microphone','','','microphone.wav',?2)",params![f.recording,frames as i64]).unwrap();
    std::fs::copy(
        directory.join("microphone.wav"),
        directory.join("system.wav"),
    )
    .unwrap();
    f.db.execute("INSERT INTO recording_tracks(recording_id,role,device_id,device_name,filename,frames) VALUES (?1,'system','','','system.wav',?2)",params![f.recording,frames as i64]).unwrap();
    let mut cfg = Config {
        executable: std::env::var("MIW_TEST_CLI").unwrap(),
        model: std::env::var("MIW_TEST_MODEL").unwrap(),
        language: "en".into(),
        executable_sha256: String::new(),
        model_sha256: String::new(),
    };
    cfg.validate_and_fingerprint().unwrap();
    set_config(&mut f.db, cfg).unwrap();
    enqueue_ready(&f.db, &f.recording);
    let (id, _, owner, _) = claim(&mut f.db).unwrap().unwrap();
    let worker = Pipeline::new(f.directory.join("test.sqlite3"), root);
    worker
        .process(&mut f.db, &id, &f.recording, &owner)
        .unwrap();
    let text = view(&f.db, None, Some(1)).unwrap().jobs[0]
        .segments
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| s.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("country"));
    assert!(text.contains("ask"));
    assert!(view(&f.db, None, Some(1)).unwrap().jobs[0]
        .segments
        .as_ref()
        .unwrap()
        .iter()
        .all(|s| s.sources.len() == 2));
    // Simulate loss before final result commit, keeping the real checkpoint.
    f.db.execute("DELETE FROM transcripts WHERE job_id=?1", [&id])
        .unwrap();
    f.db.execute(
        "UPDATE processing_jobs SET state='processing',owner='old' WHERE id=?1",
        [&id],
    )
    .unwrap();
    let mut reopened = storage::open(&f.directory.join("test.sqlite3")).unwrap();
    reconcile(&reopened).unwrap();
    retry(&reopened, &id).unwrap();
    let (_, _, owner, _) = claim(&mut reopened).unwrap().unwrap();
    worker
        .process(&mut reopened, &id, &f.recording, &owner)
        .unwrap();
    let data = view(&reopened, None, Some(1)).unwrap();
    assert_eq!(data.jobs[0].state, "completed");
    assert!(data.jobs[0].checkpoints > 0);
    assert!(Path::new(&directory.join("microphone.wav")).exists());
    drop(reopened);
    drop(f.db);
    std::fs::remove_dir_all(f.directory).unwrap();
}
