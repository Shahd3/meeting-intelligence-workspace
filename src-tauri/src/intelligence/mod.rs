//! Transcript-only intelligence: durable orchestration independent of capture and UI.
mod chat;
mod export;
pub use export::markdown as export_markdown;
pub mod contract;
#[cfg(test)]
mod tests;
mod worker;
use crate::processing::{
    self,
    settings::{self, Engine},
};
pub use chat::connection_test;
use contract::{EvidenceSegment, Notes};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub use worker::Pipeline;
type Result<T> = std::result::Result<T, String>;
pub fn db_error(_: rusqlite::Error) -> String {
    "Meeting intelligence storage is unavailable. Source transcripts and audio were retained."
        .into()
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub engine: Engine,
    pub language: String,
    pub format: String,
}
pub fn config(db: &Connection) -> Result<Config> {
    let engine =
        settings::load(db, "summarization")?.ok_or("Configure a summarization provider first.")?;
    if !matches!(
        engine,
        Engine::Remote {
            credential_id: Some(_),
            ..
        }
    ) {
        return Err("Save a summarization API key first.".into());
    }
    let (language, format) = db
        .query_row(
            "SELECT language,format FROM intelligence_preferences WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(db_error)?;
    Ok(Config {
        engine,
        language,
        format,
    })
}
pub fn automatic(db: &Connection, engine: &Engine) -> Result<bool> {
    let revision: Option<String> = db
        .query_row(
            "SELECT automatic_revision FROM intelligence_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    Ok(revision.as_deref() == Some(engine.revision()))
}
pub fn backfill(db: &Connection) -> Result<()> {
    let rows: Vec<String> = {
        let mut q=db.prepare("SELECT t.job_id FROM transcripts t LEFT JOIN transcript_revisions r ON r.id=t.job_id WHERE r.id IS NULL").map_err(db_error)?;
        let rows = q
            .query_map([], |r| r.get(0))
            .map_err(db_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_error)?;
        rows
    };
    for id in rows {
        snapshot(db, &id)?;
    }
    Ok(())
}
fn snapshot(db: &Connection, id: &str) -> Result<()> {
    let (meeting, json, warning): (i64, String, Option<String>) = db
        .query_row(
            "SELECT meeting_id,segments,warning FROM transcripts WHERE job_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(db_error)?;
    let segments: Vec<processing::Segment> =
        serde_json::from_str(&json).map_err(|_| "Transcript segments are invalid.")?;
    let source = contract::evidence_segments(id, segments);
    db.execute("INSERT OR IGNORE INTO transcript_revisions(id,meeting_id,digest,segments,warning) VALUES (?1,?2,?3,?4,?5)",params![id,meeting,digest(json.as_bytes()),serde_json::to_string(&source).map_err(|_|"Invalid evidence.")?,warning]).map_err(db_error)?;
    Ok(())
}
pub fn register_completed(db: &Connection, id: &str) -> Result<()> {
    snapshot(db, id)?;
    let Some(engine) = settings::load(db, "summarization").ok().flatten() else {
        return Ok(());
    };
    if !automatic(db, &engine)? {
        return Ok(());
    }
    let fresh:bool=db.query_row("SELECT r.rowid>p.after_recording_rowid FROM transcripts t JOIN recordings r ON r.id=t.recording_id CROSS JOIN intelligence_preferences p WHERE t.job_id=?1",[id],|r|r.get(0)).map_err(db_error)?;
    if fresh {
        let cfg = config(db)?;
        enqueue(db, id, &cfg, &format!("auto:{id}"))?;
    }
    Ok(())
}
fn enqueue(db: &Connection, transcript: &str, cfg: &Config, operation: &str) -> Result<String> {
    if let Some(id)=db.query_row("SELECT id FROM intelligence_jobs WHERE operation=?1 OR (transcript_id=?2 AND state IN ('queued','processing','retryable')) LIMIT 1",params![operation,transcript],|r|r.get(0)).optional().map_err(db_error)?{return Ok(id)}
    let meeting: i64 = db
        .query_row(
            "SELECT meeting_id FROM transcript_revisions WHERE id=?1",
            [transcript],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let id = uuid::Uuid::new_v4().to_string();
    db.execute("INSERT INTO intelligence_jobs(id,transcript_id,meeting_id,operation,state,config) VALUES (?1,?2,?3,?4,'queued',?5)",params![id,transcript,meeting,operation,serde_json::to_string(cfg).map_err(|_|"Invalid generation configuration.")?]).map_err(db_error)?;
    Ok(id)
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Generate {
        transcript_id: String,
        revision: String,
        authorized: bool,
        regenerate: bool,
    },
    Retry {
        id: String,
    },
    CompleteAction {
        id: String,
        done: bool,
    },
    Attendees {
        meeting_id: i64,
        names: Option<Vec<String>>,
    },
}
pub fn mutate(db: &mut Connection, request: Request) -> Result<()> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db_error)?;
    match request {
 Request::Generate{transcript_id,revision,authorized,regenerate}=>{let cfg=config(&tx)?; if cfg.engine.revision()!=revision{return Err("Provider settings changed. Review the saved destination.".into())}if !authorized && !automatic(&tx,&cfg.engine)?{return Err("Approve sharing this transcript with the selected provider.".into())}
 let operation=if regenerate{format!("regenerate:{}",uuid::Uuid::new_v4())}else{format!("manual:{transcript_id}:{}",cfg.engine.revision())};enqueue(&tx,&transcript_id,&cfg,&operation)?;},
 Request::Retry{id}=>{if tx.execute("UPDATE intelligence_jobs SET state='queued',stage='waiting',error=NULL,error_category=NULL,next_attempt_at=NULL,updated_at=unixepoch() WHERE id=?1 AND state='failed' AND attempts<5 AND requests<64",[id]).map_err(db_error)?!=1{return Err("Only failed jobs within the retry budget can retry.".into())}},
 Request::CompleteAction{id,done}=>{if tx.execute("UPDATE intelligence_actions SET done=?1 WHERE id=?2",params![done,id]).map_err(db_error)?!=1{return Err("Action item no longer exists.".into())}},
 Request::Attendees{meeting_id,names}=>{if let Some(names)=names {if names.len()>100 || names.iter().any(|n|n.trim().is_empty()||n.chars().count()>150||n.chars().any(char::is_control)){return Err("Enter at most 100 valid attendee names.".into())}let mut cleaned:Vec<_>=names.into_iter().map(|n|n.trim().to_string()).collect();cleaned.sort();cleaned.dedup();tx.execute("INSERT INTO intelligence_attendee_overrides(meeting_id,names) VALUES (?1,?2) ON CONFLICT(meeting_id) DO UPDATE SET names=excluded.names",params![meeting_id,serde_json::to_string(&cleaned).unwrap()]).map_err(db_error)?;}else{tx.execute("DELETE FROM intelligence_attendee_overrides WHERE meeting_id=?1",[meeting_id]).map_err(db_error)?;}}
 };
    tx.commit().map_err(db_error)
}
#[derive(Serialize)]
pub struct Job {
    pub id: String,
    pub meeting_id: i64,
    pub transcript_id: String,
    pub state: String,
    pub stage: String,
    pub attempts: u32,
    pub requests: u32,
    pub error: Option<String>,
    pub next_attempt_at: Option<i64>,
}
#[derive(Serialize)]
pub struct ActionState {
    pub id: String,
    pub ordinal: usize,
    pub done: bool,
}
#[derive(Serialize, Deserialize)]
pub struct Coverage {
    pub segment_count: usize,
    pub analyzed_segments: usize,
    pub chunk_count: usize,
    pub source_warning: Option<String>,
}
#[derive(Serialize)]
pub struct ResultView {
    pub id: String,
    pub transcript_id: String,
    pub notes: Notes,
    pub coverage: Coverage,
    pub actions: Vec<ActionState>,
}
#[derive(Serialize)]
pub struct TranscriptChoice {
    pub id: String,
    pub segment_count: usize,
}
#[derive(Serialize)]
pub struct PreviewAction {
    pub id: String,
    pub text: String,
    pub owner: Option<String>,
    pub deadline: Option<String>,
    pub done: bool,
}
#[derive(Serialize)]
pub struct Preview {
    pub meeting_id: i64,
    pub summary: String,
    pub topics: String,
    pub decision: String,
    pub actions: Vec<PreviewAction>,
}
#[derive(Serialize)]
pub struct View {
    pub previews: Vec<Preview>,
    pub jobs: Vec<Job>,
    pub result: Option<ResultView>,
    pub transcripts: Vec<TranscriptChoice>,
    pub attendees_override: Option<Vec<String>>,
    pub worker_error: Option<String>,
}
pub fn view(db: &Connection, meeting: Option<i64>, worker_error: Option<String>) -> Result<View> {
    let mut q=db.prepare("SELECT id,meeting_id,transcript_id,state,stage,attempts,requests,error,next_attempt_at FROM intelligence_jobs ORDER BY rowid").map_err(db_error)?;
    let jobs = q
        .query_map([], |r| {
            Ok(Job {
                id: r.get(0)?,
                meeting_id: r.get(1)?,
                transcript_id: r.get(2)?,
                state: r.get(3)?,
                stage: r.get(4)?,
                attempts: r.get(5)?,
                requests: r.get(6)?,
                error: r.get(7)?,
                next_attempt_at: r.get(8)?,
            })
        })
        .map_err(db_error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(db_error)?;
    let row:Option<(String,String,String,String)>=db.query_row("SELECT i.id,i.transcript_id,i.result,i.coverage FROM meeting_intelligence i JOIN meeting_intelligence_selection s ON s.result_id=i.id WHERE s.meeting_id=?1",[meeting],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(db_error)?;
    let result = if let Some((id, transcript_id, json, coverage)) = row {
        let mut q=db.prepare("SELECT id,ordinal,done FROM intelligence_actions WHERE result_id=?1 ORDER BY ordinal").map_err(db_error)?;
        let actions = q
            .query_map([&id], |r| {
                Ok(ActionState {
                    id: r.get(0)?,
                    ordinal: r.get::<_, i64>(1)? as usize,
                    done: r.get(2)?,
                })
            })
            .map_err(db_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_error)?;
        Some(ResultView {
            id,
            transcript_id,
            notes: serde_json::from_str(&json).map_err(|_| "Saved intelligence is invalid.")?,
            coverage: serde_json::from_str(&coverage).map_err(|_| "Saved coverage is invalid.")?,
            actions,
        })
    } else {
        None
    };
    let mut q=db.prepare("SELECT id,json_array_length(segments) FROM transcript_revisions WHERE meeting_id=?1 ORDER BY created_at DESC,id").map_err(db_error)?;
    let transcripts = q
        .query_map([meeting], |r| {
            Ok(TranscriptChoice {
                id: r.get(0)?,
                segment_count: r.get::<_, i64>(1)? as usize,
            })
        })
        .map_err(db_error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(db_error)?;
    let names: Option<String> = db
        .query_row(
            "SELECT names FROM intelligence_attendee_overrides WHERE meeting_id=?1",
            [meeting],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let mut q=db.prepare("SELECT i.id,i.meeting_id,i.result FROM meeting_intelligence i JOIN meeting_intelligence_selection s ON s.result_id=i.id ORDER BY i.created_at DESC,i.rowid DESC").map_err(db_error)?;
    let records = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_error)?;
    let mut previews = vec![];
    for (id, meeting_id, json) in records {
        let notes: Notes =
            serde_json::from_str(&json).map_err(|_| "Saved intelligence is invalid.")?;
        let mut q=db.prepare("SELECT id,ordinal,done FROM intelligence_actions WHERE result_id=?1 ORDER BY ordinal").map_err(db_error)?;
        let states = q
            .query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)? as usize,
                    r.get::<_, bool>(2)?,
                ))
            })
            .map_err(db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_error)?;
        let actions = states
            .into_iter()
            .filter_map(|(id, i, done)| {
                notes.action_items.get(i).map(|a| PreviewAction {
                    id,
                    text: a.description.text.clone(),
                    owner: a.owner.clone(),
                    deadline: a.deadline.clone(),
                    done,
                })
            })
            .collect();
        previews.push(Preview {
            meeting_id,
            summary: notes.executive_summary.text,
            decision: notes
                .decisions
                .first()
                .map(|p| p.text.clone())
                .unwrap_or_default(),
            topics: notes
                .topics
                .iter()
                .map(|t| {
                    format!(
                        "{} {}",
                        t.heading,
                        t.points
                            .iter()
                            .map(|p| p.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                })
                .collect::<Vec<_>>()
                .join(" "),
            actions,
        });
    }
    Ok(View {
        previews,
        jobs,
        result,
        transcripts,
        attendees_override: names
            .map(|s| serde_json::from_str(&s).map_err(|_| "Saved attendees are invalid."))
            .transpose()?,
        worker_error,
    })
}
pub fn source(db: &Connection, id: &str) -> Result<Vec<EvidenceSegment>> {
    let json: String = db
        .query_row(
            "SELECT segments FROM transcript_revisions WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if json.len() > 4 * 1024 * 1024 {
        return Err(
            "Transcript revision exceeds the supported size; no data was truncated.".into(),
        );
    }
    serde_json::from_str(&json).map_err(|_| "Saved transcript revision is invalid.".into())
}
