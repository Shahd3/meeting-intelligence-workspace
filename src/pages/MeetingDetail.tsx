import { MeetingDetailHeader } from "../components/MeetingDetailHeader";
import { MeetingTabs } from "../components/MeetingTabs";
import { Summary } from "../components/meeting-detail-tabs/Summary";
import { ActionItems } from "../components/meeting-detail-tabs/ActionItems";
import { Transcript } from "../components/meeting-detail-tabs/Transcript";
import { DeleteButton } from "../components/Icons";
import { IntelligenceControls } from "../components/MeetingIntelligence";
import { useEffect, useState } from "react";
import { TranscriptionStatus } from "../components/Transcription";
import { useProcessing } from "../data/processing";
import type { Recording } from "../data/recording";
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
  const notesReady = !!processing.view && processing.view.transcript_meeting_id === meeting.id && !!processing.view.intelligence.result && meetingNotesReady(meeting.id, processing.view.jobs, processing.view.intelligence.jobs);
  const selectMeeting = processing.selectMeeting;
  useEffect(() => { selectMeeting(meeting.id); return () => selectMeeting(null); }, [meeting.id, selectMeeting]);
  const showEvidence = (id: string) => { setEvidence(id); onTab("Transcript"); };
  const workspace = workspaces.find(item => item.id === meeting.project);
  return <>
    <MeetingDetailHeader meeting={meeting} notesReady={notesReady} onBack={onBack} />
    <div className="record-actions"><button className="pill" onClick={onEdit}>Edit meeting</button><DeleteButton label="Delete meeting" onClick={onDelete}/></div>
    <div className="meeting-meta"><span className="pill">{meeting.date} · {meeting.duration}</span><span className="tag sage">{meeting.recorded ? "✓ Audio saved" : "✓ Summary ready"}</span><select className="select" aria-label="Assign meeting to workspace" value={meeting.project} onChange={event => onAssign(Number(event.target.value))}><option value="0">Unassigned</option>{workspaces.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</select>{workspace && <button className="text" onClick={() => onWorkspace(workspace.id)}>Open workspace ↗</button>}</div>
    {processing.view?.jobs.filter(job => job.meeting_id === meeting.id).map(job => <TranscriptionStatus key={job.id} job={job} />)}
    {meeting.recorded && <IntelligenceControls key={meeting.id} meetingId={meeting.id}/>}
    <MeetingTabs tab={tab} onTab={onTab} />
    <section className="panel meeting-panel" id="meeting-tab-panel" role="tabpanel" aria-labelledby={`meeting-tab-${tabs.indexOf(tab)}`} tabIndex={0}>
      <div className="section"><h2>{tab === "Summary" ? "The conversation, distilled." : tab}</h2><span className="eyebrow">{meeting.recorded ? "" : `${meeting.speakers.length} PARTICIPANTS`}</span></div>
      {tab === "Summary" && <Summary key={meeting.id} meeting={meeting} onEvidence={showEvidence} />}
      {tab === "Action items" && <ActionItems
        key={meeting.id} meeting={meeting} workspace={workspace} completed={completed}
        onComplete={onComplete} onEvidence={showEvidence}
      />}
      {tab === "Transcript" && <Transcript meetingId={meeting.id} evidenceId={evidence} />}
    </section>{!meeting.recorded && <p className="muted demo-caption">✦ Illustrative summaries and transcript · UI demo</p>}
  </>;
}
