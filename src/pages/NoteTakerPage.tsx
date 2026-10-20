import { intelligenceLabel, meetingNotesReady } from "../data/intelligence";
import { processingLabel, useProcessing, type ProcessingJob } from "../data/processing";
import type { Meeting, Workspace } from "../data/types";

function cardStatus(meeting: Meeting, job: ProcessingJob | undefined, notesReady: boolean, generation: { meeting_id: number; state: string }[]) {
  if (!meeting.recorded) return { text: "✓ Ready", kind: "sage" };
  if (notesReady) return { text: "Notes ready", kind: "notes-ready" };
  const latest = generation.filter(item => item.meeting_id === meeting.id).at(-1);
  if (latest && ["queued", "processing", "retryable", "failed"].includes(latest.state)) {
    return { text: intelligenceLabel(latest.state), kind: "lav" };
  }
  const label = processingLabel(job);
  return label ? { text: label, kind: "sage" } : null;
}

export function NoteTakerPage({ meetings, workspaces, query, filter, onQuery, onFilter, onRecord, onMeeting }: {
  meetings: Meeting[]; workspaces: Workspace[]; query: string; filter: string;
  onQuery: (value: string) => void; onFilter: (value: string) => void; onRecord: () => void; onMeeting: (id: number) => void;
}) {
  const processing = useProcessing();
  const matches = meetings.filter(meeting => (filter === "all" || meeting.project === Number(filter)) &&
    `${meeting.title} ${meeting.summary} ${processing.view?.intelligence.previews.find(p => p.meeting_id === meeting.id)?.summary || ""} ${processing.view?.intelligence.previews.find(p => p.meeting_id === meeting.id)?.topics || ""} ${workspaces.find(workspace => workspace.id === meeting.project)?.name || "Unassigned"}`.toLowerCase().includes(query.toLowerCase()));
  return <div className="notetaker-page">
    <div className="heading"><div><span className="eyebrow">EVERY CONVERSATION, CONSIDERED</span><h1 className="heading-title">Your meeting notebook.</h1><p>The details you need. The headspace you deserve.</p></div><button className="dark" onClick={onRecord}>＋ New recording</button></div>
    <div className="toolbar"><input className="search" placeholder="Search conversations, topics, or workspaces…" aria-label="Search meetings" value={query} onChange={event => onQuery(event.target.value)} />
      <select className="select" aria-label="Workspace filter" value={filter} onChange={event => onFilter(event.target.value)}><option value="all">All workspaces</option><option value="0">Unassigned</option>{workspaces.map(workspace => <option key={workspace.id} value={workspace.id}>{workspace.name}</option>)}</select><span className="pill">Newest first ↓</span>
    </div>
    <div className="section"><small aria-live="polite">{matches.length} conversations · Your thinking, collected</small><span className="eyebrow">LIBRARY</span></div>
    <div className="library">{matches.map(meeting => {
      const workspace = workspaces.find(item => item.id === meeting.project);
      const job = processing.view?.jobs.find(item => item.meeting_id === meeting.id);
      const notesReady = meetingNotesReady(meeting.id, processing.view?.jobs ?? [], processing.view?.intelligence.jobs ?? []);
      const status = cardStatus(meeting, job, notesReady, processing.view?.intelligence.jobs ?? []);
      return <button className="card" key={meeting.id} onClick={() => onMeeting(meeting.id)}>
        <span className="card-top">
          <span className={`tag ${workspace?.color || ""}`}>{workspace?.name || "Unassigned"}</span>
          {status && <span className={`tag ${status.kind}`}>{status.text}</span>}
        </span>
        <span className="card-title">{meeting.title}</span>
        <span className="card-meta"><span>{meeting.date} · {meeting.duration}</span><span aria-hidden="true">↗</span></span>
      </button>;
    })}</div>
    {!matches.length && <div className="empty">{meetings.length ? "No matching conversations. Try another search or workspace." : "No conversations yet. Start a recording to save your first meeting."}</div>}
  </div>;
}
