use super::{
    credentials::{Vault, WindowsVault},
    settings::{self, Engine, Remote, Save},
    *,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
};
use zeroize::Zeroizing;
#[derive(Default)]
struct MemoryVault {
    keys: RefCell<BTreeMap<String, String>>,
    fail_delete: std::cell::Cell<bool>,
}
impl Vault for MemoryVault {
    fn read(&self, id: &str) -> Result<Option<Zeroizing<String>>> {
        Ok(self.keys.borrow().get(id).cloned().map(Zeroizing::new))
    }
    fn write(&self, id: &str, key: &str) -> Result<()> {
        super::credentials::valid_key(key)?;
        self.keys.borrow_mut().insert(id.into(), key.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<()> {
        if self.fail_delete.get() {
            return Err("Test deletion unavailable.".into());
        }
        self.keys.borrow_mut().remove(id);
        Ok(())
    }
}
fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    for migration in [
        include_str!("../../migrations/0001.sql"),
        include_str!("../../migrations/0002.sql"),
        include_str!("../../migrations/0003.sql"),
        include_str!("../../migrations/0004.sql"),
        include_str!("../../migrations/0005.sql"),
        include_str!("../../migrations/0006.sql"),
        include_str!("../../migrations/0007.sql"),
        include_str!("../../migrations/0008.sql"),
    ] {
        db.execute_batch(migration).unwrap();
    }
    db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    db
}
fn remote(base: &str) -> Remote {
    Remote {
        base_url: base.into(),
        model: "test-model".into(),
        response_format: "verbose_json".into(),
        language: "en".into(),
    }
}
fn save_request(kind: &str, base: &str, key: &str) -> Save {
    Save {
        kind: kind.into(),
        mode: "remote".into(),
        local: None,
        remote: Some(remote(base)),
        key_action: "replace".into(),
        api_key: Some(key.into()),
        automatic_transcription: false,
        automatic_reviewed: false,
        automatic_generation: false,
        output_language: "en".into(),
        generation_format: "json_object".into(),
    }
}
fn id(engine: &Engine) -> String {
    match engine {
        Engine::Remote {
            credential_id: Some(id),
            ..
        } => id.clone(),
        _ => panic!("No credential reference."),
    }
}
fn fixture(db: &Connection) -> String {
    db.execute("INSERT INTO meetings(title,date,duration,summary,highlights,decision,actions,speakers) VALUES ('AI fixture','','','','[]','','[]','[]')",[]).unwrap();
    let recording = uuid::Uuid::new_v4().to_string();
    db.execute("INSERT INTO recordings(id,meeting_id,title,state,finalized) VALUES (?1,1,'AI fixture','saved',1)",[&recording]).unwrap();
    enqueue(db, &recording).unwrap();
    db.execute(
        "UPDATE processing_jobs SET ready=1 WHERE recording_id=?1",
        [&recording],
    )
    .unwrap();
    recording
}
#[test]
fn settings_replace_delete_and_separate_credentials_never_serialize_key_material() {
    let mut db = db();
    let vault = MemoryVault::default();
    settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://example.invalid/v1",
            "test-placeholder-one",
        ),
        &vault,
    )
    .unwrap();
    let first = settings::load(&db, "transcription").unwrap().unwrap();
    let first_id = id(&first);
    settings::save(
        &mut db,
        save_request(
            "summarization",
            "https://example.invalid/v1",
            "test-placeholder-summary",
        ),
        &vault,
    )
    .unwrap();
    let summary_id = id(&settings::load(&db, "summarization").unwrap().unwrap());
    assert_ne!(first_id, summary_id);
    settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://example.invalid/v1",
            "test-placeholder-two",
        ),
        &vault,
    )
    .unwrap();
    assert!(vault.read(&first_id).unwrap().is_none());
    assert!(vault.read(&summary_id).unwrap().is_some());
    let current = settings::load(&db, "transcription").unwrap().unwrap();
    let json = serde_json::to_string(&current).unwrap();
    assert!(!json.contains("placeholder"));
    assert!(!json.contains("api_key"));
    let mut deletion = save_request("transcription", "https://example.invalid/v1", "");
    deletion.key_action = "delete".into();
    deletion.api_key = None;
    settings::save(&mut db, deletion, &vault).unwrap();
    assert!(vault.read(&id(&current)).unwrap().is_none());
    assert!(vault.read(&summary_id).unwrap().is_some());
}
#[test]
fn endpoint_change_requires_explicit_key_replacement_and_cleanup_is_durable() {
    let mut db = db();
    let vault = MemoryVault::default();
    settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://example.invalid/v1",
            "test-placeholder-one",
        ),
        &vault,
    )
    .unwrap();
    let old = settings::load(&db, "transcription").unwrap().unwrap();
    let mut changed = save_request("transcription", "https://another.invalid/v1", "");
    changed.key_action = "keep".into();
    changed.api_key = None;
    assert!(settings::save(&mut db, changed, &vault).is_err());
    vault.fail_delete.set(true);
    assert!(settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://another.invalid/v1",
            "test-placeholder-two"
        ),
        &vault
    )
    .is_err());
    assert_eq!(
        db.query_row("SELECT count(*) FROM ai_credential_cleanup", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(vault.read(&id(&old)).unwrap().is_some());
    vault.fail_delete.set(false);
    settings::cleanup(&db, &vault).unwrap();
    assert!(vault.read(&id(&old)).unwrap().is_none());
}
#[test]
fn failed_settings_write_removes_new_secret_without_changing_existing_settings() {
    let mut db = db();
    let vault = MemoryVault::default();
    db.execute_batch("CREATE TRIGGER fail_settings BEFORE INSERT ON ai_settings BEGIN SELECT RAISE(ABORT,'test failure'); END").unwrap();
    assert!(settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://example.invalid/v1",
            "test-placeholder-one"
        ),
        &vault
    )
    .is_err());
    assert!(vault.keys.borrow().is_empty());
    assert!(settings::load(&db, "transcription").unwrap().is_none());
}
#[test]
fn queued_jobs_require_revision_specific_consent_and_changed_jobs_cannot_mix_settings() {
    let mut db = db();
    let recording = fixture(&db);
    let vault = MemoryVault::default();
    settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://example.invalid/v1",
            "test-placeholder-one",
        ),
        &vault,
    )
    .unwrap();
    let original = settings::load(&db, "transcription").unwrap().unwrap();
    let (job, _, owner, _) = claim(&mut db).unwrap().unwrap();
    let worker = Pipeline::new(PathBuf::new(), PathBuf::new());
    let failure = worker
        .process(&mut db, &job, &recording, &owner)
        .unwrap_err();
    assert_eq!(failure.category, "consent");
    record_failure(&db, &job, &owner, 1, failure).unwrap();
    assert!(settings::rebind(&mut db, &job, original.revision(), false).is_err());
    settings::rebind(&mut db, &job, original.revision(), true).unwrap();
    let (_, _, owner, _) = claim(&mut db).unwrap().unwrap();
    checkpoint(
        &mut db,
        &job,
        &owner,
        "microphone",
        0,
        "old-checkpoint",
        &[],
    )
    .unwrap();
    settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://another.invalid/v1",
            "test-placeholder-two",
        ),
        &vault,
    )
    .unwrap();
    assert!(settings::ensure_remote_current(&db, &original).is_err());
    assert!(settings::rebind(&mut db, &job, original.revision(), true).is_err());
    record_failure(
        &db,
        &job,
        &owner,
        2,
        provider::Failure::configuration("Changed settings."),
    )
    .unwrap();
    let current = settings::load(&db, "transcription").unwrap().unwrap();
    settings::rebind(&mut db, &job, current.revision(), true).unwrap();
    assert_eq!(view(&db, None, None).unwrap().jobs[0].checkpoints, 0);
    assert_eq!(
        db.query_row("SELECT count(*) FROM recordings", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn local_config_legacy_checkpoints_and_remote_switch_are_compatible() {
    let mut db = db();
    let local = Config {
        executable: "local-fixture".into(),
        model: "model-fixture".into(),
        language: "en".into(),
        executable_sha256: "exe-digest".into(),
        model_sha256: "model-digest".into(),
    };
    let json = serde_json::to_string(&local).unwrap();
    db.execute(
        "UPDATE transcription_settings SET config=?1 WHERE id=1",
        [&json],
    )
    .unwrap();
    let engine = settings::load(&db, "transcription").unwrap().unwrap();
    assert_eq!(
        settings::fingerprint(&engine, &[1, 2]),
        provider::fingerprint(&local, &[1, 2])
    );
    assert!(matches!(
        settings::parse_job(&json).unwrap(),
        Engine::Local { .. }
    ));
    let vault = MemoryVault::default();
    settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://example.invalid/v1",
            "test-placeholder-one",
        ),
        &vault,
    )
    .unwrap();
    assert!(matches!(
        settings::load(&db, "transcription").unwrap().unwrap(),
        Engine::Remote { .. }
    ));
    assert!(matches!(
        settings::parse_job(&json).unwrap(),
        Engine::Local { .. }
    ));
}
#[test]
fn remote_timestamp_absence_is_preserved_and_invalid_formats_rejected() {
    let segments =
        remote::parse(serde_json::json!({"text":"Hello"}), "system", 60000, 1000).unwrap();
    assert!(segments[0].start_ms.is_none());
    assert!(segments[0].sources[0].end_ms.is_none());
    let timed = remote::parse(
        serde_json::json!({"text":"Hello","segments":[{"text":"Hello","start":0.1,"end":0.8}]}),
        "system",
        60000,
        1000,
    )
    .unwrap();
    assert_eq!(timed[0].start_ms, Some(60100));
    for value in [
        serde_json::json!({"text":1}),
        serde_json::json!({"text":"bad","segments":"wrong"}),
        serde_json::json!({"text":"bad","segments":[{"text":"bad","start":-1,"end":1}]}),
    ] {
        assert!(remote::parse(value, "microphone", 0, 1000).is_err());
    }
    let auth = remote::status_failure(401, None);
    assert_eq!(auth.category, "authentication");
    assert!(!auth.retryable);
    let rate = remote::status_failure(429, Some(60));
    assert!(rate.retryable);
    assert_eq!(rate.retry_after_secs, Some(60));
    assert!(!remote::status_failure(302, None).retryable);
}
#[test]
fn base_urls_reject_insecure_embedded_credentials_and_redirect_headers_are_not_followed() {
    for base in [
        "http://example.invalid/v1",
        "https://key@example.invalid/v1",
        "https://example.invalid/v1?api_key=hidden",
        "file:///tmp/model",
    ] {
        assert!(settings::base_url(base).is_err());
    }
    assert!(settings::base_url("http://127.0.0.1:1234/v1").is_ok());
    assert!(settings::base_url("https://example.invalid/v1")
        .unwrap()
        .to_string()
        .ends_with('/'));
}
fn server(body: &str, status: &str) -> (String, std::thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1/", listener.local_addr().unwrap());
    let body = body.to_string();
    let status = status.to_string();
    let thread = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut request = vec![];
        let mut buffer = [0; 8192];
        loop {
            let n = socket.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..n]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..end]);
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let reply=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nLocation: http://127.0.0.1:1/never-forward\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
        socket.write_all(reply.as_bytes()).unwrap();
        request
    });
    (address, thread)
}
#[test]
fn multipart_request_contains_model_audio_language_and_timestamp_contract() {
    let (base, thread) = server(
        "{\"text\":\"Hello\",\"segments\":[{\"text\":\"Hello\",\"start\":0,\"end\":0.5}]}",
        "200 OK",
    );
    let remote = remote(&base);
    let reply = remote::client()
        .unwrap()
        .post(format!("{base}audio/transcriptions"))
        .bearer_auth("test-placeholder-only")
        .multipart(remote::form(&remote, &vec![1; 16000]).unwrap())
        .send()
        .unwrap();
    assert!(reply.status().is_success());
    let request = thread.join().unwrap();
    let text = String::from_utf8_lossy(&request);
    for expected in [
        "POST /v1/audio/transcriptions",
        "multipart/form-data",
        "name=\"model\"",
        "test-model",
        "name=\"response_format\"",
        "verbose_json",
        "name=\"timestamp_granularities[]\"",
        "segment",
        "filename=\"audio.wav\"",
        "audio/wav",
        "RIFF",
        "WAVE",
    ] {
        assert!(text.contains(expected), "Missing multipart field.");
    }
}
#[cfg(windows)]
#[test]
fn windows_credential_persistence_replacement_and_deletion() {
    let id = uuid::Uuid::new_v4().to_string();
    let vault = WindowsVault;
    vault.write(&id, "test-placeholder-one").unwrap();
    assert!(WindowsVault.read(&id).unwrap().is_some());
    vault.write(&id, "test-placeholder-two").unwrap();
    let key = vault.read(&id).unwrap().unwrap();
    assert!(key.as_str() == "test-placeholder-two");
    drop(key);
    vault.delete(&id).unwrap();
    assert!(vault.read(&id).unwrap().is_none());
}

#[test]
fn automatic_preference_requires_review_and_never_upgrades_older_or_manual_jobs() {
    let mut db = db();
    let vault = MemoryVault::default();
    let old = fixture(&db);
    db.execute("UPDATE processing_jobs SET ready=0,stage='manual'", [])
        .unwrap();
    let mut request = save_request(
        "transcription",
        "https://example.invalid/v1",
        "test-placeholder-one",
    );
    request.automatic_transcription = true;
    assert!(settings::save(&mut db, request, &vault).is_err());
    assert!(vault.keys.borrow().is_empty());
    let mut request = save_request(
        "transcription",
        "https://example.invalid/v1",
        "test-placeholder-one",
    );
    request.automatic_transcription = true;
    request.automatic_reviewed = true;
    settings::save(&mut db, request, &vault).unwrap();
    let engine = settings::load(&db, "transcription").unwrap().unwrap();
    assert!(settings::automatic_enabled(&db, &engine).unwrap());
    enqueue_finalized(&db, &old, true).unwrap();
    reconcile(&db).unwrap();
    assert!(claim(&mut db).unwrap().is_none());
    let fresh = uuid::Uuid::new_v4().to_string();
    db.execute("INSERT INTO recordings(id,meeting_id,title,state,finalized) VALUES (?1,1,'New capture','saved',1)",[&fresh]).unwrap();
    enqueue_finalized(&db, &fresh, true).unwrap();
    enqueue_finalized(&db, &fresh, true).unwrap();
    let (job, recording, owner, _) = claim(&mut db).unwrap().unwrap();
    assert_eq!(recording, fresh);
    record_failure(&db, &job, &owner, 1, remote::status_failure(429, Some(60))).unwrap();
    let status = view(&db, None, None).unwrap();
    let retrying = status.jobs.iter().find(|j| j.id == job).unwrap();
    assert_eq!(retrying.state, "retryable");
    assert!(retrying.ready);
    assert!(retrying.next_attempt_at.unwrap() - retrying.updated_at >= 60);
    let mut changed = save_request(
        "transcription",
        "https://another.invalid/v1",
        "test-placeholder-two",
    );
    changed.automatic_transcription = true;
    assert!(settings::save(&mut db, changed, &vault).is_err());
    assert!(settings::automatic_enabled(&db, &engine).unwrap());
    let mut changed = save_request(
        "transcription",
        "https://another.invalid/v1",
        "test-placeholder-two",
    );
    changed.automatic_transcription = true;
    changed.automatic_reviewed = true;
    settings::save(&mut db, changed, &vault).unwrap();
    let current = settings::load(&db, "transcription").unwrap().unwrap();
    assert!(settings::automatic_enabled(&db, &current).unwrap());
    assert!(settings::ensure_remote_current(&db, &engine).is_err());
    assert!(!db
        .query_row(
            "SELECT ready FROM processing_jobs WHERE recording_id=?1",
            [&old],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
}

#[test]
fn manual_transcription_and_status_reads_do_not_schedule_duplicate_work() {
    let mut db = db();
    let vault = MemoryVault::default();
    settings::save(
        &mut db,
        save_request(
            "transcription",
            "https://example.invalid/v1",
            "test-placeholder",
        ),
        &vault,
    )
    .unwrap();
    let recording = fixture(&db);
    db.execute("UPDATE processing_jobs SET ready=0,stage='manual'", [])
        .unwrap();
    let engine = settings::load(&db, "transcription").unwrap().unwrap();
    let job = db
        .query_row(
            "SELECT id FROM processing_jobs WHERE recording_id=?1",
            [&recording],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    for _ in 0..5 {
        view(&db, None, Some(1)).unwrap();
        assert!(claim(&mut db).unwrap().is_none());
    }
    assert!(settings::rebind(&mut db, &job, engine.revision(), false).is_err());
    settings::rebind(&mut db, &job, engine.revision(), true).unwrap();
    assert!(settings::rebind(&mut db, &job, engine.revision(), true).is_err());
    let (_, _, owner, _) = claim(&mut db).unwrap().unwrap();
    for _ in 0..5 {
        view(&db, None, Some(1)).unwrap();
        enqueue(&db, &recording).unwrap();
        assert!(claim(&mut db).unwrap().is_none());
    }
    assert!(owned(&db, &job, &owner).is_ok());
    assert_eq!(
        db.query_row("SELECT count(*) FROM processing_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn disabling_auto_and_saving_summary_do_not_change_already_authorized_work() {
    let mut db = db();
    let vault = MemoryVault::default();
    let mut request = save_request(
        "transcription",
        "https://example.invalid/v1",
        "test-placeholder",
    );
    request.automatic_transcription = true;
    request.automatic_reviewed = true;
    settings::save(&mut db, request, &vault).unwrap();
    let engine = settings::load(&db, "transcription").unwrap().unwrap();
    // An unchanged save must retain revision and the eligibility watermark.
    let mut unchanged = save_request("transcription", "https://example.invalid/v1", "");
    unchanged.api_key = None;
    unchanged.key_action = "keep".into();
    unchanged.automatic_transcription = true;
    settings::save(&mut db, unchanged, &vault).unwrap();
    assert_eq!(
        settings::load(&db, "transcription")
            .unwrap()
            .unwrap()
            .revision(),
        engine.revision()
    );
    settings::save(
        &mut db,
        save_request(
            "summarization",
            "https://summary.invalid/v1",
            "test-summary-key",
        ),
        &vault,
    )
    .unwrap();
    assert!(settings::automatic_enabled(&db, &engine).unwrap());
    db.execute("INSERT INTO meetings(title,date,duration,summary,highlights,decision,actions,speakers) VALUES ('Auto fixture','','','','[]','','[]','[]')",[]).unwrap();
    let approved = uuid::Uuid::new_v4().to_string();
    db.execute("INSERT INTO recordings(id,meeting_id,title,state,finalized) VALUES (?1,1,'Approved','saved',1)",[&approved]).unwrap();
    enqueue_finalized(&db, &approved, true).unwrap();
    let mut disabled = save_request("transcription", "https://example.invalid/v1", "");
    disabled.api_key = None;
    disabled.key_action = "keep".into();
    settings::save(&mut db, disabled, &vault).unwrap();
    assert!(!settings::automatic_enabled(&db, &engine).unwrap());
    assert!(settings::ensure_remote_current(&db, &engine).is_ok());
    let manual = uuid::Uuid::new_v4().to_string();
    db.execute("INSERT INTO recordings(id,meeting_id,title,state,finalized) VALUES (?1,1,'Manual','saved',1)",[&manual]).unwrap();
    enqueue_finalized(&db, &manual, true).unwrap();
    assert!(!db
        .query_row(
            "SELECT ready FROM processing_jobs WHERE recording_id=?1",
            [&manual],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    assert_eq!(claim(&mut db).unwrap().unwrap().1, approved);
    assert!(claim(&mut db).unwrap().is_none());
}

#[cfg(windows)]
#[test]
fn automatic_background_transcription_completes_without_details_and_preference_survives_restart() {
    let directory = std::env::temp_dir().join(format!("miw-auto-worker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("test.sqlite3");
    let root = directory.join("recordings");
    let mut db = storage::open(&path).unwrap();
    let (base, server) = server(
        "{\"text\":\"Hello\",\"segments\":[{\"text\":\"Hello\",\"start\":0,\"end\":1}]}",
        "200 OK",
    );
    let mut request = save_request("transcription", &base, "test-placeholder-auto-only");
    request.automatic_transcription = true;
    request.automatic_reviewed = true;
    settings::save(&mut db, request, &WindowsVault).unwrap();
    let engine = settings::load(&db, "transcription").unwrap().unwrap();
    let _key_cleanup = DummyCredential(id(&engine));
    db.execute("INSERT INTO meetings(title,date,duration,summary,highlights,decision,actions,speakers) VALUES ('Auto fixture','','','','[]','','[]','[]')",[]).unwrap();
    let recording = uuid::Uuid::new_v4().to_string();
    let audio_directory = root.join(&recording);
    std::fs::create_dir_all(&audio_directory).unwrap();
    let mut writer =
        crate::recording::wav::Wav::create(&audio_directory.join("microphone.wav")).unwrap();
    writer.append_at(0, &[1u8, 0, 1, 0].repeat(48000)).unwrap();
    writer.finish(48000).unwrap();
    let tx = db.transaction().unwrap();
    tx.execute("INSERT INTO recordings(id,meeting_id,title,state,finalized,complete,elapsed_ms) VALUES (?1,1,'Auto fixture','saved',1,1,1000)",[&recording]).unwrap();
    tx.execute("INSERT INTO recording_tracks(recording_id,role,device_id,device_name,filename,frames) VALUES (?1,'microphone','','','microphone.wav',48000)",[&recording]).unwrap();
    enqueue_finalized(&tx, &recording, true).unwrap();
    tx.commit().unwrap();
    drop(db);
    let mut reopened = storage::open(&path).unwrap();
    assert!(settings::automatic_enabled(&reopened, &engine).unwrap());
    let worker = Pipeline::new(path.clone(), root);
    worker.start();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let complete = loop {
        let state: String = reopened
            .query_row(
                "SELECT state FROM processing_jobs WHERE recording_id=?1",
                [&recording],
                |r| r.get(0),
            )
            .unwrap();
        if state == "completed" {
            break true;
        }
        if std::time::Instant::now() > deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    worker.shutdown.store(true, Ordering::Release);
    std::thread::sleep(Duration::from_millis(700));
    assert!(
        complete,
        "Backend did not finish automatic transcription: {}",
        serde_json::to_string(&worker.status(None).unwrap()).unwrap()
    );
    let bytes = server.join().unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("POST /v1/audio/transcriptions"));
    enqueue(&reopened, &recording).unwrap();
    reconcile(&reopened).unwrap();
    assert!(claim(&mut reopened).unwrap().is_none());
    assert_eq!(
        reopened
            .query_row("SELECT count(*) FROM transcripts", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .query_row("SELECT attempts FROM processing_jobs", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(audio_directory.join("microphone.wav").exists());
    drop(worker);
    drop(reopened);
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(windows)]
struct DummyCredential(String);
#[cfg(windows)]
impl DummyCredential {
    fn new() -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        WindowsVault
            .write(&id, "test-placeholder-http-only")
            .unwrap();
        Self(id)
    }
}
#[cfg(windows)]
impl Drop for DummyCredential {
    fn drop(&mut self) {
        let _ = WindowsVault.delete(&self.0);
    }
}

#[cfg(windows)]
#[test]
fn real_adapter_handles_mock_auth_rate_limits_redirects_and_unsupported_bodies() {
    use provider::Provider;
    let credential = DummyCredential::new();
    for (status, category, retryable) in [
        ("401 Unauthorized", "authentication", false),
        ("403 Forbidden", "authentication", false),
        ("429 Too Many Requests", "rate_limit", true),
        ("503 Unavailable", "provider", true),
        ("302 Found", "configuration", false),
        ("400 Bad Request", "unsupported", false),
        ("200 OK", "unsupported", false),
    ] {
        let (base, thread) = server("Provider echo: test-placeholder-http-only", status);
        let remote = remote(&base);
        let result = remote::RemoteProvider {
            settings: &remote,
            credential_id: &credential.0,
        }
        .transcribe(&vec![1; 16000], std::path::Path::new("."), "microphone", 0);
        let error = result.unwrap_err();
        assert_eq!(error.category, category);
        assert_eq!(error.retryable, retryable);
        assert!(!error.message.contains("placeholder"));
        let request = thread.join().unwrap();
        assert!(String::from_utf8_lossy(&request).contains("POST /v1/audio/transcriptions"));
    }
    let (base, thread) = server("{\"text\":\"test-placeholder-http-only\"}", "200 OK");
    let remote = remote(&base);
    let error = remote::RemoteProvider {
        settings: &remote,
        credential_id: &credential.0,
    }
    .transcribe(&vec![1; 16000], std::path::Path::new("."), "microphone", 0)
    .unwrap_err();
    assert_eq!(error.category, "unsupported");
    assert!(!error.message.contains("placeholder"));
    thread.join().unwrap();
}
#[cfg(windows)]
#[test]
fn actual_remote_adapter_preserves_sources_and_summary_test_only_checks_model_access() {
    use provider::Provider;
    let credential = DummyCredential::new();
    let (base, thread) = server(
        "{\"text\":\"Hello\",\"segments\":[{\"text\":\"Hello\",\"start\":0.1,\"end\":0.8}]}",
        "200 OK",
    );
    let remote = remote(&base);
    let segments = remote::RemoteProvider {
        settings: &remote,
        credential_id: &credential.0,
    }
    .transcribe(&vec![1; 16000], std::path::Path::new("."), "system", 60000)
    .unwrap();
    assert_eq!(segments[0].start_ms, Some(60100));
    assert_eq!(segments[0].sources[0].role, "system");
    thread.join().unwrap();
    let (base, thread) = server("{\"data\":[{\"id\":\"test-model\"}]}", "200 OK");
    let engine = Engine::Remote {
        remote: super::ai_tests::remote(&base),
        revision: uuid::Uuid::new_v4().to_string(),
        credential_id: Some(credential.0.clone()),
    };
    assert!(remote::connection_test(&engine, "summarization")
        .unwrap()
        .contains("Chat completions compatibility has not been tested"));
    let request = thread.join().unwrap();
    assert!(String::from_utf8_lossy(&request).contains("GET /v1/models"));
    assert!(!String::from_utf8_lossy(&request).contains("chat/completions"));
}
