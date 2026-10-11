import type { Meeting, Workspace } from "../data/types";

export function MeetingRow({ meeting, workspace, onOpen }: { meeting: Meeting; workspace?: Workspace; onOpen: (id: number) => void }) {
  return <button className="row" onClick={() => onOpen(meeting.id)}>
    <span className={`badge ${workspace?.color || "sage"}`} aria-hidden="true">▤</span>
    <span className="row-body"><span className="row-title">{meeting.title}</span><small>{workspace?.name || "Unassigned"} · {meeting.date} · {meeting.duration}</small></span>
    <span aria-hidden="true">↗</span>
  </button>;
}
