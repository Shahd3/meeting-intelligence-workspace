import type { Completion, Meeting, Workspace } from "../data/types";

export function MeetingTasks({ meetings, workspaces, completed, onComplete, limit }: {
  meetings: Meeting[]; workspaces: Workspace[]; completed: Completion;
  onComplete: (key: string, value: boolean) => void; limit?: number;
}) {
  const tasks = meetings.flatMap(meeting => meeting.actions.map((action, index) => ({ meeting, action, key: `${meeting.id}-${index}` })));
  if (!tasks.length) return <p className="muted">No action items captured yet.</p>;
  return <>{tasks.slice(0, limit).map(({ meeting, action, key }) => <div className="task" key={key}>
    <input type="checkbox" id={`list-action-${key}`} checked={!!completed[key]} onChange={event => onComplete(key, event.target.checked)} />
    <label htmlFor={`list-action-${key}`}>{action}<small>{workspaces.find(workspace => workspace.id === meeting.project)?.name || "Unassigned"} · {meeting.title}</small></label>
    <span className="tag">{completed[key] ? "Done" : "To do"}</span>
  </div>)}</>;
}
