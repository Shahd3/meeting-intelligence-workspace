//! Durable local transcription orchestration. No UI, cloud, or capture ownership.
#[cfg(test)]
mod ai_tests;
pub mod credentials;
mod provider;
mod remote;
pub mod settings;
#[cfg(test)]
mod tests;
use crate::storage;
use fs2::FileExt;
pub use provider::Config;
pub fn test_connection(engine: &settings::Engine, kind: &str) -> Result<String> {
    remote::connection_test(engine, kind).map_err(|e| e.message)
}
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
type Result<T> = std::result::Result<T, String>;
fn db_error(_: rusqlite::Error) -> String {
    "Transcription storage is unavailable. Source audio was retained.".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub role: String,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Segment {
    pub text: String,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub sources: Vec<Source>,
}
#[derive(Debug, Serialize)]
pub struct Job {
    pub id: String,
    pub meeting_id: i64,
    pub recording_id: String,
    pub state: String,
    pub stage: String,
    pub ready: bool,
    pub provider_revision: Option<String>,
    pub attempts: u32,
    pub error: Option<String>,
    pub error_category: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub next_attempt_at: Option<i64>,
    pub checkpoints: u32,
    pub segments: Option<Vec<Segment>>,
    pub warning: Option<String>,
}
#[derive(Serialize)]
pub struct View {
    pub jobs: Vec<Job>,
    pub config: Option<Config>,
    pub ai: settings::View,
    pub worker_error: Option<String>,
    pub transcript_meeting_id: Option<i64>,
}
fn insert_job(db: &Connection, recording: &str) -> Result<bool> {
    let inserted=db.execute("INSERT OR IGNORE INTO processing_jobs(id,recording_id,meeting_id,state,stage,ready) SELECT ?1,id,meeting_id,'queued','manual',0 FROM recordings WHERE id=?2 AND finalized=1 AND meeting_id IS NOT NULL",params![uuid::Uuid::new_v4().to_string(),recording]).map_err(db_error)?;
    Ok(inserted == 1)
}
pub fn enqueue(db: &Connection, recording: &str) -> Result<()> {
    insert_job(db, recording)?;
    Ok(())
}
// Called only for a new finalization in the recording transaction. Backfill and
// duplicate saves use enqueue() and never upgrade older jobs automatically.
pub fn enqueue_finalized(db: &Connection, recording: &str, permit_auto: bool) -> Result<()> {
    if !insert_job(db, recording)? || !permit_auto {
        return Ok(());
    }
    let Some(engine) = settings::load(db, "transcription").ok().flatten() else {
        return Ok(());
    };
    if !settings::automatic_enabled(db, &engine)? {
        return Ok(());
    }
    let eligible:bool=db.query_row("SELECT r.rowid>p.after_recording_rowid FROM recordings r CROSS JOIN transcription_preferences p WHERE r.id=?1 AND p.id=1",[recording],|r|r.get(0)).map_err(db_error)?;
    if !eligible {
        return Ok(());
    }
    let upload = if matches!(engine, settings::Engine::Remote { .. }) {
        Some(engine.revision())
    } else {
        None
    };
    db.execute("UPDATE processing_jobs SET ready=1,stage='waiting',config=?1,upload_revision=?2 WHERE recording_id=?3",params![serde_json::to_string(&engine).map_err(|_|"Invalid provider settings.")?,upload,recording]).map_err(db_error)?;
    Ok(())
}

pub fn reconcile(db: &Connection) -> Result<()> {
    // Caller owns the exclusive OS worker lock; a live owner cannot be reclaimed.
    db.execute("UPDATE processing_jobs SET state=CASE WHEN attempts<3 THEN 'retryable' ELSE 'failed' END, stage='interrupted',owner=NULL,error_category='interrupted',error='Processing was interrupted. Saved checkpoints and audio were retained.',next_attempt_at=unixepoch()+5,updated_at=unixepoch() WHERE state='processing'",[]).map_err(db_error)?;
    let recordings: Vec<String> = {
        let mut q = db
            .prepare("SELECT id FROM recordings WHERE finalized=1 AND meeting_id IS NOT NULL")
            .map_err(db_error)?;
        let rows = q
            .query_map([], |r| r.get(0))
            .map_err(db_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_error)?;
        rows
    };
    for recording in recordings {
        enqueue(db, &recording)?;
    }
    Ok(())
}
pub fn config(db: &Connection) -> Result<Option<Config>> {
    let text: Option<String> = db
        .query_row(
            "SELECT config FROM transcription_settings WHERE id=1",
            [],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    text.map(|s| {
        serde_json::from_str(&s).map_err(|_| "Local transcription configuration is invalid.".into())
    })
    .transpose()
}
pub fn set_config(db: &mut Connection, mut config: Config) -> Result<()> {
    if db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM ai_settings WHERE kind='transcription')",
            [],
            |r| r.get::<_, bool>(0),
        )
        .map_err(db_error)?
    {
        return Err("Use AI Processing settings to change the configured provider.".into());
    }
    config.validate_and_fingerprint()?;
    let tx = db.transaction().map_err(db_error)?;
    tx.execute(
        "UPDATE transcription_settings SET config=?1 WHERE id=1",
        [serde_json::to_string(&config).map_err(|_| "Invalid configuration.")?],
    )
    .map_err(db_error)?;
    // Configuration errors are actionable, not transient: a corrected config wakes them.
    tx.execute("UPDATE processing_jobs SET state='queued',error=NULL,error_category=NULL,updated_at=unixepoch() WHERE state='failed' AND error_category='configuration' AND attempts<5",[]).map_err(db_error)?;
    tx.commit().map_err(db_error)
}
pub fn retry(db: &Connection, id: &str) -> Result<()> {
    let changed=db.execute("UPDATE processing_jobs SET state='queued',stage='waiting',error=NULL,error_category=NULL,next_attempt_at=NULL,updated_at=unixepoch() WHERE id=?1 AND ready=1 AND state IN ('failed','retryable') AND attempts<5",[id]).map_err(db_error)?;
    if changed == 0 {
        return Err("Retry is unavailable: the job is active, complete, missing, or its five-attempt lifetime limit was reached.".into());
    }
    Ok(())
}
pub fn view(
    db: &Connection,
    worker_error: Option<String>,
    meeting_id: Option<i64>,
) -> Result<View> {
    let mut q=db.prepare("SELECT j.id,j.meeting_id,j.recording_id,j.state,j.stage,j.attempts,j.error,j.error_category,j.created_at,j.updated_at,j.started_at,j.completed_at,j.next_attempt_at,(SELECT count(*) FROM transcription_checkpoints c WHERE c.job_id=j.id),CASE WHEN j.meeting_id=?1 THEN t.segments ELSE NULL END,t.warning,j.ready,json_extract(j.config,'$.revision') FROM processing_jobs j LEFT JOIN transcripts t ON t.job_id=j.id ORDER BY j.created_at DESC").map_err(db_error)?;
    let jobs = q
        .query_map([meeting_id], |r| {
            let json: Option<String> = r.get(14)?;
            let segments = json
                .map(|s| {
                    serde_json::from_str::<Vec<Segment>>(&s).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            14,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })
                })
                .transpose()?;
            Ok(Job {
                id: r.get(0)?,
                meeting_id: r.get(1)?,
                recording_id: r.get(2)?,
                state: r.get(3)?,
                stage: r.get(4)?,
                ready: r.get(16)?,
                provider_revision: r.get(17)?,
                attempts: r.get(5)?,
                error: r.get(6)?,
                error_category: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
                started_at: r.get(10)?,
                completed_at: r.get(11)?,
                next_attempt_at: r.get(12)?,
                checkpoints: r.get(13)?,
                segments,
                warning: r.get(15)?,
            })
        })
        .map_err(db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_error)?;
    Ok(View {
        jobs,
        config: config(db)?,
        ai: settings::view(db)?,
        worker_error,
        transcript_meeting_id: meeting_id,
    })
}
fn claim(db: &mut Connection) -> Result<Option<(String, String, String, u32)>> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let row:Option<(String,String,u32)>=tx.query_row("SELECT id,recording_id,attempts FROM processing_jobs WHERE ready=1 AND attempts<5 AND (state='queued' OR (state='retryable' AND next_attempt_at<=unixepoch())) ORDER BY created_at LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
    let result = if let Some((id, recording, attempts)) = row {
        let owner = uuid::Uuid::new_v4().to_string();
        tx.execute("UPDATE processing_jobs SET state='processing',stage='validating',owner=?1,attempts=attempts+1,started_at=unixepoch(),updated_at=unixepoch(),error=NULL,error_category=NULL WHERE id=?2",params![owner,id]).map_err(db_error)?;
        Some((id, recording, owner, attempts + 1))
    } else {
        None
    };
    tx.commit().map_err(db_error)?;
    Ok(result)
}
fn owned(db: &Connection, id: &str, owner: &str) -> Result<()> {
    if !db.query_row("SELECT EXISTS(SELECT 1 FROM processing_jobs WHERE id=?1 AND owner=?2 AND state='processing')",params![id,owner],|r|r.get::<_,bool>(0)).map_err(db_error)? {return Err("Processing ownership ended; results were not committed.".into());}
    Ok(())
}
fn checkpoint(
    db: &mut Connection,
    id: &str,
    owner: &str,
    source: &str,
    chunk: u32,
    key: &str,
    segments: &[Segment],
) -> Result<()> {
    let tx = db.transaction().map_err(db_error)?;
    owned(&tx, id, owner)?;
    tx.execute("INSERT INTO transcription_checkpoints(job_id,source,chunk,fingerprint,segments) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(job_id,source,chunk) DO UPDATE SET fingerprint=excluded.fingerprint,segments=excluded.segments",params![id,source,chunk,key,serde_json::to_string(segments).map_err(|_|"Invalid segments.")?]).map_err(db_error)?;
    tx.execute(
        "UPDATE processing_jobs SET stage=?1,updated_at=unixepoch() WHERE id=?2",
        params![format!("transcribing_{source}"), id],
    )
    .map_err(db_error)?;
    tx.commit().map_err(db_error)
}
fn complete(
    db: &mut Connection,
    id: &str,
    owner: &str,
    segments: Vec<Segment>,
    warning: Option<String>,
) -> Result<()> {
    let tx = db.transaction().map_err(db_error)?;
    owned(&tx, id, owner)?;
    let missing_timestamps = segments.iter().any(|s| s.start_ms.is_none());
    let warning = if missing_timestamps {
        Some(format!("{}Provider did not return segment timestamps. Word timing and cross-source echo matching are unavailable.",warning.map(|w|format!("{w} ")).unwrap_or_default()))
    } else {
        warning
    };
    let segments = merge_echoes(segments);
    tx.execute("INSERT INTO transcripts(job_id,meeting_id,recording_id,provider,segments,warning) SELECT id,meeting_id,recording_id,CASE WHEN json_extract(config,'$.mode')='remote' THEN 'openai-compatible' ELSE 'whisper.cpp' END,?1,?2 FROM processing_jobs WHERE id=?3",params![serde_json::to_string(&segments).map_err(|_|"Invalid transcript.")?,warning,id]).map_err(db_error)?;
    tx.execute("UPDATE processing_jobs SET state='completed',stage='complete',owner=NULL,error=NULL,error_category=NULL,completed_at=unixepoch(),updated_at=unixepoch() WHERE id=?1",[id]).map_err(db_error)?;
    tx.commit().map_err(db_error)
}
fn record_failure(
    db: &Connection,
    id: &str,
    owner: &str,
    attempts: u32,
    failure: provider::Failure,
) -> Result<()> {
    let retryable = failure.retryable && attempts < 3;
    db.execute("UPDATE processing_jobs SET state=?1,stage='waiting',owner=NULL,error=?2,error_category=?3,attempts=attempts-CASE WHEN ?3='consent' AND attempts>0 THEN 1 ELSE 0 END,next_attempt_at=unixepoch()+?4,updated_at=unixepoch() WHERE id=?5 AND owner=?6",params![if retryable{"retryable"}else{"failed"},failure.message,failure.category,(10i64*2i64.pow(attempts.min(3))).max(failure.retry_after_secs.unwrap_or(0).min(900) as i64),id,owner]).map_err(db_error)?;
    Ok(())
}
fn merge_echoes(mut segments: Vec<Segment>) -> Vec<Segment> {
    fn normalized(s: &str) -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    }
    segments.sort_by_key(|s| s.start_ms.unwrap_or(u64::MAX));
    let mut merged: Vec<Segment> = vec![];
    for s in segments {
        let duplicate = merged
            .iter_mut()
            .rev()
            .take_while(|m| {
                m.end_ms.unwrap_or(0).saturating_add(2000) >= s.start_ms.unwrap_or(u64::MAX)
            })
            .find(|m| {
                m.sources.iter().all(|r| r.role != s.sources[0].role)
                    && m.start_ms
                        .zip(s.start_ms)
                        .map(|(a, b)| a.abs_diff(b) <= 2000)
                        .unwrap_or(false)
                    && m.end_ms
                        .zip(s.end_ms)
                        .map(|(a, b)| a.abs_diff(b) <= 2000)
                        .unwrap_or(false)
                    && normalized(&m.text) == normalized(&s.text)
            });
        if let Some(old) = duplicate {
            old.sources.extend(s.sources);
        } else {
            merged.push(s);
        }
    }
    merged
}
pub struct Pipeline {
    pub shutdown: AtomicBool,
    pub database: PathBuf,
    pub root: PathBuf,
    error: std::sync::Mutex<Option<String>>,
}
impl Pipeline {
    pub fn new(database: PathBuf, root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            shutdown: AtomicBool::new(false),
            database,
            root,
            error: std::sync::Mutex::new(None),
        })
    }
    pub fn status(&self, meeting_id: Option<i64>) -> Result<View> {
        view(
            &storage::open(&self.database)?,
            self.error
                .lock()
                .map_err(|_| "Worker state unavailable.")?
                .clone(),
            meeting_id,
        )
    }
    pub fn start(self: &Arc<Self>) {
        let worker = self.clone();
        if std::thread::Builder::new().name("meeting-transcription".into()).spawn(move||{
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||worker.run()));
        let error=match result {Ok(Ok(()))=>None,Ok(Err(e))=>Some(e),Err(_)=>Some("Transcription worker stopped unexpectedly. Restart to recover queued work.".into())};
        if let Ok(mut state)=worker.error.lock(){*state=error;}
    }).is_err(){if let Ok(mut error)=self.error.lock(){*error=Some("Unable to start local transcription worker.".into());}}
    }
    fn run(&self) -> Result<()> {
        let directory = self
            .database
            .parent()
            .ok_or("Missing application directory.")?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("processing.lock"))
            .map_err(|_| "Unable to acquire processing lock.")?;
        lock.try_lock_exclusive()
            .map_err(|_| "Another application instance owns transcription processing.")?;
        let mut db = storage::open(&self.database)?;
        reconcile(&db)?;
        // Retired references cannot be used by a current remote settings revision.
        // Failure remains visible through settings status; it does not block audio recovery.
        let _ = settings::cleanup(&db, &credentials::WindowsVault);
        while !self.shutdown.load(Ordering::Acquire) {
            if let Some((id, recording, owner, attempts)) = claim(&mut db)? {
                let outcome = self.process(&mut db, &id, &recording, &owner);
                if let Err(failure) = outcome {
                    record_failure(&db, &id, &owner, attempts, failure)?;
                }
            } else {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        Ok(())
    }
    fn process(
        &self,
        db: &mut Connection,
        id: &str,
        recording_id: &str,
        owner: &str,
    ) -> std::result::Result<(), provider::Failure> {
        use provider::{Failure, Provider};
        let saved: Option<String> = db
            .query_row(
                "SELECT config FROM processing_jobs WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let engine = if let Some(json) = saved {
            settings::parse_job(&json)?
        } else {
            settings::load(db, "transcription")?.ok_or_else(|| {
                Failure::configuration(
                    "Configure a transcription provider in AI Processing settings, then retry.",
                )
            })?
        };
        settings::ensure_remote_current(db, &engine)?;
        if let settings::Engine::Remote { revision, .. } = &engine {
            let approval: Option<String> = db
                .query_row(
                    "SELECT upload_revision FROM processing_jobs WHERE id=?1",
                    [id],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            if approval.as_deref() != Some(revision) {
                return Err(Failure{category:"consent",message:"Audio upload has not been authorized. Review the saved provider and approve this recording in meeting details.".into(),retryable:false,retry_after_secs:None});
            }
        }
        let config_json = serde_json::to_string(&engine)
            .map_err(|_| Failure::configuration("Invalid provider configuration."))?;
        owned(db, id, owner)?;
        db.execute(
            "UPDATE processing_jobs SET config=?1 WHERE id=?2 AND owner=?3",
            params![config_json, id, owner],
        )
        .map_err(db_error)?;
        let adapter: Box<dyn Provider> = match &engine {
            settings::Engine::Local { local, .. } => {
                local.verify()?;
                Box::new(provider::WhisperCli {
                    config: local,
                    shutdown: &self.shutdown,
                })
            }
            settings::Engine::Remote {
                remote,
                credential_id,
                ..
            } => Box::new(remote::RemoteProvider {
                settings: remote,
                credential_id: credential_id.as_deref().ok_or_else(|| {
                    Failure::configuration("Save a transcription API key before processing.")
                })?,
            }),
        };
        let recording = crate::recording::list(db)?
            .into_iter()
            .find(|r| r.id == recording_id)
            .ok_or_else(|| Failure::input("The source recording is unavailable."))?;
        uuid::Uuid::parse_str(recording_id)
            .map_err(|_| Failure::input("Invalid recording reference."))?;
        if recording.tracks.is_empty() {
            return Err(Failure::input("No audio tracks are available."));
        }
        let scratch = directory_scratch(&self.root, id)?;
        let mut all = vec![];
        for track in &recording.tracks {
            if !["microphone", "system"].contains(&track.role.as_str())
                || ![
                    format!("{}.wav", track.role),
                    format!("{}.recovered.wav", track.role),
                ]
                .contains(&track.filename)
            {
                return Err(Failure::input("Invalid source track reference."));
            }
            let path = self.root.join(recording_id).join(&track.filename);
            let mut audio = provider::Audio::open(&path, track.frames)?;
            let mut chunk = 0;
            while let Some(pcm) = audio.next_chunk()? {
                if self.shutdown.load(Ordering::Acquire) {
                    return Err(Failure::interrupted());
                }
                owned(db, id, owner)?;
                settings::ensure_remote_current(db, &engine)?;
                let key = settings::fingerprint(&engine, &pcm);
                let saved:Option<String>=db.query_row("SELECT segments FROM transcription_checkpoints WHERE job_id=?1 AND source=?2 AND chunk=?3 AND fingerprint=?4",params![id,track.role,chunk,key],|r|r.get(0)).optional().map_err(db_error)?;
                let offset = chunk as u64 * provider::CHUNK_MS;
                let duration = pcm.len() as u64 * 1000 / 16000;
                let segments = if let Some(json) = saved {
                    let segments: Vec<Segment> = serde_json::from_str(&json)
                        .map_err(|_| Failure::input("A saved checkpoint is invalid."))?;
                    provider::validate(&segments, &track.role, offset, duration)?;
                    segments
                } else {
                    db.execute("UPDATE processing_jobs SET stage=?1,updated_at=unixepoch() WHERE id=?2 AND owner=?3",params![format!("transcribing_{}_chunk_{}",track.role,chunk+1),id,owner]).map_err(db_error)?;
                    let segments = adapter.transcribe(&pcm, &scratch.path, &track.role, offset)?;
                    checkpoint(db, id, owner, &track.role, chunk, &key, &segments)?;
                    segments
                };
                all.extend(segments);
                chunk += 1;
            }
        }
        settings::ensure_remote_current(db, &engine)?;
        complete(
            db,
            id,
            owner,
            all,
            if recording.complete {
                None
            } else {
                Some("Recovered or discontinuous source audio: transcript coverage may be incomplete.".into())
            },
        )?;
        Ok(())
    }
}
struct Scratch {
    path: PathBuf,
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
fn directory_scratch(root: &std::path::Path, id: &str) -> Result<Scratch> {
    if uuid::Uuid::parse_str(id)
        .map_err(|_| "Invalid job identity.")?
        .to_string()
        != id
    {
        return Err("Invalid job identity.".into());
    }
    let path = root.join("transcription-scratch").join(id);
    std::fs::create_dir_all(&path)
        .map_err(|_| "Unable to create local transcription scratch storage.")?;
    Ok(Scratch { path })
}
