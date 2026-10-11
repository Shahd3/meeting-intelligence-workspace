import type { Completion, Meeting, MeetingTab, Workspace } from "../data/types";

const tabs: MeetingTab[] = ["Summary", "Highlights", "Decisions", "Action items", "Transcript"];

export function MeetingDetail({ meeting, workspaces, tab, completed, onTab, onBack, onAssign, onWorkspace, onComplete, onEdit, onDelete }: {
  meeting: Meeting; workspaces: Workspace[]; tab: MeetingTab; completed: Completion; onTab: (tab: MeetingTab) => void;
  onBack: () => void; onAssign: (project: number) => void; onWorkspace: (id: number) => void; onComplete: (key: string, value: boolean) => void;
  onEdit: () => void; onDelete: () => void;
}) {
  const workspace = workspaces.find(item => item.id === meeting.project);
  return <>
    <div className="detail-heading"><button className="circle" onClick={onBack} aria-label="Back">←</button><div><span className="eyebrow">MEETING NOTEBOOK / CONVERSATION</span><h1>{meeting.title}</h1></div></div>
    <div className="record-actions"><button className="pill" onClick={onEdit}>Edit meeting</button><button className="text" onClick={onDelete}>Delete meeting</button></div>
    <div className="meeting-meta"><span className="pill">{meeting.date} · {meeting.duration}</span><span className="tag sage">✓ Summary ready</span><select className="select" aria-label="Assign meeting to workspace" value={meeting.project} onChange={event => onAssign(Number(event.target.value))}><option value="0">Unassigned</option>{workspaces.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</select>{workspace && <button className="text" onClick={() => onWorkspace(workspace.id)}>Open workspace ↗</button>}</div>
    <div className="tabs" role="tablist" aria-label="Meeting details">{tabs.map((item, index) => <button key={item} id={`meeting-tab-${index}`} role="tab" aria-selected={tab === item} aria-controls="meeting-tab-panel" tabIndex={tab === item ? 0 : -1} className={`tab ${tab === item ? "active" : ""}`} onClick={() => onTab(item)} onKeyDown={event => {
      let next = index;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = tabs.length - 1;
      else return;
      event.preventDefault(); onTab(tabs[next]); document.getElementById(`meeting-tab-${next}`)?.focus();
    }}>{item}</button>)}</div>
    <section className="panel meeting-panel" id="meeting-tab-panel" role="tabpanel" aria-labelledby={`meeting-tab-${tabs.indexOf(tab)}`} tabIndex={0}>
      <div className="section"><h2>{tab === "Summary" ? "The conversation, distilled." : tab}</h2><span className="eyebrow">{meeting.speakers.length} PARTICIPANTS</span></div>
      {tab === "Summary" && <><p className="prose">{meeting.summary}</p><div className="callout"><span className="eyebrow">THE TAKEAWAY</span><p className="prose">{meeting.decision}</p></div><h3>In the room</h3><div className="participants">{meeting.speakers.map(speaker => <span className="pill" key={speaker}>{speaker}</span>)}</div></>}
      {tab === "Highlights" && meeting.highlights.map((highlight, index) => <div className="row" key={index}><span className="badge lav">{String(index + 1).padStart(2, "0")}</span><p className="prose">{highlight}</p></div>)}
      {tab === "Decisions" && <div className="callout"><span className="eyebrow">AGREED IN THIS MEETING</span><p className="prose">{meeting.decision}</p></div>}
      {tab === "Action items" && meeting.actions.map((action, index) => {
        const key = `${meeting.id}-${index}`;
        return <div className="task" key={key}><input type="checkbox" id={`action-${key}`} checked={!!completed[key]} onChange={event => onComplete(key, event.target.checked)} /><label htmlFor={`action-${key}`}>{action}<small>{workspace?.owner || "Shahd"} · Due {workspace?.due || "Oct 14"}</small></label><span className="tag">{completed[key] ? "Done" : "To do"}</span></div>;
      })}
      {tab === "Transcript" && <div className="transcript">{[...meeting.highlights, meeting.summary, ...meeting.highlights].map((text, index) => <div className="speech" key={index}><small>{String(index * 4).padStart(2, "0")}:24</small><div><h3>{meeting.speakers[index % meeting.speakers.length]}</h3><p>{text}</p></div></div>)}</div>}
    </section><p className="muted demo-caption">✦ Illustrative summaries and transcript · UI demo</p>
  </>;
}
