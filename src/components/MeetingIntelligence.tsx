import { useState } from "react";
import { useProcessing } from "../data/processing";
import { direction, intelligenceLabel, meaningfulText, meaningfulPoints, type Evidence, type Point } from "../data/intelligence";
import type { Meeting, MeetingTab } from "../data/types";
export function IntelligenceControls({ meetingId }: { meetingId: number }) {
 const data = useProcessing(); const view = data.view?.intelligence; const provider = data.view?.ai.summarization;
 const [approved, setApproved] = useState(false); const [selected, setSelected] = useState("");
 if (!view || data.view?.transcript_meeting_id !== meetingId) return <p role="status">Loading meeting intelligence…</p>;
 const choices = view.transcripts; const transcript = choices.find(t => t.id === selected) || choices.find(t => t.id === view.result?.transcript_id) || choices[0];
 const jobs = view.jobs.filter(j => j.meeting_id === meetingId); const active = jobs.some(j => ["queued", "processing", "retryable"].includes(j.state));
 const latest = jobs[jobs.length - 1];

 return <div className="intelligence-controls">
  {latest && latest.state !== "completed" && <div className="transcription-status" role="status"><span className="tag lav">{intelligenceLabel(latest.state)}</span>{latest.state === "processing" && <small> {latest.stage.replaceAll("_", " ")}</small>}{latest.state === "retryable" && latest.next_attempt_at && <small> Next attempt: {new Date(latest.next_attempt_at * 1000).toLocaleTimeString()}</small>}{latest.error && <p>{latest.error}</p>}{latest.state === "failed" && <button className="pill" disabled={data.busy || latest.attempts >= 5 || latest.requests >= 64} onClick={() => void data.command("intelligence_request", { request: { kind: "retry", id: latest.id } })}>Retry AI analysis</button>}</div>}
  {view.worker_error && <p role="alert">{view.worker_error}</p>}
  {!active && choices.length > 0 && !view.result && <>
   {!provider ? <p className="muted">Configure and authorize a summarization provider in Recording Preferences → AI Processing. Your transcript is already saved.</p> : <>
    {choices.length > 1 && <label>Source transcript<select className="select" value={transcript?.id || ""} onChange={event => setSelected(event.target.value)}>{choices.map((t,i) => <option key={t.id} value={t.id}>Recording transcript {i + 1} · {t.segment_count} segments</option>)}</select><small>Recordings are analyzed separately; existing transcripts are not silently combined.</small></label>}
    {!provider.automatic_generation && <label className="ai-consent"><input type="checkbox" checked={approved} onChange={event => setApproved(event.target.checked)} />I authorize sharing this transcript with {provider.remote?.base_url} using {provider.remote?.model}, including bounded retries. Charges may apply.</label>}
    <button className="pill" disabled={data.busy || !provider.key_saved || (!approved && !provider.automatic_generation)} onClick={() => void data.command("intelligence_request", { request: { kind: "generate", transcript_id: transcript?.id, revision: provider.revision, authorized: approved, regenerate: latest?.state === "failed" } }).then(ok => { if (ok) setApproved(false); })}>{latest?.state === "failed" ? "Restart analysis with saved settings" : "Generate meeting notes"}</button>
   </>}
  </>}
  {!choices.length && <p className="muted">Meeting intelligence follows completed transcription.</p>}
  {data.error && <p role="alert">{data.error}</p>}
 </div>;
}
export function EvidenceLinks({ evidence, onEvidence }: { evidence: Evidence[]; onEvidence: (id: string) => void }) {
 return <span className="evidence-links">{evidence.filter((e,i,all) => all.findIndex(other => other.segment_id === e.segment_id) === i).map((e,i) => <button className="text" title={e.quote} key={e.segment_id} onClick={() => onEvidence(e.segment_id)}>Source {i + 1}</button>)}</span>;
}
function Fact({ point }: { point: Point }) { return <div className="intelligence-fact"><p className="prose">{point.text}</p></div>; }
export function attendeeInitials(name: string): string {
 const clean = name.replace(/\(.*\)/g, "").trim();
 const parts = clean.split(/\s+/).filter(Boolean);
 if (!parts.length) return "?";
 if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
 return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}
export function IntelligenceContent({ meeting, tab, onEvidence }: { meeting: Meeting; tab: MeetingTab; onEvidence: (id: string) => void }) {
 const data = useProcessing(); const view = data.view?.intelligence; const result = view?.result;
 const [editing, setEditing] = useState(false); const [names, setNames] = useState("");
 if (!view || data.view?.transcript_meeting_id !== meeting.id) return <p role="status">Loading saved meeting notes…</p>;
 if (!result) return <p className="muted">{view.jobs.some(j=>j.meeting_id===meeting.id && ["queued","processing","retryable"].includes(j.state)) ? "Meeting notes will appear here when analysis finishes. You can leave this screen." : "No meeting intelligence is available yet. The original transcript and audio remain available."}</p>;
 const n = result.notes;
 const attendees = (view.attendees_override ?? n.attendees.filter(a=>meaningfulText(a.name)).map(a => a.role ? `${a.name} (${a.role})` : a.name)).filter(meaningfulText);
 const topics = n.topics.map(t=>({...t,points:meaningfulPoints(t.points)})).filter(t=>t.points.length);
 const decisions=meaningfulPoints(n.decisions), questions=meaningfulPoints(n.open_questions), followups=meaningfulPoints(n.follow_ups);
 const actions=n.action_items.map((action,ordinal)=>({action,ordinal})).filter(({action})=>meaningfulText(action.description.text));
 const dir = direction(n.executive_summary.text + n.topics.map(t=>t.heading).join(" "));
 const bulletList = (points: Point[]) => <ul className="points-list">{points.map((p, i) => <li key={i}><p className="prose">{p.text}</p></li>)}</ul>;
 return <div className="meeting-intelligence" dir={dir}>
  {view.jobs.some(j=>j.meeting_id===meeting.id && ["queued","processing","retryable"].includes(j.state)) && <p role="status">Showing the previous successful result while new notes are prepared.</p>}
  {result.coverage.source_warning && <p className="recording-warning">{result.coverage.source_warning}</p>}
  {tab === "Summary" && <>
   {meaningfulText(n.executive_summary.text) && <Fact point={n.executive_summary}/>}
   {!meaningfulText(n.executive_summary.text) && !attendees.length && !topics.length && !decisions.length && !questions.length && !followups.length && <p className="muted">No substantive summary was identified.</p>}
   {attendees.length > 0 && <details className="attendee-section" open><summary>Attendees <small>{attendees.length}</small></summary><div className="avatar-stack">{attendees.slice(0, 6).map(name=><span className="attendee-avatar" key={name} title={name}>{attendeeInitials(name)}</span>)}{attendees.length > 6 && <span className="attendee-avatar more" title={`+${attendees.length - 6} more`}>+{attendees.length - 6}</span>}</div></details>}
   <button className="text" onClick={() => { setEditing(!editing); setNames(attendees.join("\n")); }}>Correct attendees</button>
   {editing && <form onSubmit={event=>{event.preventDefault();void data.command("intelligence_request",{request:{kind:"attendees",meeting_id:meeting.id,names:names.split("\n").map(s=>s.trim()).filter(Boolean)}}).then(ok=>{if(ok)setEditing(false)});}}><label className="field">Confirmed attendees (one per line)<textarea value={names} onChange={event=>setNames(event.target.value)} rows={4}/></label><button className="pill" disabled={data.busy}>Save confirmed attendees</button><button type="button" className="text" onClick={()=>setEditing(false)}>Cancel</button></form>}
   {view.attendees_override !== null && <><small>User-confirmed attendee list.</small><button className="text" disabled={data.busy} onClick={()=>void data.command("intelligence_request",{request:{kind:"attendees",meeting_id:meeting.id,names:null}})}>Use evidence-backed attendees</button></>}
   {topics.length > 0 && <details className="subsection sage" open><summary>Discussion overview</summary>{bulletList(topics.flatMap(t => t.points))}</details>}
  </>}
  {tab === "Summary" && <>{decisions.length > 0 && <details className="subsection sage" open><summary>Confirmed decisions</summary>{bulletList(decisions)}</details>}{questions.length > 0 && <details className="subsection sage" open><summary>Open questions and unresolved proposals</summary>{bulletList(questions)}</details>}{followups.length > 0 && <details className="subsection sage" open><summary>Follow-ups</summary>{bulletList(followups)}</details>}</>}
  {tab === "Action items" && (actions.length ? actions.map(({action:a,ordinal:i})=>{const state=result.actions.find(s=>s.ordinal===i);return <div className="task intelligence-action" key={state?.id || i}><input type="checkbox" id={`intelligence-action-${state?.id}`} checked={state?.done || false} disabled={data.busy || !state} onChange={event=>void data.command("intelligence_request",{request:{kind:"complete_action",id:state?.id,done:event.target.checked}})}/><div><label htmlFor={`intelligence-action-${state?.id}`}>{a.description.text}</label><small>{meaningfulText(a.owner) ? a.owner : "Unassigned"}{meaningfulText(a.deadline) ? ` · ${a.deadline}` : ""}</small><EvidenceLinks evidence={[...a.description.evidence,...a.owner_evidence,...a.deadline_evidence]} onEvidence={onEvidence}/></div><span className="tag">{state?.done ? "Done" : "To do"}</span></div>}) : <p className="muted">No explicit action items were documented.</p>)}
 </div>;
}
