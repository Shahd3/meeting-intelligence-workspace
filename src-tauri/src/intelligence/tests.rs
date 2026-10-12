use super::worker::*;
use super::*;
use crate::{
    processing::{
        self,
        credentials::{Vault, WindowsVault},
        settings::{Engine, Remote, Save},
    },
    storage,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
struct Fixture {
    directory: std::path::PathBuf,
    db: Connection,
    path: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("miw-intelligence-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("test.sqlite3");
        let db = storage::open(&path).unwrap();
        Self {
            directory,
            path,
            db,
        }
    }
    fn provider(&self, base: &str) -> Config {
        let engine = Engine::Remote {
            remote: Remote {
                base_url: base.into(),
                model: "synthetic-chat".into(),
                response_format: "json".into(),
                language: "auto".into(),
            },
            revision: uuid::Uuid::new_v4().to_string(),
            credential_id: Some(uuid::Uuid::new_v4().to_string()),
        };
        self.db.execute("INSERT INTO ai_settings(kind,config) VALUES ('summarization',?1) ON CONFLICT(kind) DO UPDATE SET config=excluded.config",[serde_json::to_string(&engine).unwrap()]).unwrap();
        Config {
            engine,
            language: "en".into(),
            format: "json_object".into(),
        }
    }
    fn transcript(&mut self) -> String {
        self.db.execute("INSERT INTO meetings(title,date,duration,summary,highlights,decision,actions,speakers) VALUES ('User title','','','','[]','','[]','[]')",[]).unwrap();
        let m = self.db.last_insert_rowid();
        let r = uuid::Uuid::new_v4().to_string();
        let id = uuid::Uuid::new_v4().to_string();
        self.db.execute("INSERT INTO recordings(id,meeting_id,title,state,finalized) VALUES (?1,?2,'User title','saved',1)",params![r,m]).unwrap();
        self.db.execute("INSERT INTO processing_jobs(id,recording_id,meeting_id,state,owner,ready) VALUES (?1,?2,?3,'processing','test-owner',1)",params![id,r,m]).unwrap();
        processing::complete(
            &mut self.db,
            &id,
            "test-owner",
            vec![processing::Segment {
                text: "We agreed to test the application.".into(),
                start_ms: None,
                end_ms: None,
                sources: vec![],
            }],
            None,
        )
        .unwrap();
        id
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn point(id: &str) -> contract::Point {
    contract::Point {
        text: "Test the application".into(),
        evidence: vec![contract::Evidence {
            segment_id: format!("{id}:0"),
            quote: "We agreed to test the application.".into(),
        }],
    }
}
fn notes(id: &str) -> Notes {
    let mut n = empty();
    n.executive_summary = point(id);
    n.suggested_title = point(id);
    n.action_items.push(contract::Action {
        description: point(id),
        owner: None,
        owner_evidence: vec![],
        deadline: None,
        deadline_evidence: vec![],
    });
    n
}
#[test]
fn structured_contract_rejects_unknown_evidence_and_invented_owner() {
    let mut f = Fixture::new();
    let id = f.transcript();
    let src = source(&f.db, &id).unwrap();
    let mut n = notes(&id);
    contract::validate(&n, &src).unwrap();
    assert!(n.attendees.is_empty());
    assert!(n.action_items[0].deadline.is_none());
    n.action_items[0].owner = Some("Invented owner".into());
    assert!(contract::validate(&n, &src).is_err());
    n.action_items[0].owner = None;
    n.executive_summary.evidence[0].segment_id = "missing".into();
    assert!(contract::validate(&n, &src).is_err());
    n.executive_summary.evidence[0].segment_id = format!("{id}:0");
    n.executive_summary.evidence[0].quote = "Not in source".into();
    assert!(contract::validate(&n, &src).is_err());
    assert!(serde_json::from_value::<Notes>(serde_json::json!({"schema_version":1})).is_err());
}
#[test]
fn automatic_generation_is_atomic_unique_and_excludes_old_sources() {
    let mut f = Fixture::new();
    let cfg = f.provider("https://example.invalid/v1/");
    let old = f.transcript();
    f.db.execute("UPDATE intelligence_preferences SET automatic_revision=?1,after_recording_rowid=(SELECT max(rowid) FROM recordings) WHERE id=1",[cfg.engine.revision()]).unwrap();
    let fresh = f.transcript();
    register_completed(&f.db, &fresh).unwrap();
    assert_eq!(
        f.db.query_row("SELECT count(*) FROM intelligence_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        f.db.query_row("SELECT transcript_id FROM intelligence_jobs", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        fresh
    );
    register_completed(&f.db, &old).unwrap();
    assert_eq!(
        f.db.query_row("SELECT count(*) FROM transcripts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    let mut db = storage::open(&f.path).unwrap();
    assert!(claim(&mut db).unwrap().is_some());
    assert!(claim(&mut db).unwrap().is_none());
}
#[test]
fn recovery_failure_retry_and_regeneration_preserve_transcript_and_history() {
    let mut f = Fixture::new();
    let cfg = f.provider("https://example.invalid/v1/");
    let id = f.transcript();
    let jobid = enqueue(&f.db, &id, &cfg, "manual-fixture").unwrap();
    let job = claim(&mut f.db).unwrap().unwrap();
    fail(
        &f.db,
        &job,
        processing::provider::Failure::provider("Synthetic temporary error"),
    )
    .unwrap();
    assert!(claim(&mut f.db).unwrap().is_none());
    f.db.execute("UPDATE intelligence_jobs SET next_attempt_at=0", [])
        .unwrap();
    let interrupted = claim(&mut f.db).unwrap().unwrap();
    reconcile(&f.db).unwrap();
    assert!(finish(
        &mut f.db,
        &interrupted,
        &notes(&id),
        &Coverage {
            segment_count: 1,
            analyzed_segments: 1,
            chunk_count: 1,
            source_warning: None
        }
    )
    .is_err());
    f.db.execute("UPDATE intelligence_jobs SET next_attempt_at=0", [])
        .unwrap();
    let current = claim(&mut f.db).unwrap().unwrap();
    finish(
        &mut f.db,
        &current,
        &notes(&id),
        &Coverage {
            segment_count: 1,
            analyzed_segments: 1,
            chunk_count: 1,
            source_warning: None,
        },
    )
    .unwrap();
    let meeting: i64 =
        f.db.query_row(
            "SELECT meeting_id FROM transcripts WHERE job_id=?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    let v = view(&f.db, Some(meeting), None).unwrap();
    let result = v.result.unwrap();
    assert_eq!(result.id, jobid);
    let action = &result.actions[0];
    mutate(
        &mut f.db,
        Request::CompleteAction {
            id: action.id.clone(),
            done: true,
        },
    )
    .unwrap();
    mutate(
        &mut f.db,
        Request::Attendees {
            meeting_id: meeting,
            names: Some(vec!["User confirmed attendee".into()]),
        },
    )
    .unwrap();
    mutate(
        &mut f.db,
        Request::Generate {
            transcript_id: id.clone(),
            revision: cfg.engine.revision().into(),
            authorized: true,
            regenerate: true,
        },
    )
    .unwrap();
    let before = view(&f.db, Some(meeting), None).unwrap();
    assert!(before.result.unwrap().actions[0].done);
    let next = claim(&mut f.db).unwrap().unwrap();
    finish(
        &mut f.db,
        &next,
        &notes(&id),
        &Coverage {
            segment_count: 1,
            analyzed_segments: 1,
            chunk_count: 1,
            source_warning: None,
        },
    )
    .unwrap();
    let reopened = storage::open(&f.path).unwrap();
    let after = view(&reopened, Some(meeting), None).unwrap();
    assert!(!after.result.unwrap().actions[0].done);
    assert_eq!(
        after.attendees_override.unwrap(),
        vec!["User confirmed attendee"]
    );
    assert!(reopened
        .query_row(
            "SELECT done FROM intelligence_actions WHERE id=?1",
            [&action.id],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    assert_eq!(
        reopened
            .query_row("SELECT count(*) FROM transcripts", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .query_row("SELECT title FROM meetings WHERE id=?1", [meeting], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        "User title"
    );
}
#[test]
fn long_transcript_planning_covers_late_unicode_and_rejects_oversize() {
    let text = "مرحبا ".repeat(5000) + " LATE DISTINCT WORKSTREAM";
    let source = vec![contract::EvidenceSegment {
        id: "test:0".into(),
        segment: processing::Segment {
            text: text.clone(),
            start_ms: None,
            end_ms: None,
            sources: vec![],
        },
    }];
    let chunks = plan(&source).unwrap();
    assert!(chunks.len() > 1);
    let recovered: String = chunks
        .iter()
        .flatten()
        .map(|s| s.segment.text.as_str())
        .collect();
    assert_eq!(recovered, text);
    let mut huge = source;
    huge[0].segment.text = "x".repeat(330000);
    assert!(plan(&huge).is_err());
}
#[test]
fn completion_failure_rolls_back_and_stale_result_cannot_replace_newer() {
    let mut f = Fixture::new();
    let cfg = f.provider("https://example.invalid/v1/");
    let id = f.transcript();
    enqueue(&f.db, &id, &cfg, "original").unwrap();
    let old = claim(&mut f.db).unwrap().unwrap();
    fail(
        &f.db,
        &old,
        processing::provider::Failure::input("Synthetic failure"),
    )
    .unwrap();
    enqueue(&f.db, &id, &cfg, "newer").unwrap();
    let new = claim(&mut f.db).unwrap().unwrap();
    let coverage = Coverage {
        segment_count: 1,
        analyzed_segments: 1,
        chunk_count: 1,
        source_warning: None,
    };
    f.db.execute_batch("CREATE TRIGGER block_intelligence BEFORE INSERT ON intelligence_actions BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(finish(&mut f.db, &new, &notes(&id), &coverage).is_err());
    assert_eq!(
        f.db.query_row("SELECT count(*) FROM meeting_intelligence", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    f.db.execute_batch("DROP TRIGGER block_intelligence")
        .unwrap();
    finish(&mut f.db, &new, &notes(&id), &coverage).unwrap();
    mutate(&mut f.db, Request::Retry { id: old.id.clone() }).unwrap();
    let retry = claim(&mut f.db).unwrap().unwrap();
    finish(&mut f.db, &retry, &notes(&id), &coverage).unwrap();
    assert_eq!(
        f.db.query_row(
            "SELECT result_id FROM meeting_intelligence_selection",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        new.id
    );
}
#[derive(Default)]
struct MemoryVault(std::cell::RefCell<std::collections::BTreeMap<String, String>>);
impl Vault for MemoryVault {
    fn read(&self, id: &str) -> Result<Option<zeroize::Zeroizing<String>>> {
        Ok(self
            .0
            .borrow()
            .get(id)
            .cloned()
            .map(zeroize::Zeroizing::new))
    }
    fn write(&self, id: &str, key: &str) -> Result<()> {
        self.0.borrow_mut().insert(id.into(), key.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<()> {
        self.0.borrow_mut().remove(id);
        Ok(())
    }
}
#[test]
fn generation_settings_require_separate_review_and_provider_changes_revoke_consent() {
    let mut f = Fixture::new();
    let vault = MemoryVault::default();
    let request = |reviewed, language: &str| Save {
        kind: "summarization".into(),
        mode: "remote".into(),
        local: None,
        remote: Some(Remote {
            base_url: "https://example.invalid/v1/".into(),
            model: "synthetic-model".into(),
            response_format: "json".into(),
            language: "auto".into(),
        }),
        key_action: "replace".into(),
        api_key: Some("test-key".into()),
        automatic_transcription: false,
        automatic_generation: true,
        automatic_reviewed: reviewed,
        output_language: language.into(),
        generation_format: "json_object".into(),
    };
    assert!(settings::save(&mut f.db, request(false, "en"), &vault).is_err());
    settings::save(&mut f.db, request(true, "ar"), &vault).unwrap();
    let cfg = config(&f.db).unwrap();
    assert!(automatic(&f.db, &cfg.engine).unwrap());
    assert_eq!(cfg.language, "ar");
    assert!(!settings::automatic_enabled(&f.db, &cfg.engine).unwrap());
    assert!(settings::save(&mut f.db, request(false, "en"), &vault).is_err());
}
fn mock_server(status: &str, body: String) -> (String, std::thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1/", listener.local_addr().unwrap());
    let status = status.to_string();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stream = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(_) => {
                    assert!(Instant::now() < deadline, "Mock server got no request");
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        };
        // Windows accepted sockets may inherit the listener nonblocking mode.
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = vec![];
        let mut buffer = [0; 4096];
        loop {
            let n = stream.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..n]);
            if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..pos]);
                let size = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= pos + 4 + size {
                    break;
                }
            }
        }
        write!(stream,"HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",status,body.len(),body).unwrap();
        bytes
    });
    (base, server)
}
#[test]
fn chat_adapter_validates_protocol_auth_rate_limits_and_truncation() {
    for (status, body, expected) in [
        ("401 Unauthorized", "{}", "authentication"),
        ("429 Too Many Requests", "{}", "rate_limit"),
        (
            "200 OK",
            r#"{"choices":[{"finish_reason":"length","message":{"content":"{}"}}]}"#,
            "input",
        ),
        ("200 OK", "{}", "input"),
    ] {
        let f = Fixture::new();
        let (base, server) = mock_server(status, body.into());
        let cfg = f.provider(&base);
        let vault = MemoryVault::default();
        let Engine::Remote {
            credential_id: Some(id),
            ..
        } = &cfg.engine
        else {
            panic!()
        };
        vault.write(id, "synthetic-test-key").unwrap();
        let source = vec![];
        let error =
            chat::request_with_vault(&cfg, serde_json::json!({"segments":[]}), &source, &vault)
                .err()
                .unwrap();
        assert_eq!(error.category, expected);
        let req = server.join().unwrap();
        assert!(String::from_utf8_lossy(&req).contains("POST /v1/chat/completions"));
    }
}
#[cfg(windows)]
#[test]
fn real_background_worker_completes_authorized_synthetic_transcript_without_ui() {
    let mut f = Fixture::new();

    let n = empty();
    let body=serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":serde_json::to_string(&n).unwrap()}}]}).to_string();
    let (base, server) = mock_server("200 OK", body);
    let cfg = f.provider(&base);
    let Engine::Remote {
        credential_id: Some(keyid),
        ..
    } = &cfg.engine
    else {
        panic!()
    };
    WindowsVault
        .write(keyid, "test-intelligence-worker-only")
        .unwrap();
    f.db.execute(
        "UPDATE intelligence_preferences SET automatic_revision=?1 WHERE id=1",
        [cfg.engine.revision()],
    )
    .unwrap();
    let transcript = f.transcript();
    let pipeline = Pipeline::new(f.path.clone());
    pipeline.start();
    let deadline = Instant::now() + Duration::from_secs(20);
    let complete = loop {
        let state: String =
            f.db.query_row(
                "SELECT state FROM intelligence_jobs WHERE transcript_id=?1",
                [&transcript],
                |r| r.get(0),
            )
            .unwrap();
        if state == "completed" {
            break true;
        }
        if state == "failed" || Instant::now() > deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(30));
    };
    pipeline.shutdown.store(true, Ordering::Release);
    std::thread::sleep(Duration::from_millis(600));
    WindowsVault.delete(keyid).unwrap();
    assert!(
        complete,
        "{}",
        pipeline
            .error()
            .unwrap_or_else(|| "Synthetic generation did not finish".into())
    );
    let request = server.join().unwrap();
    assert!(String::from_utf8_lossy(&request).contains("chat/completions"));
    let db = storage::open(&f.path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM meeting_intelligence", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT attempts FROM intelligence_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM transcripts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[cfg(windows)]
#[test]
fn validated_checkpoint_survives_restart_and_changed_provider_stops_requests() {
    let mut f = Fixture::new();
    let id = f.transcript();
    let body=serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":serde_json::to_string(&notes(&id)).unwrap()}}]}).to_string();
    let (base, server) = mock_server("200 OK", body);
    let cfg = f.provider(&base);
    let Engine::Remote {
        credential_id: Some(keyid),
        ..
    } = &cfg.engine
    else {
        panic!()
    };
    WindowsVault.write(keyid, "test-checkpoint-only").unwrap();
    enqueue(&f.db, &id, &cfg, "checkpoint-fixture").unwrap();
    let first = claim(&mut f.db).unwrap().unwrap();
    let source = source(&f.db, &id).unwrap();
    let input = serde_json::json!({"segments":source});
    let pipeline = Pipeline::new(f.path.clone());
    pipeline
        .stage(
            &f.db,
            &first,
            "analyzing_transcript",
            &cfg,
            input.clone(),
            &source,
        )
        .unwrap();
    server.join().unwrap();
    reconcile(&f.db).unwrap();
    f.db.execute("UPDATE intelligence_jobs SET next_attempt_at=0", [])
        .unwrap();
    let second = claim(&mut f.db).unwrap().unwrap();
    let restarted = Pipeline::new(f.path.clone());
    restarted
        .stage(
            &f.db,
            &second,
            "analyzing_transcript",
            &cfg,
            input.clone(),
            &source,
        )
        .unwrap();
    assert_eq!(
        f.db.query_row("SELECT requests FROM intelligence_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let replacement = f.provider("https://example.invalid/v1/");
    let error = restarted
        .stage(&f.db, &second, "analyzing_transcript", &cfg, input, &source)
        .err()
        .unwrap();
    assert_eq!(error.category, "configuration");
    assert_ne!(replacement.engine.revision(), cfg.engine.revision());
    WindowsVault.delete(keyid).unwrap();
}
#[test]
fn migration_preserves_old_transcript_without_scheduling_generation() {
    let directory =
        std::env::temp_dir().join(format!("miw-generation-migration-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("old.sqlite3");
    {
        let db = Connection::open(&path).unwrap();
        for sql in [
            include_str!("../../migrations/0001.sql"),
            include_str!("../../migrations/0002.sql"),
            include_str!("../../migrations/0003.sql"),
            include_str!("../../migrations/0004.sql"),
            include_str!("../../migrations/0005.sql"),
            include_str!("../../migrations/0006.sql"),
            include_str!("../../migrations/0007.sql"),
        ] {
            db.execute_batch(sql).unwrap();
        }
        db.execute_batch("INSERT INTO meetings(id,title,date,duration,summary,highlights,decision,actions,speakers) VALUES(1,'Original title','','','','[]','','[]','[]'); INSERT INTO recordings(id,meeting_id,title,state,finalized) VALUES('legacy-source',1,'Original title','saved',1); INSERT INTO processing_jobs(id,recording_id,meeting_id,state) VALUES('legacy-job','legacy-source',1,'completed'); INSERT INTO transcripts(job_id,recording_id,meeting_id,provider,segments) VALUES('legacy-job','legacy-source',1,'fixture','[]');").unwrap();
    }
    {
        let db = storage::open(&path).unwrap();
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            9
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM transcript_revisions", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM intelligence_jobs", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.query_row("SELECT title FROM meetings", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "Original title"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn persisted_read_model_reflects_completed_actions_and_keeps_source_data_independent() {
    let mut f = Fixture::new();
    let cfg = f.provider("https://example.invalid/v1/");
    let id = f.transcript();
    enqueue(&f.db, &id, &cfg, "preview-fixture").unwrap();
    let job = claim(&mut f.db).unwrap().unwrap();
    finish(
        &mut f.db,
        &job,
        &notes(&id),
        &Coverage {
            segment_count: 1,
            analyzed_segments: 1,
            chunk_count: 1,
            source_warning: None,
        },
    )
    .unwrap();
    let before = view(&f.db, None, None).unwrap();
    assert_eq!(before.previews.len(), 1);
    assert_eq!(before.previews[0].actions[0].text, "Test the application");
    assert!(!before.previews[0].actions[0].done);
    mutate(
        &mut f.db,
        Request::CompleteAction {
            id: before.previews[0].actions[0].id.clone(),
            done: true,
        },
    )
    .unwrap();
    assert!(view(&f.db, None, None).unwrap().previews[0].actions[0].done);
    assert_eq!(
        f.db.query_row("SELECT summary FROM meetings", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        ""
    );
}

#[test]
fn minutes_export_is_persisted_collision_safe_and_preserves_sources() {
    let mut f = Fixture::new();
    let cfg = f.provider("https://example.invalid/v1/");
    let id = f.transcript();
    enqueue(&f.db, &id, &cfg, "download-fixture").unwrap();
    let job = claim(&mut f.db).unwrap().unwrap();
    let coverage = Coverage {
        segment_count: 1,
        analyzed_segments: 1,
        chunk_count: 1,
        source_warning: None,
    };
    assert!(download_minutes(&mut f.db, 1, &f.directory).is_err());
    finish(&mut f.db, &job, &notes(&id), &coverage).unwrap();
    let before = view(&f.db, Some(1), None).unwrap();
    assert!(before.jobs[0].has_result);
    let name = download_minutes(&mut f.db, 1, &f.directory).unwrap();
    let original = std::fs::read(f.directory.join(&name)).unwrap();
    let second = download_minutes(&mut f.db, 1, &f.directory).unwrap();
    assert_ne!(name, second);
    assert_eq!(std::fs::read(f.directory.join(&name)).unwrap(), original);
    assert!(String::from_utf8(original).unwrap().contains("User title"));
    let arabic = super::download::save(
        &f.directory,
        "اجتماع / ../../CON:",
        "2026-10-12",
        "ملخص الاجتماع",
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(f.directory.join(arabic)).unwrap(),
        "ملخص الاجتماع"
    );
    assert_eq!(
        f.db.query_row("SELECT count(*) FROM transcripts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        f.db.query_row("SELECT count(*) FROM recordings", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    mutate(
        &mut f.db,
        Request::Generate {
            transcript_id: id,
            revision: cfg.engine.revision().into(),
            authorized: true,
            regenerate: true,
        },
    )
    .unwrap();
    assert!(download_minutes(&mut f.db, 1, &f.directory).is_err());
}

#[cfg(windows)]
#[test]
fn actual_transcription_worker_automatically_hands_off_to_generation_without_ui() {
    let f = Fixture::new();
    let (stt_base,stt_server)=mock_server("200 OK",r#"{"text":"We agreed to test the application.","segments":[{"text":"We agreed to test the application.","start":0,"end":1}]}"#.into());
    let stt_key = uuid::Uuid::new_v4().to_string();
    let stt_engine = Engine::Remote {
        remote: Remote {
            base_url: stt_base,
            model: "synthetic-stt".into(),
            response_format: "verbose_json".into(),
            language: "en".into(),
        },
        revision: uuid::Uuid::new_v4().to_string(),
        credential_id: Some(stt_key.clone()),
    };
    WindowsVault
        .write(&stt_key, "synthetic-handoff-stt")
        .unwrap();
    f.db.execute(
        "INSERT INTO ai_settings(kind,config) VALUES ('transcription',?1)",
        [serde_json::to_string(&stt_engine).unwrap()],
    )
    .unwrap();
    f.db.execute(
        "UPDATE transcription_preferences SET automatic_revision=?1 WHERE id=1",
        [stt_engine.revision()],
    )
    .unwrap();
    f.db.execute("INSERT INTO meetings(id,title,date,duration,summary,highlights,decision,actions,speakers) VALUES (1,'Handoff fixture','','','','[]','','[]','[]')",[]).unwrap();
    let recording = uuid::Uuid::new_v4().to_string();
    let root = f.directory.join("recordings");
    let audio = root.join(&recording);
    std::fs::create_dir_all(&audio).unwrap();
    let mut writer = crate::recording::wav::Wav::create(&audio.join("microphone.wav")).unwrap();
    writer.append_at(0, &[1u8, 0, 1, 0].repeat(48000)).unwrap();
    writer.finish(48000).unwrap();
    f.db.execute("INSERT INTO recordings(id,meeting_id,title,state,finalized,complete,elapsed_ms) VALUES (?1,1,'Handoff fixture','saved',1,1,1000)",[&recording]).unwrap();
    f.db.execute("INSERT INTO recording_tracks(recording_id,role,device_id,device_name,filename,frames) VALUES (?1,'microphone','','','microphone.wav',48000)",[&recording]).unwrap();
    processing::enqueue_finalized(&f.db, &recording, true).unwrap();
    let transcript: String =
        f.db.query_row("SELECT id FROM processing_jobs", [], |r| r.get(0))
            .unwrap();
    let body=serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":serde_json::to_string(&notes(&transcript)).unwrap()}}]}).to_string();
    let (base, gen_server) = mock_server("200 OK", body);
    let cfg = f.provider(&base);
    let Engine::Remote {
        credential_id: Some(key),
        ..
    } = &cfg.engine
    else {
        panic!()
    };
    WindowsVault.write(key, "synthetic-handoff-chat").unwrap();
    f.db.execute(
        "UPDATE intelligence_preferences SET automatic_revision=?1 WHERE id=1",
        [cfg.engine.revision()],
    )
    .unwrap();
    let stt = processing::Pipeline::new(f.path.clone(), root);
    let generation = Pipeline::new(f.path.clone());
    stt.start();
    generation.start();
    let deadline = Instant::now() + Duration::from_secs(20);
    let complete = loop {
        let n: i64 =
            f.db.query_row("SELECT count(*) FROM meeting_intelligence", [], |r| {
                r.get(0)
            })
            .unwrap();
        if n == 1 {
            break true;
        }
        if Instant::now() > deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(30));
    };
    stt.shutdown.store(true, Ordering::Release);
    generation.shutdown.store(true, Ordering::Release);
    std::thread::sleep(Duration::from_millis(600));
    WindowsVault.delete(&stt_key).unwrap();
    WindowsVault.delete(key).unwrap();
    assert!(complete, "Automatic pipeline did not complete.");
    stt_server.join().unwrap();
    gen_server.join().unwrap();
    register_completed(&f.db, &transcript).unwrap();
    let db = storage::open(&f.path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM intelligence_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT attempts FROM intelligence_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT state FROM processing_jobs", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "completed"
    );
    assert!(audio.join("microphone.wav").exists());
}

#[test]
fn ai_title_is_atomic_persistent_and_respects_manual_edits() {
    for edit in [None, Some("My title"), Some("Untitled Meeting")] {
        let mut f = Fixture::new();
        let cfg = f.provider("https://example.invalid/v1/");
        let id = f.transcript();
        f.db.execute(
            "UPDATE meetings SET title='Untitled Meeting',title_source='system'",
            [],
        )
        .unwrap();
        enqueue(&f.db, &id, &cfg, "title-test").unwrap();
        let job = claim(&mut f.db).unwrap().unwrap();
        let mut snapshot = storage::apply(&mut f.db, storage::Request::Load).unwrap();
        let stale = snapshot.meetings[0].clone();
        if let Some(title) = edit {
            snapshot.meetings[0].title = title.into();
            storage::apply(
                &mut f.db,
                storage::Request::UpdateMeeting {
                    meeting: snapshot.meetings.remove(0),
                    title_edited: Some(true),
                },
            )
            .unwrap();
        }
        finish(
            &mut f.db,
            &job,
            &notes(&id),
            &Coverage {
                segment_count: 1,
                analyzed_segments: 1,
                chunk_count: 1,
                source_warning: None,
            },
        )
        .unwrap();
        // A stale assignment/summary edit must not undo the selected title.
        storage::apply(
            &mut f.db,
            storage::Request::UpdateMeeting {
                meeting: stale,
                title_edited: Some(false),
            },
        )
        .unwrap();
        let reopened = storage::open(&f.path).unwrap();
        let actual: (String, String) = reopened
            .query_row("SELECT title,title_source FROM meetings", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(
            actual,
            (
                edit.unwrap_or("Test the application").into(),
                if edit.is_some() { "manual" } else { "ai" }.into()
            )
        );
    }
}
#[test]
fn legacy_placeholder_title_is_conservatively_manual() {
    let mut f = Fixture::new();
    let cfg = f.provider("https://example.invalid/v1/");
    let id = f.transcript();
    f.db.execute("UPDATE meetings SET title='Untitled Meeting'", [])
        .unwrap();
    enqueue(&f.db, &id, &cfg, "legacy-title").unwrap();
    let job = claim(&mut f.db).unwrap().unwrap();
    finish(
        &mut f.db,
        &job,
        &notes(&id),
        &Coverage {
            segment_count: 1,
            analyzed_segments: 1,
            chunk_count: 1,
            source_warning: None,
        },
    )
    .unwrap();
    assert_eq!(
        f.db.query_row("SELECT title FROM meetings", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "Untitled Meeting"
    );
}
