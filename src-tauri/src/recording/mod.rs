mod playback;
mod service;
pub(crate) mod wav;
#[cfg(windows)]
mod windows;
pub use playback::response;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
pub use service::{Action, Recorder, State as RecordingState, Status};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
}
#[derive(Serialize)]
pub struct Devices {
    pub microphones: Vec<Device>,
    pub outputs: Vec<Device>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub microphone: bool,
    pub system: bool,
    pub microphone_device: Option<String>,
    pub system_device: Option<String>,
    pub title: String,
}
impl Selection {
    fn validate(&self) -> Result<()> {
        if !self.microphone && !self.system {
            return Err("Choose at least one audio source.".into());
        }
        if self.title.chars().count() > 90 || self.title.contains('\0') {
            return Err("Meeting titles must be at most 90 characters.".into());
        }
        for id in [&self.microphone_device, &self.system_device]
            .into_iter()
            .flatten()
        {
            if id.len() > 2000 || id.contains('\0') {
                return Err("Invalid audio device identity.".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub role: String,
    pub device_id: String,
    pub device_name: String,
    pub filename: String,
    pub frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
}
fn manual_title_source() -> String {
    "manual".into()
}
pub(crate) fn initial_title(title: &str) -> (String, String) {
    if title.trim().is_empty() {
        ("Untitled Meeting".into(), "system".into())
    } else {
        (title.trim().into(), "manual".into())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recording {
    pub id: String,
    pub meeting_id: Option<i64>,
    #[serde(default = "manual_title_source")]
    pub title_source: String,
    pub title: String,
    pub state: String,
    pub elapsed_ms: u64,
    pub recovered: bool,
    pub complete: bool,
    pub warning: Option<String>,
    pub created_at: String,
    pub tracks: Vec<Track>,
}
fn db_error(_: rusqlite::Error) -> String {
    "Recording metadata could not be saved. Audio files were retained; retry recovery after checking disk access.".into()
}
pub fn list(db: &Connection) -> Result<Vec<Recording>> {
    let mut statement=db.prepare("SELECT id,meeting_id,title,state,elapsed_ms,recovered,complete,warning,created_at,title_source FROM recordings ORDER BY created_at DESC").map_err(db_error)?;
    let mut recordings = statement
        .query_map([], |r| {
            Ok(Recording {
                id: r.get(0)?,
                meeting_id: r.get(1)?,
                title_source: r.get(9)?,
                title: r.get(2)?,
                state: r.get(3)?,
                elapsed_ms: r.get::<_, i64>(4)? as u64,
                recovered: r.get(5)?,
                complete: r.get(6)?,
                warning: r.get(7)?,
                created_at: r.get(8)?,
                tracks: vec![],
            })
        })
        .map_err(db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_error)?;
    for recording in &mut recordings {
        let mut statement=db.prepare("SELECT role,device_id,device_name,filename,frames,sample_rate,channels FROM recording_tracks WHERE recording_id=?1 ORDER BY role").map_err(db_error)?;
        recording.tracks = statement
            .query_map([&recording.id], |r| {
                Ok(Track {
                    role: r.get(0)?,
                    device_id: r.get(1)?,
                    device_name: r.get(2)?,
                    filename: r.get(3)?,
                    frames: r.get::<_, i64>(4)? as u64,
                    sample_rate: r.get(5)?,
                    channels: r.get(6)?,
                })
            })
            .map_err(db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_error)?;
    }
    Ok(recordings)
}
fn register(db: &mut Connection, recording: &Recording) -> Result<()> {
    let tx = db.transaction().map_err(db_error)?;
    tx.execute(
        "INSERT INTO recordings(id,title,state,title_source) VALUES (?1,?2,'starting',?3)",
        params![recording.id, recording.title, recording.title_source],
    )
    .map_err(db_error)?;
    for track in &recording.tracks {
        tx.execute("INSERT INTO recording_tracks(recording_id,role,device_id,device_name,filename) VALUES (?1,?2,?3,?4,?5)",params![recording.id,track.role,track.device_id,track.device_name,track.filename]).map_err(db_error)?;
    }
    tx.commit().map_err(db_error)
}
fn checkpoint(db: &mut Connection, recording: &Recording) -> Result<()> {
    let tx = db.transaction().map_err(db_error)?;
    tx.execute(
        "UPDATE recordings SET state=?1,elapsed_ms=?2,warning=?3 WHERE id=?4",
        params![
            recording.state,
            recording.elapsed_ms as i64,
            recording.warning,
            recording.id
        ],
    )
    .map_err(db_error)?;
    for track in &recording.tracks {
        tx.execute(
            "UPDATE recording_tracks SET frames=?1 WHERE recording_id=?2 AND role=?3",
            params![track.frames as i64, recording.id, track.role],
        )
        .map_err(db_error)?;
    }
    tx.commit().map_err(db_error)
}
// Durable registration receipt: retrying after a lost response cannot create another meeting.
fn commit_result(db: &mut Connection, recording: &mut Recording) -> Result<()> {
    let tx = db.transaction().map_err(db_error)?;
    let (existing, finalized): (Option<i64>, bool) = tx
        .query_row(
            "SELECT meeting_id,finalized FROM recordings WHERE id=?1",
            [&recording.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(db_error)?;
    if finalized {
        recording.meeting_id = existing;
        crate::processing::enqueue(&tx, &recording.id)?;
        tx.commit().map_err(db_error)?;
        return Ok(());
    }
    if !recording.tracks.iter().any(|t| t.frames > 0) {
        return Err("No usable audio was captured. Any partial files were retained.".into());
    }
    let meeting = if let Some(id) = existing {
        id
    } else {
        let seconds = recording.elapsed_ms / 1000;
        let duration = format!("{:02}:{:02}", seconds / 60, seconds % 60);
        tx.execute("INSERT INTO meetings(title,date,duration,summary,highlights,decision,actions,speakers,title_source) VALUES (?1,date('now'),?2,'','[]','','[]','[]',?3)",params![recording.title,duration,recording.title_source]).map_err(db_error)?;
        tx.last_insert_rowid()
    };
    tx.execute("UPDATE recordings SET meeting_id=?1,state=?2,elapsed_ms=?3,recovered=?4,complete=?5,warning=?6,finalized=1 WHERE id=?7",params![meeting,recording.state,recording.elapsed_ms as i64,recording.recovered,recording.complete,recording.warning,recording.id]).map_err(db_error)?;
    for t in &recording.tracks {
        tx.execute(
            "UPDATE recording_tracks SET filename=?1,frames=?2 WHERE recording_id=?3 AND role=?4",
            params![t.filename, t.frames as i64, recording.id, t.role],
        )
        .map_err(db_error)?;
    }
    crate::processing::enqueue_finalized(
        &tx,
        &recording.id,
        // A WASAPI discontinuity marks audio as recovered even after a successful
        // Stop & Save. Only interrupted/failed captures must remain manual.
        recording.state == "saved",
    )?;
    tx.commit().map_err(db_error)?;
    recording.meeting_id = Some(meeting);
    Ok(())
}
fn mark_failed(db: &mut Connection, id: &str, message: &str) {
    let _ = db.execute(
        "UPDATE recordings SET state='failed',warning=?1 WHERE id=?2 AND finalized=0",
        params![message, id],
    );
}
fn write_marker(directory: &Path, recording: &Recording) -> Result<()> {
    let temporary = directory.join("finalized.json.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| "Unable to write recording finalization marker.")?;
    file.write_all(
        &serde_json::to_vec(recording).map_err(|_| "Unable to encode recording metadata.")?,
    )
    .map_err(|_| "Unable to write recording finalization marker.")?;
    file.sync_all()
        .map_err(|_| "Unable to synchronize recording metadata.")?;
    drop(file);
    std::fs::rename(temporary, directory.join("finalized.json"))
        .map_err(|_| String::from("Unable to finalize recording metadata."))
}
fn valid_id(id: &str) -> Result<()> {
    let parsed = uuid::Uuid::parse_str(id).map_err(|_| "Invalid recording identity.")?;
    if parsed.to_string() != id {
        return Err("Invalid recording identity.".into());
    }
    Ok(())
}
fn valid_filename(role: &str, filename: &str) -> bool {
    ["microphone", "system"].contains(&role)
        && [
            format!("{role}.wav"),
            format!("{role}.recovered.wav"),
            format!("{role}.wav.part"),
        ]
        .contains(&filename.to_string())
}
// Called only while holding the exclusive recorder ownership lock.
fn recover(db: &mut Connection, root: &Path) -> Result<()> {
    let pending: Vec<String> = {
        let mut query = db
            .prepare("SELECT id FROM recordings WHERE finalized=0")
            .map_err(db_error)?;
        let rows = query
            .query_map([], |r| r.get(0))
            .map_err(db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_error)?;
        rows
    };
    for id in pending {
        let result = (|| {
            valid_id(&id)?;
            let mut record = list(db)?
                .into_iter()
                .find(|r| r.id == id)
                .ok_or("Recording metadata is missing.")?;
            let directory = root.join(&id);
            let marker = directory.join("finalized.json");
            if marker.exists() {
                let file =
                    File::open(&marker).map_err(|_| "Unable to read finalization marker.")?;
                // Metadata is app-owned but still validate identities and relative paths.
                let saved: Recording =
                    serde_json::from_reader(std::io::Read::take(file, 64 * 1024))
                        .map_err(|_| "Finalization marker is invalid; files retained.")?;
                if saved.id != record.id
                    || saved.tracks.len() != record.tracks.len()
                    || !["saved", "failed"].contains(&saved.state.as_str())
                    || saved.elapsed_ms > 14_400_000
                    || !["manual", "system"].contains(&saved.title_source.as_str())
                    || saved.title.trim().is_empty()
                    || saved.title.chars().count() > 90
                    || saved.title.contains('\0')
                    || (saved.complete && (saved.recovered || saved.state != "saved"))
                {
                    return Err("Finalization marker does not match recording metadata.".into());
                }
                let mut roles = std::collections::BTreeSet::new();
                for t in &saved.tracks {
                    if !valid_filename(&t.role, &t.filename)
                        || t.filename.ends_with(".part")
                        || !record.tracks.iter().any(|old| old.role == t.role)
                        || !roles.insert(&t.role)
                        || t.sample_rate != wav::RATE
                        || t.channels != 2
                    {
                        return Err("Invalid finalized track reference.".into());
                    }
                    if wav::inspect(&directory.join(&t.filename))? != t.frames {
                        return Err("Finalized recording length does not match metadata.".into());
                    }
                }
                record = saved;
            } else {
                for t in &mut record.tracks {
                    if !valid_filename(&t.role, &t.filename) {
                        return Err("Invalid recording track reference.".into());
                    }
                    let finalized = directory.join(format!("{}.wav", t.role));
                    let part = directory.join(format!("{}.wav.part", t.role));
                    if finalized.exists() {
                        t.frames = wav::inspect(&finalized)?;
                        t.filename = format!("{}.wav", t.role);
                    } else if part.exists() {
                        t.filename = format!("{}.recovered.wav", t.role);
                        t.frames = wav::repair_copy(&part, &directory.join(&t.filename))?;
                    } else {
                        t.frames = 0;
                    }
                }
                record.elapsed_ms = record
                    .tracks
                    .iter()
                    .map(|t| t.frames * 1000 / t.sample_rate as u64)
                    .max()
                    .unwrap_or(0);
                record.recovered = true;
                record.complete = false;
                record.state = "failed".into();
                record.warning=Some("Recovered after interrupted capture/finalization. Sources may be incomplete; originals were retained. Review playback before future processing.".into());
                write_marker(&directory, &record)?;
            }
            commit_result(db, &mut record)
        })();
        if let Err(error) = result {
            mark_failed(db, &id, &error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
