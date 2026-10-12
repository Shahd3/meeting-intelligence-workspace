import { intelligenceLabel } from "../data/intelligence";
import { processingLabel, useProcessing } from "../data/processing";
import type { Meeting, Workspace } from "../data/types";

export function MeetingRow({ meeting, workspace, onOpen }: { meeting: Meeting; workspace?: Workspace; onOpen: (id: number) => void }) {
  const processing = useProcessing();
  return <button className="row" onClick={() => onOpen(meeting.id)}>
    <span className={`badge ${workspace?.color || "sage"}`} aria-hidden="true">▤</span>
    <span className="row-body"><span className="row-title">{meeting.title}</span><small>{workspace?.name || "Unassigned"} · {meeting.date} · {meeting.duration}</small>{meeting.recorded && <small>{processingLabel(processing.view?.jobs.find(job => job.meeting_id === meeting.id))}</small>}{meeting.recorded && processing.view?.intelligence.jobs.some(job => job.meeting_id === meeting.id) && <small>{intelligenceLabel(processing.view.intelligence.jobs.filter(job => job.meeting_id === meeting.id).at(-1)!.state)}</small>}</span>
    <span aria-hidden="true">↗</span>
  </button>;
}
