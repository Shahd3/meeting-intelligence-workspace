import { MeetingRow } from "../components/MeetingRow";
import { WorkspaceTask } from "../components/WorkspaceTask";
import { MeetingTasks } from "../components/MeetingTasks";
import type { Completion, Meeting, Page, Workspace } from "../data/types";

export function HomePage({ meetings, workspaces, completed, onComplete, onRecord, onMeeting, onWorkspace, onNavigate }: {
  meetings: Meeting[]; workspaces: Workspace[]; completed: Completion;
  onComplete: (key: string, value: boolean) => void; onRecord: () => void; onMeeting: (id: number) => void;
  onWorkspace: (id: number) => void; onNavigate: (page: Page) => void;
}) {
  const decision = meetings.find(meeting => meeting.decision.trim());
  return <>
    <div className="heading"><div><h1>Good morning, Shahd<span className="greeting-dot">.</span></h1><p>Make room for the conversation. We’ll keep the clarity.</p></div><span className="eyebrow">SATURDAY / 10 OCTOBER</span></div>
    <div className="grid"><div>
      <section className="panel hero"><span className="eyebrow">FROM CONVERSATION TO CLARITY</span><h2>Good ideas deserve<br />a place to land.</h2><p>Capture the moment. Connect the dots.<br />Let your next meeting move things forward.</p><button className="dark" onClick={onRecord}>● &nbsp; Start recording &nbsp; ↗</button><div className="art" aria-hidden="true">✳</div></section>
      <div className="stats">
        <div className="stat"><span>YOUR CONVERSATIONS</span><strong>{String(meetings.length).padStart(2, "0")}</strong><small>Meetings captured</small></div>
        <div className="stat peach"><span>SHARED CONTEXT</span><strong>{String(workspaces.length).padStart(2, "0")}</strong><small>Active workspaces</small></div>
        <div className="stat lav"><span>WHAT’S NEXT</span><strong>{String(workspaces.filter(workspace => workspace.task && !completed[`workspace-${workspace.id}`]).length + meetings.reduce((count, meeting) => count + meeting.actions.filter((_, index) => !completed[`${meeting.id}-${index}`]).length, 0)).padStart(2, "0")}</strong><small>Open action items</small></div>
      </div>
      <section className="panel"><div className="section"><h2>Recent conversations</h2><button className="text" onClick={() => onNavigate("notes")}>View all ↗</button></div><div className="recent-list">{!meetings.length && <p className="muted">No conversations yet. Start a recording to begin.</p>}{meetings.slice(0, 3).map(meeting => <MeetingRow key={meeting.id} meeting={meeting} workspace={workspaces.find(workspace => workspace.id === meeting.project)} onOpen={onMeeting} />)}</div></section>
    </div><div>
      <section className="panel"><div className="section"><h2>A little forward motion</h2><span className="tag sage">Next steps</span></div><div className="task-list">{workspaces.filter(workspace => workspace.task).slice(0, 3).map(workspace => <WorkspaceTask key={workspace.id} workspace={workspace} checked={!!completed[`workspace-${workspace.id}`]} onChange={onComplete} />)}<MeetingTasks meetings={meetings} workspaces={workspaces} completed={completed} onComplete={onComplete} limit={3} /></div></section>
      <section className="panel insight"><span className="eyebrow">A DECISION WORTH REMEMBERING &nbsp; ✦</span><p>{decision ? `“${decision.decision}”` : "Your next conversation starts here."}</p>{decision && <div className="section flush"><small>{workspaces.find(item => item.id === decision.project)?.name || "Unassigned"} · {decision.date}</small><button className="circle" onClick={() => onMeeting(decision.id)} aria-label="Open decision">↗</button></div>}</section>
      <div className="section workspace-heading"><h2>Your workspaces</h2><button className="text" onClick={() => onNavigate("spaces")}>Explore ↗</button></div>
      <div className="mini">{workspaces.slice(0, 2).map(workspace => <button className={workspace.color} key={workspace.id} onClick={() => onWorkspace(workspace.id)}><span className="mini-symbol" aria-hidden="true">{workspace.symbol}</span><h3>{workspace.name}</h3><small>{meetings.filter(meeting => meeting.project === workspace.id).length} conversations ↗</small></button>)}</div>
    </div></div>
  </>;
}
