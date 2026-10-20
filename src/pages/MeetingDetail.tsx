import { DeleteButton, EditButton } from "../components/Icons";
import { attendeeInitials, IntelligenceControls, IntelligenceContent } from "../components/MeetingIntelligence";
import { useEffect, useState } from "react";
import { MeetingTranscript, TranscriptionStatus } from "../components/Transcription";
import { useProcessing } from "../data/processing";
import type { Recording } from "../data/recording";
import { invoke } from "@tauri-apps/api/core";
import { meetingNotesReady, meetingSections } from "../data/intelligence";
import type { Completion, Meeting, MeetingTab, Workspace } from "../data/types";

const tabs: readonly MeetingTab[] = meetingSections;

export function MeetingDetail({ meeting, workspaces, tab, completed, onTab, onBack, onAssign, onWorkspace, onComplete, onEdit, onDelete }: {
  recordings: Recording[]; meeting: Meeting; workspaces: Workspace[]; tab: MeetingTab; completed: Completion; onTab: (tab: MeetingTab) => void;
  onBack: () => void; onAssign: (project: number) => void; onWorkspace: (id: number) => void; onComplete: (key: string, value: boolean) => void;
  onEdit: () => void; onDelete: () => void;
}) {
  const [evidence, setEvidence] = useState<string | null>(null);
  const processing = useProcessing();
  const [exporting, setExporting] = useState(false);
  const [exportMessage, setExportMessage] = useState<string | null>(null);
  const notesReady = !!processing.view && processing.view.transcript_meeting_id === meeting.id && !!processing.view.intelligence.result && meetingNotesReady(meeting.id, processing.view.jobs, processing.view.intelligence.jobs);
  const selectMeeting = processing.selectMeeting;
  useEffect(() => { selectMeeting(meeting.id); return () => selectMeeting(null); }, [meeting.id, selectMeeting]);
  const workspace = workspaces.find(item => item.id === meeting.project);
  return <div className="detail-page">
    <div className="detail-header">
    <button className="circle back-button" onClick={onBack} aria-label="Back">←</button>
    <div className="detail-heading"><div><span className="eyebrow">MEETING NOTEBOOK / CONVERSATION</span><h1>{meeting.title}</h1><small className="detail-date">{meeting.date} · {meeting.duration}</small></div>{notesReady && <span className="tag notes-ready">Notes ready</span>}<button className="pill mom-button" disabled={!notesReady || exporting} title={notesReady ? "Save Minutes of Meeting to Downloads" : "Meeting notes must be ready before generating MoM"} onClick={async () => { setExporting(true); setExportMessage(null); try { const name = await invoke<string>("intelligence_download", { meetingId: meeting.id }); setExportMessage(`Minutes saved to Downloads: ${name}`); } catch (cause) { setExportMessage(String(cause)); } finally { setExporting(false); } }}>{exporting ? "Saving MoM…" : "Generate MoM"}</button></div>
    {exportMessage && <p role="status">{exportMessage}</p>}
    <div className="meeting-meta"><select className="select" aria-label="Assign meeting to workspace" value={meeting.project} onChange={event => onAssign(Number(event.target.value))}><option value="0">Unassigned</option>{workspaces.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</select>{workspace && <button className="open-workspace" onClick={() => onWorkspace(workspace.id)} aria-label={`Open ${workspace.name} workspace`} title={`Open ${workspace.name} workspace`}>↗</button>}<div className="record-tools"><EditButton label="Edit meeting" onClick={onEdit}/><DeleteButton label="Delete meeting" onClick={onDelete}/></div></div>
    {processing.view?.jobs.filter(job => job.meeting_id === meeting.id).map(job => <TranscriptionStatus key={job.id} job={job} />)}
    {meeting.recorded && <IntelligenceControls key={meeting.id} meetingId={meeting.id}/>}
    <div className="tabs" role="tablist" aria-label="Meeting details">{tabs.map((item, index) => <button key={item} id={`meeting-tab-${index}`} role="tab" aria-selected={tab === item} aria-controls="meeting-tab-panel" tabIndex={tab === item ? 0 : -1} className={`tab ${tab === item ? "active" : ""}`} onClick={() => onTab(item)} onKeyDown={event => {
      let next = index;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = tabs.length - 1;
      else return;
      event.preventDefault(); onTab(tabs[next]); document.getElementById(`meeting-tab-${next}`)?.focus();
    }}>{item === "Action items" ? "Action Items" : item}</button>)}</div>
    </div>
    <section className="panel meeting-panel detail-scroll" id="meeting-tab-panel" role="tabpanel" aria-labelledby={`meeting-tab-${tabs.indexOf(tab)}`} tabIndex={0}>
      <div className="section"><h2>{tab === "Summary" ? "Executive summary" : tab}</h2><span className="eyebrow">{meeting.recorded ? "" : `${meeting.speakers.length} PARTICIPANTS`}</span></div>
      {meeting.recorded && tab !== "Transcript" && <IntelligenceContent key={meeting.id} meeting={meeting} tab={tab} onEvidence={id => { setEvidence(id); onTab("Transcript"); }}/>}
      {tab === "Summary" && !meeting.recorded && <><p className="prose">{meeting.summary}</p><div className="callout"><span className="eyebrow">THE TAKEAWAY</span><p className="prose">{meeting.decision}</p></div>{meeting.highlights.length > 0 && <details className="subsection sage" open><summary>Highlights</summary><ul className="points-list">{meeting.highlights.map((point, i) => <li key={i}><p className="prose">{point}</p></li>)}</ul></details>}{meeting.speakers.length > 0 && <details className="attendee-section" open><summary>In the room</summary><div className="avatar-stack">{meeting.speakers.map(speaker => <span className="attendee-avatar" key={speaker} title={speaker}>{attendeeInitials(speaker)}</span>)}</div></details>}</>}
      {tab === "Action items" && !meeting.recorded && meeting.actions.map((action, index) => {
        const key = `${meeting.id}-${index}`;
        return <div className="task" key={key}><input type="checkbox" id={`action-${key}`} checked={!!completed[key]} onChange={event => onComplete(key, event.target.checked)} /><label htmlFor={`action-${key}`}>{action}<small>{workspace?.owner || "Shahd"} · Due {workspace?.due || "Oct 14"}</small></label><span className="tag">{completed[key] ? "Done" : "To do"}</span></div>;
      })}
      {tab === "Transcript" && <MeetingTranscript meetingId={meeting.id} evidenceId={evidence} />}
    </section>{!meeting.recorded && <p className="muted demo-caption">✦ Illustrative summaries and transcript · UI demo</p>}
  </div>;
}
