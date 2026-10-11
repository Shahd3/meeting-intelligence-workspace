//! Local repositories. No Tauri, UI, audio-file deletion, or provider dependencies.
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub color: String,
    pub symbol: String,
    pub overview: String,
    pub decision: Option<String>,
    pub task: String,
    pub owner: String,
    pub due: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meeting {
    pub id: i64,
    pub title: String,
    // UI uses zero for unassigned; SQLite uses NULL and a foreign key.
    pub project: i64,
    pub date: String,
    pub duration: String,
    pub summary: String,
    pub highlights: Vec<String>,
    pub decision: String,
    pub actions: Vec<String>,
    pub speakers: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Load,
    CreateWorkspace { workspace: Workspace },
    UpdateWorkspace { workspace: Workspace },
    DeleteWorkspace { id: i64 },
    CreateMeeting { meeting: Meeting },
    UpdateMeeting { meeting: Meeting },
    DeleteMeeting { id: i64 },
    Complete { key: String, value: bool },
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub meetings: Vec<Meeting>,
    pub workspaces: Vec<Workspace>,
    pub completed: BTreeMap<String, bool>,
}

type Result<T> = std::result::Result<T, String>;
fn db_error(_: rusqlite::Error) -> String {
    "Local database operation failed. Check available disk space and access, then reload your data."
        .into()
}

pub fn open(path: &Path) -> Result<Connection> {
    let mut db = Connection::open(path).map_err(db_error)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;
    db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
        .map_err(db_error)?;
    let tx = db.transaction().map_err(db_error)?;
    let version: i64 = tx
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(db_error)?;
    if version > 1 {
        return Err("This database requires a newer application version.".into());
    }
    if version == 0 {
        tx.execute_batch(include_str!("../migrations/0001.sql"))
            .map_err(db_error)?;
    }
    tx.commit().map_err(db_error)?;
    Ok(db)
}

fn text(value: &str, max: usize) -> Result<()> {
    if value.chars().count() > max || value.contains('\0') {
        return Err("A field is too long or contains invalid characters.".into());
    }
    Ok(())
}
fn workspace_valid(w: &Workspace) -> Result<()> {
    if w.name.trim().is_empty() {
        return Err("Enter a workspace name.".into());
    }
    text(&w.name, 50)?;
    text(&w.description, 180)?;
    if !["pink", "sage", "lav", "yellow", "peach"].contains(&w.color.as_str()) {
        return Err("Select a valid workspace color.".into());
    }
    for s in [&w.symbol, &w.overview, &w.task, &w.owner, &w.due] {
        text(s, 10_000)?;
    }
    text(w.decision.as_deref().unwrap_or(""), 10_000)
}
fn meeting_valid(m: &Meeting) -> Result<()> {
    if m.title.trim().is_empty() {
        return Err("Enter a meeting title.".into());
    }
    if m.project < 0 {
        return Err("Select a valid workspace.".into());
    }
    if m.project > 0 {
        id_valid(m.project)?;
    }
    text(&m.title, 90)?;
    text(&m.date, 100)?;
    text(&m.duration, 100)?;
    text(&m.summary, 100_000)?;
    text(&m.decision, 10_000)?;
    for list in [&m.highlights, &m.actions, &m.speakers] {
        if list.len() > 1000 {
            return Err("Too many meeting entries.".into());
        }
        for s in list {
            text(s, 10_000)?;
        }
    }
    Ok(())
}
fn changed(count: usize) -> Result<()> {
    if count == 0 {
        Err("This record no longer exists. Reload your data.".into())
    } else {
        Ok(())
    }
}
fn id_valid(id: i64) -> Result<()> {
    if id <= 0 || id > 9_007_199_254_740_991 {
        Err("Invalid record identity.".into())
    } else {
        Ok(())
    }
}
fn json(value: &[String]) -> Result<String> {
    serde_json::to_string(value).map_err(|_| "Unable to encode meeting entries.".into())
}
fn strings(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Vec<String>> {
    let value: String = row.get(index)?;
    serde_json::from_str(&value).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(e))
    })
}

pub fn apply(db: &mut Connection, request: Request) -> Result<Snapshot> {
    let tx = db.transaction().map_err(db_error)?;
    match request {
        Request::Load => {}
        Request::CreateWorkspace { workspace: w } => {
            workspace_valid(&w)?;
            tx.execute("INSERT INTO workspaces(name,description,color,symbol,overview,decision,task,owner,due) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![w.name.trim(),w.description,w.color,w.symbol,w.overview,w.decision,w.task,w.owner,w.due]).map_err(db_error)?;
        }
        Request::UpdateWorkspace { workspace: w } => {
            id_valid(w.id)?;
            workspace_valid(&w)?;
            changed(tx.execute("UPDATE workspaces SET name=?1,description=?2,color=?3,symbol=?4,overview=?5,decision=?6,task=?7,owner=?8,due=?9 WHERE id=?10", params![w.name.trim(),w.description,w.color,w.symbol,w.overview,w.decision,w.task,w.owner,w.due,w.id]).map_err(db_error)?)?;
        }
        Request::DeleteWorkspace { id } => {
            id_valid(id)?;
            changed(
                tx.execute("DELETE FROM workspaces WHERE id=?1", [id])
                    .map_err(db_error)?,
            )?;
        }
        Request::CreateMeeting { meeting: m } => {
            meeting_valid(&m)?;
            tx.execute("INSERT INTO meetings(title,workspace_id,date,duration,summary,highlights,decision,actions,speakers) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![m.title.trim(),if m.project==0 {None}else{Some(m.project)},m.date,m.duration,m.summary,json(&m.highlights)?,m.decision,json(&m.actions)?,json(&m.speakers)?]).map_err(db_error)?;
        }
        Request::UpdateMeeting { meeting: m } => {
            id_valid(m.id)?;
            meeting_valid(&m)?;
            // Index-based completion identities must not survive a changed action list.
            let old: String = tx
                .query_row("SELECT actions FROM meetings WHERE id=?1", [m.id], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(db_error)?
                .ok_or("This meeting no longer exists.")?;
            let actions = json(&m.actions)?;
            if old != actions {
                tx.execute("DELETE FROM meeting_completion WHERE meeting_id=?1", [m.id])
                    .map_err(db_error)?;
            }
            changed(tx.execute("UPDATE meetings SET title=?1,workspace_id=?2,date=?3,duration=?4,summary=?5,highlights=?6,decision=?7,actions=?8,speakers=?9 WHERE id=?10",params![m.title.trim(),if m.project==0 {None}else{Some(m.project)},m.date,m.duration,m.summary,json(&m.highlights)?,m.decision,actions,json(&m.speakers)?,m.id]).map_err(db_error)?)?;
        }
        Request::DeleteMeeting { id } => {
            id_valid(id)?;
            changed(
                tx.execute("DELETE FROM meetings WHERE id=?1", [id])
                    .map_err(db_error)?,
            )?;
        }
        Request::Complete { key, value } => {
            if let Some(id) = key
                .strip_prefix("workspace-")
                .and_then(|s| s.parse::<i64>().ok())
            {
                id_valid(id)?;
                tx.execute("INSERT INTO workspace_completion(workspace_id,done) VALUES (?1,?2) ON CONFLICT(workspace_id) DO UPDATE SET done=excluded.done",params![id,value]).map_err(db_error)?;
            } else if let Some((id, index)) = key.split_once('-') {
                let id = id.parse::<i64>().map_err(|_| "Invalid task identity.")?;
                let index = index
                    .parse::<usize>()
                    .map_err(|_| "Invalid task identity.")?;
                id_valid(id)?;
                let actions: Vec<String> = tx
                    .query_row("SELECT actions FROM meetings WHERE id=?1", [id], |r| {
                        strings(r, 0)
                    })
                    .map_err(db_error)?;
                if index >= actions.len() {
                    return Err("This action item no longer exists.".into());
                }
                tx.execute("INSERT INTO meeting_completion(meeting_id,action_index,done) VALUES (?1,?2,?3) ON CONFLICT(meeting_id,action_index) DO UPDATE SET done=excluded.done",params![id,index as i64,value]).map_err(db_error)?;
            } else {
                return Err("Invalid task identity.".into());
            }
        }
    }
    let result = snapshot(&tx)?;
    tx.commit().map_err(db_error)?;
    Ok(result)
}

fn snapshot(db: &Connection) -> Result<Snapshot> {
    let mut statement = db.prepare("SELECT id,name,description,color,symbol,overview,decision,task,owner,due FROM workspaces ORDER BY id").map_err(db_error)?;
    let workspaces = statement
        .query_map([], |r| {
            Ok(Workspace {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                color: r.get(3)?,
                symbol: r.get(4)?,
                overview: r.get(5)?,
                decision: r.get(6)?,
                task: r.get(7)?,
                owner: r.get(8)?,
                due: r.get(9)?,
            })
        })
        .map_err(db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_error)?;
    let mut statement = db.prepare("SELECT id,title,COALESCE(workspace_id,0),date,duration,summary,highlights,decision,actions,speakers FROM meetings ORDER BY id DESC").map_err(db_error)?;
    let meetings = statement
        .query_map([], |r| {
            Ok(Meeting {
                id: r.get(0)?,
                title: r.get(1)?,
                project: r.get(2)?,
                date: r.get(3)?,
                duration: r.get(4)?,
                summary: r.get(5)?,
                highlights: strings(r, 6)?,
                decision: r.get(7)?,
                actions: strings(r, 8)?,
                speakers: strings(r, 9)?,
            })
        })
        .map_err(db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_error)?;
    let mut completed = BTreeMap::new();
    let mut statement = db.prepare("SELECT 'workspace-'||workspace_id,done FROM workspace_completion UNION ALL SELECT meeting_id||'-'||action_index,done FROM meeting_completion").map_err(db_error)?;
    for row in statement
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))
        .map_err(db_error)?
    {
        let (key, value) = row.map_err(db_error)?;
        completed.insert(key, value);
    }
    Ok(Snapshot {
        workspaces,
        meetings,
        completed,
    })
}

#[cfg(test)]
mod tests;
