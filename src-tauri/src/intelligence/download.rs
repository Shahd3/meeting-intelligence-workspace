//! Deterministic Markdown from persisted intelligence; no provider call or frontend document payload.
use super::{
    contract::{Notes, Point},
    db_error,
};
use rusqlite::{Connection, OptionalExtension};
use std::{fs::OpenOptions, io::Write, path::Path};
fn clean(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn points(items: &[Point]) -> String {
    if items.is_empty() {
        "None documented.".into()
    } else {
        items
            .iter()
            .map(|p| format!("- {}", clean(&p.text)))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
pub(super) fn render(
    title: &str,
    date: &str,
    duration: &str,
    n: &Notes,
    attendees: Option<Vec<String>>,
) -> String {
    let names = attendees.unwrap_or_else(|| {
        n.attendees
            .iter()
            .map(|a| {
                a.role
                    .as_ref()
                    .map(|r| format!("{} ({r})", a.name))
                    .unwrap_or_else(|| a.name.clone())
            })
            .collect()
    });
    let mut parts = vec![
        format!("# {}", clean(title)),
        "AI-generated draft · Minutes of Meeting".into(),
        format!("{} · {}", clean(date), clean(duration)),
        "## Attendees".into(),
        if names.is_empty() {
            "Attendance not established.".into()
        } else {
            names
                .iter()
                .map(|s| clean(s))
                .collect::<Vec<_>>()
                .join(", ")
        },
        "## Executive summary".into(),
        clean(&n.executive_summary.text),
        "## Discussion".into(),
    ];
    for t in &n.topics {
        parts.push(format!("### {}\n{}", clean(&t.heading), points(&t.points)))
    }
    parts.extend([
        "## Decisions".into(),
        points(&n.decisions),
        "## Action items".into(),
    ]);
    parts.push(if n.action_items.is_empty() {
        "No actions documented.".into()
    } else {
        format!(
            "| Action | Owner | Deadline |\n| --- | --- | --- |\n{}",
            n.action_items
                .iter()
                .map(|a| format!(
                    "| {} | {} | {} |",
                    clean(&a.description.text),
                    clean(a.owner.as_deref().unwrap_or("Not assigned")),
                    clean(a.deadline.as_deref().unwrap_or("Not stated"))
                ))
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    parts.extend([
        "## Follow-ups".into(),
        points(&n.follow_ups),
        "## Open questions and unresolved proposals".into(),
        points(&n.open_questions),
    ]);
    parts.join("\n\n")
}
pub(super) fn save(
    directory: &Path,
    title: &str,
    date: &str,
    contents: &str,
) -> Result<String, String> {
    let slug: String = title
        .chars()
        .take(60)
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let date: String = date
        .chars()
        .take(20)
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    let base = format!(
        "meeting-{}-{}-minutes",
        if slug.trim_matches('-').is_empty() {
            "notes"
        } else {
            slug.trim_matches('-')
        },
        if date.is_empty() { "undated" } else { &date }
    );
    for i in 0..10000 {
        let name = if i == 0 {
            format!("{base}.md")
        } else {
            format!("{base}-{}.md", i + 1)
        };
        let path = directory.join(&name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if file
                    .write_all(contents.as_bytes())
                    .and_then(|_| file.sync_all())
                    .is_err()
                {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err("Unable to save Minutes to Downloads.".into());
                }
                return Ok(name);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => {
                return Err("Unable to access Downloads. No existing file was overwritten.".into())
            }
        }
    }
    Err("Too many duplicate Minutes filenames in Downloads.".into())
}
pub fn meeting(db: &mut Connection, id: i64, directory: &Path) -> Result<String, String> {
    let tx = db.transaction().map_err(db_error)?;
    let latest:Option<(String,bool)>=tx.query_row("SELECT j.state,EXISTS(SELECT 1 FROM meeting_intelligence r JOIN meeting_intelligence_selection s ON s.result_id=r.id WHERE r.id=j.id AND s.meeting_id=j.meeting_id) FROM intelligence_jobs j WHERE j.meeting_id=?1 ORDER BY j.rowid DESC LIMIT 1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    let pending:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM processing_jobs WHERE meeting_id=?1 AND state!='completed') OR EXISTS(SELECT 1 FROM intelligence_jobs WHERE meeting_id=?1 AND state IN ('queued','processing','retryable'))",[id],|r|r.get(0)).map_err(db_error)?;
    if pending || !matches!(latest,Some((ref state,true)) if state=="completed") {
        return Err("Meeting notes are not ready. Wait for processing to complete.".into());
    }
    let (title,date,duration,json):(String,String,String,String)=tx.query_row("SELECT m.title,m.date,m.duration,i.result FROM meetings m JOIN meeting_intelligence_selection s ON s.meeting_id=m.id JOIN meeting_intelligence i ON i.id=s.result_id WHERE m.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(db_error)?;
    let notes: Notes =
        serde_json::from_str(&json).map_err(|_| "Saved meeting intelligence is invalid.")?;
    let names: Option<String> = tx
        .query_row(
            "SELECT names FROM intelligence_attendee_overrides WHERE meeting_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let names = names
        .map(|n| {
            serde_json::from_str::<Vec<String>>(&n).map_err(|_| "Saved attendees are invalid.")
        })
        .transpose()?;
    let content = render(&title, &date, &duration, &notes, names);
    tx.commit().map_err(db_error)?;
    save(directory, &title, &date, &content)
}
