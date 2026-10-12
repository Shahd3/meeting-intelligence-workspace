import { intelligenceLabel, meetingNotesReady } from "../data/intelligence";
import { processingLabel, useProcessing } from "../data/processing";
import type { Meeting, Workspace } from "../data/types";

export function MeetingRow({ meeting, workspace, onOpen }: { meeting: Meeting; workspace?: Workspace; onOpen: (id: number) => void }) {
  const processing = useProcessing();
  return <button className="row" onClick={() => onOpen(meeting.id)}>
    <span className={`badge ${workspace?.color || "sage"}`} aria-hidden="true">▤</span>
    <span className="row-body"><span className="row-title">{meeting.title}</span><small>{workspace?.name || "Unassigned"} · {meeting.date} · {meeting.duration}</small>{meeting.recorded && processingLabel(processing.view?.jobs.find(job => job.meeting_id === meeting.id)) && <small>{processingLabel(processing.view?.jobs.find(job => job.meeting_id === meeting.id))}</small>}</span>{meeting.recorded && processing.view && (processing.view.intelligence.jobs.filter(job => job.meeting_id === meeting.id).at(-1)?.state !== "completed" && processing.view.intelligence.jobs.some(job => job.meeting_id === meeting.id) || meetingNotesReady(meeting.id, processing.view.jobs, processing.view.intelligence.jobs)) && <span className={`tag meeting-status ${meetingNotesReady(meeting.id, processing.view.jobs, processing.view.intelligence.jobs) ? "notes-ready" : "lav"}`}>{intelligenceLabel(processing.view.intelligence.jobs.filter(job => job.meeting_id === meeting.id).at(-1)!.state)}</span>}
    <span aria-hidden="true">↗</span>
  </button>;
}
