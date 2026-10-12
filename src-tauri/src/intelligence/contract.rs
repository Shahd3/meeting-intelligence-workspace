use crate::processing::{provider::Failure, Segment};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub segment_id: String,
    pub quote: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub text: String,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topic {
    pub heading: String,
    pub points: Vec<Point>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub description: Point,
    pub owner: Option<String>,
    pub owner_evidence: Vec<Evidence>,
    pub deadline: Option<String>,
    pub deadline_evidence: Vec<Evidence>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attendee {
    pub name: String,
    pub role: Option<String>,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notes {
    pub schema_version: u32,
    pub suggested_title: Point,
    pub executive_summary: Point,
    pub topics: Vec<Topic>,
    pub decisions: Vec<Point>,
    pub action_items: Vec<Action>,
    pub follow_ups: Vec<Point>,
    pub open_questions: Vec<Point>,
    pub attendees: Vec<Attendee>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EvidenceSegment {
    pub id: String,
    #[serde(flatten)]
    pub segment: Segment,
}
pub fn evidence_segments(id: &str, segments: Vec<Segment>) -> Vec<EvidenceSegment> {
    segments
        .into_iter()
        .enumerate()
        .map(|(i, segment)| EvidenceSegment {
            id: format!("{id}:{i}"),
            segment,
        })
        .collect()
}
fn invalid() -> Failure {
    Failure::input("AI output failed structured evidence validation. Transcript and previous results were retained.")
}
pub fn validate(notes: &Notes, source: &[EvidenceSegment]) -> Result<(), Failure> {
    if notes.schema_version != 1
        || notes.topics.len() > 100
        || notes.action_items.len() > 200
        || notes.attendees.len() > 100
    {
        return Err(invalid());
    }
    let index: BTreeMap<_, _> = source
        .iter()
        .map(|s| (s.id.as_str(), s.segment.text.as_str()))
        .collect();
    let evidence = |items: &[Evidence]| -> Result<(), Failure> {
        if items.len() > 100 {
            return Err(invalid());
        }
        for e in items {
            if e.quote.trim().is_empty()
                || e.quote.len() > 2000
                || !index
                    .get(e.segment_id.as_str())
                    .map(|text| text.contains(&e.quote))
                    .unwrap_or(false)
            {
                return Err(invalid());
            }
        }
        Ok(())
    };
    let point = |p: &Point| -> Result<(), Failure> {
        if p.text.len() > 8000 || (!p.text.trim().is_empty() && p.evidence.is_empty()) {
            return Err(invalid());
        }
        evidence(&p.evidence)
    };
    point(&notes.suggested_title)?;
    if notes.suggested_title.text.chars().count() > 90 {
        return Err(invalid());
    }
    point(&notes.executive_summary)?;
    for t in &notes.topics {
        if t.heading.trim().is_empty() || t.heading.len() > 300 || t.points.len() > 200 {
            return Err(invalid());
        }
        for p in &t.points {
            point(p)?
        }
    }
    for p in notes
        .decisions
        .iter()
        .chain(&notes.follow_ups)
        .chain(&notes.open_questions)
    {
        point(p)?
    }
    for a in &notes.action_items {
        point(&a.description)?;
        evidence(&a.owner_evidence)?;
        evidence(&a.deadline_evidence)?;
        for (value, ev) in [
            (&a.owner, &a.owner_evidence),
            (&a.deadline, &a.deadline_evidence),
        ] {
            if let Some(v) = value {
                if v.trim().is_empty()
                    || v.len() > 300
                    || ev.is_empty()
                    || !ev.iter().any(|e| e.quote.contains(v))
                {
                    return Err(invalid());
                }
            } else if !ev.is_empty() {
                return Err(invalid());
            }
        }
    }
    for a in &notes.attendees {
        if a.name.trim().is_empty()
            || a.name.len() > 150
            || a.evidence.is_empty()
            || !a.evidence.iter().any(|e| e.quote.contains(&a.name))
        {
            return Err(invalid());
        }
        evidence(&a.evidence)?;
        if let Some(role) = &a.role {
            if role.is_empty() || !a.evidence.iter().any(|e| e.quote.contains(role)) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
pub fn schema() -> serde_json::Value {
    use serde_json::json;
    let ev = json!({"type":"array","items":{"type":"object","additionalProperties":false,"required":["segment_id","quote"],"properties":{"segment_id":{"type":"string"},"quote":{"type":"string"}}}});
    let point = json!({"type":"object","additionalProperties":false,"required":["text","evidence"],"properties":{"text":{"type":"string"},"evidence":ev}});
    let points = json!({"type":"array","items":point});
    json!({"type":"object","additionalProperties":false,"required":["schema_version","suggested_title","executive_summary","topics","decisions","action_items","follow_ups","open_questions","attendees"],"properties":{
    "schema_version":{"type":"integer","enum":[1]},"suggested_title":point,"executive_summary":point,
    "topics":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["heading","points"],"properties":{"heading":{"type":"string"},"points":points}}},
    "decisions":points,"follow_ups":points,"open_questions":points,
    "action_items":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["description","owner","owner_evidence","deadline","deadline_evidence"],"properties":{"description":point,"owner":{"type":["string","null"]},"owner_evidence":ev,"deadline":{"type":["string","null"]},"deadline_evidence":ev}}},
    "attendees":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["name","role","evidence"],"properties":{"name":{"type":"string"},"role":{"type":["string","null"]},"evidence":ev}}}
    }})
}
