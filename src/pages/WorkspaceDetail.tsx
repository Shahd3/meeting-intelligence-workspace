import { DeleteButton } from "../components/Icons";
import { MeetingRow } from "../components/MeetingRow";
import { ProjectAssistant } from "../components/ProjectAssistant";
import { WorkspaceTask } from "../components/WorkspaceTask";
import { MeetingTasks } from "../components/MeetingTasks";
import type { ChatMessage, Completion, Meeting, Workspace } from "../data/types";

export function WorkspaceDetail({ workspace, meetings, completed, messages, onBack, onMeeting, onComplete, onAsk, onEdit, onDelete }: {
  workspace: Workspace; meetings: Meeting[]; completed: Completion; messages: ChatMessage[];
  onBack: () => void; onMeeting: (id: number) => void; onComplete: (key: string, value: boolean) => void; onAsk: (question: string) => void;
  onEdit: () => void; onDelete: () => void;
}) {
  return <>
    <div className="detail-heading"><button className="circle" onClick={onBack} aria-label="Back to workspaces">←</button><span className={`badge workspace-badge ${workspace.color}`} aria-hidden="true">{workspace.symbol}</span><div><span className="eyebrow">PROJECT WORKSPACE / {meetings.length} CONVERSATIONS</span><h1>{workspace.name}</h1></div></div>
    <div className="record-actions"><button className="pill" onClick={onEdit}>Edit workspace</button><DeleteButton label="Delete workspace" onClick={onDelete}/></div>
    <div className="project-layout"><div>
      <section className="panel"><span className="eyebrow">THE BIGGER PICTURE</span><h2 className="overview-title">Where things stand.</h2><p className="prose overview-text">{workspace.overview}</p><div className={`callout ${workspace.color}`}><span className="eyebrow">COLLECTIVE SUMMARY</span><p className="prose">{meetings.length ? meetings.map(meeting => meeting.summary).join(" ") : "Ready for your first conversation. Assign a meeting from your notebook to begin."}</p></div><h3>Important decisions</h3>{meetings.length ? meetings.map(meeting => <div key={meeting.id}><p className="prose spaced">{meeting.decision}</p><button className="text" onClick={() => onMeeting(meeting.id)}>{meeting.title} ↗</button></div>) : <p className="muted spaced">No decisions captured yet.</p>}</section>
      <section className="panel stacked-panel"><div className="section"><h2>Next steps</h2><span className="tag">Action items</span></div>{workspace.task && <WorkspaceTask workspace={workspace} checked={!!completed[`workspace-${workspace.id}`]} onChange={onComplete} />}<MeetingTasks meetings={meetings} workspaces={[workspace]} completed={completed} onComplete={onComplete} /></section>
      <section className="panel stacked-panel"><div className="section"><h2>Meeting history</h2><small>{meetings.length} collected</small></div>{meetings.length ? meetings.map(meeting => <MeetingRow key={meeting.id} meeting={meeting} workspace={workspace} onOpen={onMeeting} />) : <p className="muted">No meetings assigned yet.</p>}</section>
    </div><ProjectAssistant key={workspace.id} workspace={workspace} meetings={meetings} messages={messages} onAsk={onAsk} onMeeting={onMeeting} /></div>
  </>;
}
