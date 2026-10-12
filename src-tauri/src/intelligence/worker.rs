use super::*;
use crate::{processing::provider::Failure, storage};
use fs2::FileExt;
use serde_json::json;
use std::{
    fs::OpenOptions,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
pub struct Pipeline {
    pub database: PathBuf,
    pub shutdown: AtomicBool,
    error: Mutex<Option<String>>,
}
impl Pipeline {
    pub fn new(database: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            database,
            shutdown: AtomicBool::new(false),
            error: Mutex::new(None),
        })
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|e| e.clone())
    }
    pub fn start(self: &Arc<Self>) {
        let worker = self.clone();
        if std::thread::Builder::new()
            .name("meeting-intelligence".into())
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.run()));
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(e)) => Some(e),
                    Err(_) => Some(
                        "Intelligence worker stopped unexpectedly. Restart to recover jobs.".into(),
                    ),
                };
                if let Ok(mut e) = worker.error.lock() {
                    *e = error
                }
            })
            .is_err()
        {
            if let Ok(mut e) = self.error.lock() {
                *e = Some("Unable to start intelligence worker.".into())
            }
        }
    }
    fn run(&self) -> Result<()> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.database.with_file_name("intelligence.lock"))
            .map_err(|_| "Unable to open intelligence worker lock.")?;
        lock.try_lock_exclusive()
            .map_err(|_| "Another application instance owns intelligence processing.")?;
        let mut db = storage::open(&self.database)?;
        reconcile(&db)?;
        while !self.shutdown.load(Ordering::Acquire) {
            if let Some(job) = claim(&mut db)? {
                if let Err(f) = self.process(&mut db, &job) {
                    fail(&db, &job, f)?;
                }
            } else {
                std::thread::sleep(Duration::from_millis(500))
            }
        }
        Ok(())
    }
    pub(super) fn stage(
        &self,
        db: &Connection,
        job: &Claim,
        key: &str,
        cfg: &Config,
        input: serde_json::Value,
        source: &[EvidenceSegment],
    ) -> std::result::Result<Notes, Failure> {
        self.check(db, job, cfg)?;
        let fingerprint =
            digest(format!("v1:{}:{}", serde_json::to_string(cfg).unwrap(), input).as_bytes());
        let saved:Option<String>=db.query_row("SELECT result FROM intelligence_checkpoints WHERE job_id=?1 AND stage=?2 AND fingerprint=?3",params![job.id,key,fingerprint],|r|r.get(0)).optional().map_err(db_error)?;
        if let Some(saved) = saved {
            let n: Notes = serde_json::from_str(&saved)
                .map_err(|_| Failure::input("Generation checkpoint is invalid."))?;
            contract::validate(&n, source)?;
            return Ok(n);
        }
        if db.execute("UPDATE intelligence_jobs SET requests=requests+1,stage=?1,updated_at=unixepoch() WHERE id=?2 AND owner=?3 AND state='processing' AND requests<64",params![key,job.id,job.owner]).map_err(db_error)?!=1{return Err(Failure::input("The 64-request generation budget was reached. Source transcript was retained."))}
        let notes = chat::request(cfg, input, source)?;
        self.check(db, job, cfg)?;
        db.execute("INSERT INTO intelligence_checkpoints(job_id,stage,fingerprint,result) VALUES (?1,?2,?3,?4) ON CONFLICT(job_id,stage) DO UPDATE SET fingerprint=excluded.fingerprint,result=excluded.result",params![job.id,key,fingerprint,serde_json::to_string(&notes).unwrap()]).map_err(db_error)?;
        Ok(notes)
    }
    fn check(
        &self,
        db: &Connection,
        job: &Claim,
        cfg: &Config,
    ) -> std::result::Result<(), Failure> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(Failure::interrupted());
        }
        owned(db, job)?;
        let engine = settings::load(db, "summarization")?
            .ok_or_else(|| Failure::configuration("Summarization settings are missing."))?;
        if engine.revision() != cfg.engine.revision() {
            return Err(Failure::configuration("Summarization provider changed. Review latest settings and regenerate; the previous result remains available."));
        }
        Ok(())
    }
    pub(super) fn process(
        &self,
        db: &mut Connection,
        job: &Claim,
    ) -> std::result::Result<(), Failure> {
        let cfg: Config = serde_json::from_str(&job.config)
            .map_err(|_| Failure::configuration("Saved generation settings are invalid."))?;
        self.check(db, job, &cfg)?;
        let source = source(db, &job.transcript)?;
        let chunks = plan(&source)?;
        let notes = if chunks.is_empty() {
            empty()
        } else if chunks.len() == 1 {
            self.stage(
                db,
                job,
                "analyzing_transcript",
                &cfg,
                json!({"segments":chunks[0]}),
                &source,
            )?
        } else {
            let mut outputs = Vec::new();
            for (i, c) in chunks.iter().enumerate() {
                outputs.push(self.stage(
                    db,
                    job,
                    &format!("extracting_{}_of_{}", i + 1, chunks.len()),
                    &cfg,
                    json!({"segments":c}),
                    c,
                )?);
            }
            // Bounded hierarchical reduction. No input is discarded to fit a request.
            for depth in 0..5 {
                let groups = group(&outputs)?;
                let mut next = vec![];
                for (i, g) in groups.iter().enumerate() {
                    next.push(self.stage(
                        db,
                        job,
                        &format!("synthesizing_{}_{}", depth, i),
                        &cfg,
                        json!({"findings":g}),
                        &source,
                    )?)
                }
                if next.len() == 1 {
                    outputs = next;
                    break;
                }
                if next.len() >= outputs.len() {
                    return Err(Failure::input("Detailed findings exceed the supported context budget. Nothing was truncated; use a higher-capacity workflow."));
                }
                outputs = next;
            }
            if outputs.len() != 1 {
                return Err(Failure::input(
                    "Generation exceeded its bounded synthesis depth.",
                ));
            }
            outputs.remove(0)
        };
        self.check(db, job, &cfg)?;
        let warning: Option<String> = db
            .query_row(
                "SELECT warning FROM transcript_revisions WHERE id=?1",
                [&job.transcript],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        finish(
            db,
            job,
            &notes,
            &Coverage {
                segment_count: source.len(),
                analyzed_segments: source.len(),
                chunk_count: chunks.len(),
                source_warning: warning,
            },
        )?;
        Ok(())
    }
}
pub(super) struct Claim {
    pub id: String,
    pub transcript: String,
    pub owner: String,
    pub attempts: u32,
    pub config: String,
}
pub(super) fn owned(db: &Connection, job: &Claim) -> Result<()> {
    if !db.query_row("SELECT EXISTS(SELECT 1 FROM intelligence_jobs WHERE id=?1 AND owner=?2 AND state='processing')",params![job.id,job.owner],|r|r.get::<_,bool>(0)).map_err(db_error)?{return Err("Generation ownership was lost.".into())}
    Ok(())
}
pub(super) fn reconcile(db: &Connection) -> Result<()> {
    db.execute("UPDATE intelligence_jobs SET state=CASE WHEN attempts<3 AND requests<64 THEN 'retryable' ELSE 'failed' END,stage='interrupted',owner=NULL,error='Generation was interrupted; transcript and checkpoints retained.',error_category='interrupted',next_attempt_at=unixepoch()+5 WHERE state='processing'",[]).map_err(db_error)?;
    Ok(())
}
pub(super) fn claim(db: &mut Connection) -> Result<Option<Claim>> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let row:Option<(String,String,u32,String)>=tx.query_row("SELECT id,transcript_id,attempts,config FROM intelligence_jobs WHERE attempts<5 AND (state='queued' OR (state='retryable' AND next_attempt_at<=unixepoch())) ORDER BY rowid LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(db_error)?;
    let job = if let Some((id, transcript, attempts, config)) = row {
        let owner = uuid::Uuid::new_v4().to_string();
        tx.execute("UPDATE intelligence_jobs SET state='processing',owner=?1,attempts=attempts+1,stage='preparing',error=NULL,error_category=NULL,updated_at=unixepoch() WHERE id=?2",params![owner,id]).map_err(db_error)?;
        Some(Claim {
            id,
            transcript,
            attempts: attempts + 1,
            config,
            owner,
        })
    } else {
        None
    };
    tx.commit().map_err(db_error)?;
    Ok(job)
}
pub(super) fn fail(db: &Connection, job: &Claim, f: Failure) -> Result<()> {
    db.execute("UPDATE intelligence_jobs SET state=?1,owner=NULL,error=?2,error_category=?3,next_attempt_at=unixepoch()+?4,updated_at=unixepoch() WHERE id=?5 AND owner=?6",params![if f.retryable&&job.attempts<3{"retryable"}else{"failed"},f.message,f.category,(10i64*2i64.pow(job.attempts.min(3))).max(f.retry_after_secs.unwrap_or(0).min(900) as i64),job.id,job.owner]).map_err(db_error)?;
    Ok(())
}
pub(super) fn finish(
    db: &mut Connection,
    job: &Claim,
    notes: &Notes,
    coverage: &Coverage,
) -> Result<()> {
    let tx = db.transaction().map_err(db_error)?;
    owned(&tx, job)?;
    contract::validate(notes, &source(&tx, &job.transcript)?).map_err(|f| f.message)?;
    tx.execute("INSERT INTO meeting_intelligence(id,meeting_id,transcript_id,result,coverage) SELECT id,meeting_id,transcript_id,?1,?2 FROM intelligence_jobs WHERE id=?3",params![serde_json::to_string(notes).unwrap(),serde_json::to_string(coverage).unwrap(),job.id]).map_err(db_error)?;
    for i in 0..notes.action_items.len() {
        tx.execute(
            "INSERT INTO intelligence_actions(id,result_id,ordinal) VALUES (?1,?2,?3)",
            params![uuid::Uuid::new_v4().to_string(), job.id, i as i64],
        )
        .map_err(db_error)?;
    }
    tx.execute("INSERT INTO meeting_intelligence_selection(meeting_id,result_id) SELECT meeting_id,id FROM intelligence_jobs WHERE id=?1 ON CONFLICT(meeting_id) DO UPDATE SET result_id=excluded.result_id WHERE (SELECT rowid FROM intelligence_jobs WHERE id=excluded.result_id)>(SELECT rowid FROM intelligence_jobs WHERE id=meeting_intelligence_selection.result_id)",[&job.id]).map_err(db_error)?;
    tx.execute("UPDATE intelligence_jobs SET state='completed',stage='complete',owner=NULL,error=NULL,error_category=NULL,completed_at=unixepoch(),updated_at=unixepoch() WHERE id=?1 AND owner=?2",params![job.id,job.owner]).map_err(db_error)?;
    tx.commit().map_err(db_error)
}
pub(super) fn plan(
    source: &[EvidenceSegment],
) -> std::result::Result<Vec<Vec<EvidenceSegment>>, Failure> {
    if source
        .iter()
        .map(|s| s.segment.text.chars().count())
        .sum::<usize>()
        > 320000
    {
        return Err(Failure::input(
            "Transcript exceeds the supported input budget; no content was truncated or uploaded.",
        ));
    }
    let mut chunks = vec![];
    let mut chunk = vec![];
    let mut size = 0;
    if source.len() > 20000 {
        return Err(Failure::input(
            "Transcript exceeds the supported segment limit; nothing was truncated.",
        ));
    }
    for s in source {
        let chars: Vec<_> = s.segment.text.chars().collect();
        for part in chars.chunks(10000) {
            if size + part.len() > 10000 && !chunk.is_empty() {
                chunks.push(std::mem::take(&mut chunk));
                size = 0;
            }
            let mut segment = s.clone();
            segment.segment.text = part.iter().collect();
            size += part.len();
            chunk.push(segment);
        }
    }
    if !chunk.is_empty() {
        chunks.push(chunk)
    }
    if chunks.len() > 32 {
        return Err(Failure::input(
            "Transcript exceeds 32 input chunks; nothing was truncated or uploaded.",
        ));
    }
    Ok(chunks)
}
fn group(outputs: &[Notes]) -> std::result::Result<Vec<Vec<Notes>>, Failure> {
    let mut groups = vec![];
    let mut current = vec![];
    let mut size = 0;
    for n in outputs {
        let length = serde_json::to_string(n).unwrap().chars().count();
        if length > 24000 {
            return Err(Failure::input("An intermediate result exceeds the synthesis context budget; no content was dropped."));
        }
        if size + length > 24000 && !current.is_empty() {
            groups.push(std::mem::take(&mut current));
            size = 0
        }
        current.push(n.clone());
        size += length;
    }
    if !current.is_empty() {
        groups.push(current)
    }
    Ok(groups)
}
pub(super) fn empty() -> Notes {
    serde_json::from_value(json!({"schema_version":1,"suggested_title":{"text":"","evidence":[]},"executive_summary":{"text":"","evidence":[]},"topics":[],"decisions":[],"action_items":[],"follow_ups":[],"open_questions":[],"attendees":[]})).unwrap()
}
