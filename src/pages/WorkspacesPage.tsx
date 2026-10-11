import type { Meeting, Workspace } from "../data/types";

export function WorkspacesPage({ meetings, workspaces, onCreate, onWorkspace }: {
  meetings: Meeting[]; workspaces: Workspace[]; onCreate: () => void; onWorkspace: (id: number) => void;
}) {
  return <>
    <div className="heading"><div><span className="eyebrow">A HOME FOR THE BIGGER PICTURE</span><h1 className="heading-title">Ideas, in good company.</h1><p>Bring conversations together. See what’s taking shape.</p></div><button className="dark" onClick={onCreate}>＋ Create workspace</button></div>
    <div className="section workspace-list-heading"><h2>Your workspaces <small>({workspaces.length})</small></h2><span className="eyebrow">MADE FOR CONNECTION</span></div>
    {!workspaces.length && <section className="panel empty"><h2>A home for your first project.</h2><p className="muted">Create a workspace to bring your conversations together.</p></section>}
    <div className="library">{workspaces.map(workspace => <button className={`project ${workspace.color}`} key={workspace.id} onClick={() => onWorkspace(workspace.id)}><span className="card-top"><span className="project-symbol" aria-hidden="true">{workspace.symbol}</span><span className="tag project-type">Project workspace</span></span><span className="project-title">{workspace.name}</span><span className="project-description">{workspace.description}</span><span className="card-bottom"><small>{meetings.filter(meeting => meeting.project === workspace.id).length} meetings · {meetings.find(meeting => meeting.project === workspace.id)?.date || "Just created"}</small><span aria-hidden="true">↗</span></span></button>)}</div>
    <section className="panel workspace-tip"><span className="tip-symbol" aria-hidden="true">⌘</span><div><h3>A shared thread, without the searching.</h3><p className="muted spaced-small">Each workspace connects your meetings, decisions, and next steps — with an assistant that knows the context.</p></div></section>
  </>;
}
