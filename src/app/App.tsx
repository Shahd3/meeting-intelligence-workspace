import { DeleteButton } from "../components/Icons";
import { RecordingPlayback } from "../components/RecordingPlayback";
import { TranscriptionPreferences } from "../components/Transcription";
import { RecordingPreferences } from "../components/RecordingPreferences";
import { AppShell } from "../components/AppShell";
import { AboutDemo, CreateWorkspaceForm } from "../components/DemoDialogs";
import { needsRecovery } from "../data/recordingFlow";
import { RecordingPanel } from "../components/RecordingPanel";
import { Modal } from "../components/Modal";
import { useLocalData } from "../data/useLocalData";
import { EditMeetingForm } from "../components/EditMeetingForm";
import { HomePage } from "../pages/HomePage";
import { MeetingDetail } from "../pages/MeetingDetail";
import { NoteTakerPage } from "../pages/NoteTakerPage";
import { WorkspaceDetail } from "../pages/WorkspaceDetail";
import { WorkspacesPage } from "../pages/WorkspacesPage";

import { useAppNavigation } from "../hooks/useAppNavigation";
import { useDialogs, dialogTitles } from "../hooks/useDialogs";
import { useRecordingFlow } from "../hooks/useRecordingFlow";
import { useToast } from "../hooks/useToast";

export function App() {
  const data = useLocalData();
  const { meetings, workspaces, completed, recordings, ready, busy, error, run, complete } = data;
  const { toast, notify } = useToast();
  const {
    location, tab, setTab, query, setQuery, filter, setFilter, meeting, workspace,
    dashboard, navigate, openMeeting, openWorkspace, backFromMeeting,
  } = useAppNavigation(data);
  const {
    dialog, setDialog, close, dismiss, openRecording, chats,
    createWorkspace, updateMeeting, deleteRecord, ask,
  } = useDialogs({ ...data, workspace, navigate, notify });
  const {
    recorder, desktop, recordingOpen, saving, saveError, retrySaved,
    record, recover, onRecovered, observedRecording,
  } = useRecordingFlow({ ready, busy, run, dismiss, openRecording, navigate, notify });

  return <>
    <AppShell page={location.page} dashboard={dashboard} onNavigate={navigate} onAbout={() => setDialog({ kind: "about" })} onSettings={() => setDialog({ kind: "settings" })}>
      {!ready ? <section className="panel empty" role="status"><h1>{busy ? "Opening your local workspace…" : "Your local workspace is unavailable."}</h1>{!busy && <button className="dark" onClick={() => void run({ kind: "load" })}>Try again</button>}</section>
        : meeting ? <MeetingDetail recordings={recordings.filter(item => item.meeting_id === meeting.id)} meeting={meeting} workspaces={workspaces} tab={tab} completed={completed} onTab={setTab} onBack={backFromMeeting} onAssign={project => {
        void updateMeeting({ ...meeting, project });
      }} onEdit={() => setDialog({ kind: "editMeeting", meeting })} onDelete={() => setDialog({ kind: "deleteMeeting", id: meeting.id })} onWorkspace={openWorkspace} onComplete={complete} />
        : workspace ? <WorkspaceDetail workspace={workspace} meetings={meetings.filter(item => item.project === workspace.id)} completed={completed} messages={chats[workspace.id] || []} onEdit={() => setDialog({ kind: "editWorkspace", workspace })} onDelete={() => setDialog({ kind: "deleteWorkspace", id: workspace.id })} onBack={() => navigate("spaces")} onMeeting={openMeeting} onComplete={complete} onAsk={ask} />
          : location.page === "home" ? <HomePage meetings={meetings} workspaces={workspaces} completed={completed} onComplete={complete} onRecord={record} onMeeting={openMeeting} onWorkspace={openWorkspace} onNavigate={navigate} />
            : location.page === "notes" ? <NoteTakerPage meetings={meetings} workspaces={workspaces} query={query} filter={filter} onQuery={setQuery} onFilter={setFilter} onRecord={record} onMeeting={openMeeting} />
              : <WorkspacesPage meetings={meetings} workspaces={workspaces} onCreate={() => setDialog({ kind: "create" })} onWorkspace={openWorkspace} />}
      {ready && recordings.some(needsRecovery) && <aside className="recovery-notice" role="status"><p>Interrupted audio is retained and needs recovery.</p><button className="pill" disabled={recorder.active || recorder.busy || busy} onClick={() => void recover()}>Recover interrupted recording</button></aside>}
    </AppShell>
    {recorder.active && dialog?.kind !== "record" && <div className="recording-banner" role="status"><span>{recorder.status.state} · {Math.floor(recorder.status.elapsed_ms / 1000)}s{recorder.status.warning && <small className="recording-banner-warning">{recorder.status.warning}</small>}</span><button className="pill" onClick={record}>Open recording controls</button></div>}
    {saveError && dialog?.kind !== "record" && <div className="recording-alert" role="alert"><p>{saveError}</p><button className="pill" onClick={retrySaved}>Retry loading saved recording</button></div>}
    {((recorder.status.error && observedRecording.current === recorder.status.id) || recorder.error) && dialog?.kind !== "record" && <div className="recording-alert" role="alert"><p>{recorder.status.error || recorder.error}</p><button className="pill" disabled={recorder.active || recorder.busy || busy} onClick={() => void recover()}>Check recording recovery</button></div>}
    {desktop.view?.widget_error && <div className="recording-alert" role="alert">{desktop.view.widget_error}</div>}
    {busy && <div className="storage-busy" role="status" aria-live="polite">{ready ? "Saving local data…" : "Loading local data…"}</div>}
    {error && <div className="storage-error" role="alert"><p>{error}</p><button className="pill" disabled={busy} onClick={() => void run({ kind: "load" })}>Reload data</button></div>}
    {dialog && <Modal title={dialogTitles[dialog.kind]} onClose={close}>
      <fieldset className="dialog-fields" disabled={busy}>
      {dialog.kind === "settings" && <><RecordingPreferences /><TranscriptionPreferences />{recordings.some(item => item.meeting_id === null || needsRecovery(item)) && <details className="audio-settings"><summary>Recording recovery and retained audio</summary><button className="pill" disabled={recorder.active || recorder.busy} onClick={() => void recover()}>Check recording recovery</button>{recordings.filter(item => item.meeting_id === null || needsRecovery(item)).map(item => <RecordingPlayback key={item.id} recording={item}/>)}</details>}</>}
      {dialog.kind === "about" && <AboutDemo onClose={close} />}
      {dialog.kind === "create" && <CreateWorkspaceForm onSave={createWorkspace} onClose={close} />}
      {dialog.kind === "editWorkspace" && <CreateWorkspaceForm initial={dialog.workspace} onSave={createWorkspace} onClose={close} />}
      {dialog.kind === "editMeeting" && <EditMeetingForm meeting={dialog.meeting} onSave={updateMeeting} onClose={close} />}
      {(dialog.kind === "deleteWorkspace" || dialog.kind === "deleteMeeting") && <><p className="prose">{dialog.kind === "deleteWorkspace" ? "Meetings will become unassigned. Their notes and any recording files will be kept." : "This removes the meeting notes and task completion permanently. Any recording files will be kept."}</p><div className="actions"><button className="pill" onClick={close}>Cancel</button><DeleteButton label={dialog.kind === "deleteWorkspace" ? "Confirm delete workspace" : "Confirm delete meeting"} disabled={busy} onClick={() => void deleteRecord()}/></div></>}
      {dialog.kind === "record" && <RecordingPanel key={recordingOpen} recorder={recorder} saving={saving} saveError={saveError} onRetry={retrySaved} onRecovered={onRecovered} />}
      </fieldset>
    </Modal>}
    <div role="status" aria-live="polite">{toast && <div className="toast" key={toast.id}>{toast.text}</div>}</div>
  </>;
}
