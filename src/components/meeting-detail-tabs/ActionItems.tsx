import type { Completion, Meeting, Workspace } from "../../data/types";
import { IntelligenceContent } from "../MeetingIntelligence";

type Props = {
  meeting: Meeting;
  workspace?: Workspace;
  completed: Completion;
  onComplete: (key: string, value: boolean) => void;
  onEvidence: (id: string) => void;
};

export function ActionItems({ meeting, workspace, completed, onComplete, onEvidence }: Props) {
  if (meeting.recorded) {
    return <IntelligenceContent meeting={meeting} tab="Action items" onEvidence={onEvidence} />;
  }
  return <>{meeting.actions.map((action, index) => {
    const key = `${meeting.id}-${index}`;
    return <div className="task" key={key}>
      <input type="checkbox" id={`action-${key}`} checked={!!completed[key]}
        onChange={event => onComplete(key, event.target.checked)} />
      <label htmlFor={`action-${key}`}>
        {action}<small>{workspace?.owner || "Shahd"} · Due {workspace?.due || "Oct 14"}</small>
      </label>
      <span className="tag">{completed[key] ? "Done" : "To do"}</span>
    </div>;
  })}</>;
}
