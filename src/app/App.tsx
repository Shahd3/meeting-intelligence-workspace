import { useEffect, useRef, useState } from "react";
import { AppShell } from "../components/AppShell";
import { AboutDemo, CreateWorkspaceForm } from "../components/DemoDialogs";
import { needsRecovery, savedMeetingAvailable } from "../data/recordingFlow";
import { useRecording } from "../data/recording";
import { RecordingPanel } from "../components/RecordingPanel";
import { RecordingPlayback } from "../components/RecordingPlayback";
import { Modal } from "../components/Modal";
import { useLocalData } from "../data/useLocalData";
import { EditMeetingForm } from "../components/EditMeetingForm";
import { simulateAnswer } from "../data/simulation";
import type { ChatMessage, Color, Meeting, Workspace, MeetingTab, Page } from "../data/types";
import { HomePage } from "../pages/HomePage";
import { MeetingDetail } from "../pages/MeetingDetail";
import { NoteTakerPage } from "../pages/NoteTakerPage";
import { WorkspaceDetail } from "../pages/WorkspaceDetail";
import { WorkspacesPage } from "../pages/WorkspacesPage";

type Location = { page: Page; meetingId?: number; workspaceId?: number };
type Dialog = { kind: "about" | "create" | "record" } | { kind: "editWorkspace"; workspace: Workspace } | { kind: "editMeeting"; meeting: Meeting } | { kind: "deleteWorkspace" | "deleteMeeting"; id: number };
const dialogTitles = { about: "A thoughtful space to gather.", create: "A new place for your ideas.", record: "Make room for the conversation.", editWorkspace: "Edit workspace", editMeeting: "Edit meeting", deleteWorkspace: "Delete workspace?", deleteMeeting: "Delete meeting?" };

export function App() {
  const { meetings, workspaces, completed, recordings, ready, busy, error, run } = useLocalData();
  const recorder = useRecording();
  const [location, setLocation] = useState<Location>({ page: "home" });
  const [tab, setTab] = useState<MeetingTab>("Summary");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [chats, setChats] = useState<Record<number, ChatMessage[]>>({});
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [toast, setToast] = useState<{ id: number; text: string } | null>(null);
  const [recordingOpen, setRecordingOpen] = useState(0);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const observedRecording = useRef<string | null>(null);
  const refreshedRecording = useRef<string | null>(null);
  const refreshPending = useRef(false);
  const [retrySave, setRetrySave] = useState(0);
  useEffect(() => {
    if (recorder.active && recorder.status.id) observedRecording.current = recorder.status.id;
    const id = recorder.status.id;
    if (!id || recorder.active || !["saved", "failed"].includes(recorder.status.state) || refreshedRecording.current === id || busy || refreshPending.current) return;
    refreshedRecording.current = id;
    refreshPending.current = true;
    const returnToLibrary = recorder.status.state === "saved" && observedRecording.current === id;
    if (returnToLibrary) { setSaving(true); setSaveError(null); }
    void run({ kind: "load" }).then(data => {
      if (returnToLibrary) {
        if (savedMeetingAvailable(recorder.status, data?.meetings)) {
          setDialog(null); setLocation({ page: "notes" }); setQuery(""); setFilter("all");
          setToast({ id: Date.now(), text: "Recording saved. Your meeting is ready in Note Taker." });
          observedRecording.current = null; setSaving(false);
        } else {
          setSaveError("Audio was saved, but the meeting list could not be refreshed. Retry to reopen your library.");
        }
      }
    }).finally(() => { refreshPending.current = false; });
  }, [recorder.status.id, recorder.status.state, recorder.status.meeting_id, recorder.active, busy, run, retrySave]);
  const retrySaved = () => { refreshedRecording.current = null; setRetrySave(value => value + 1); };
  const recover = async () => {
    if (!await recorder.recover()) return;
    if (await run({ kind: "load" })) {
      observedRecording.current = null; setDialog(null); setLocation({ page: "notes" }); setQuery(""); setFilter("all");
      setToast({ id: Date.now(), text: "Recovery checked. Retained audio is available in Note Taker." });
    }
  };
  const ids = useRef(Date.now());
  const previousView = useRef("");
  const nextId = () => ++ids.current;
  const meeting = meetings.find(item => item.id === location.meetingId);
  const workspace = workspaces.find(item => item.id === location.workspaceId);
  const dashboard = ready && location.page === "home" && !meeting && !workspace;
  const viewKey = `${location.page}:${location.meetingId || ""}:${location.workspaceId || ""}`;

  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 3000);
    return () => window.clearTimeout(timer);
  }, [toast]);
  useEffect(() => {
    document.body.classList.toggle("dashboard", dashboard);
    return () => document.body.classList.remove("dashboard");
  }, [dashboard]);
  useEffect(() => {
    if (previousView.current && previousView.current !== viewKey) {
      window.scrollTo(0, 0);
      const heading = document.querySelector<HTMLElement>("#view h1");
      heading?.setAttribute("tabindex", "-1"); heading?.focus({ preventScroll: true });
    }
    previousView.current = viewKey;
  }, [viewKey]);

  const notify = (text: string) => setToast({ id: nextId(), text });
  const navigate = (page: Page) => { setLocation({ page }); setQuery(""); setFilter("all"); };
  const openMeeting = (id: number) => { setLocation(current => ({ ...current, meetingId: id })); setTab("Summary"); };
  const openWorkspace = (id: number) => setLocation({ page: "spaces", workspaceId: id });
  const complete = (key: string, value: boolean) => { void run({ kind: "complete", key, value }); };
  const record = () => { setRecordingOpen(value => value + 1); setDialog({ kind: "record" }); };
  const close = () => { if (!busy) setDialog(null); };
  const createWorkspace = async (name: string, description: string, color: Color) => {
    const editing = dialog?.kind === "editWorkspace" ? dialog.workspace : null;
    const data = await run({ kind: editing ? "update_workspace" : "create_workspace", workspace: {
      ...(editing || { id: 0, symbol: "✳", overview: "A new space to collect conversations and build shared understanding.", task: "", owner: "", due: "" }), name, description, color,
    } });
    if (!data) return;
    setDialog(null); setChats({});
    if (!editing) navigate("spaces");
    notify(editing ? "Workspace updated." : "Workspace created. Add your first meeting.");
  };
  const updateMeeting = async (updated: Meeting) => {
    if (!await run({ kind: "update_meeting", meeting: updated })) return;
    setDialog(null); setChats({}); notify("Meeting updated.");
  };
  const deleteRecord = async () => {
    if (dialog?.kind !== "deleteWorkspace" && dialog?.kind !== "deleteMeeting") return;
    const kind = dialog.kind === "deleteWorkspace" ? "delete_workspace" : "delete_meeting";
    if (!await run({ kind, id: dialog.id })) return;
    setDialog(null); setChats({}); navigate(kind === "delete_workspace" ? "spaces" : "notes");
    notify(kind === "delete_workspace" ? "Workspace deleted. Meetings are now unassigned." : "Meeting deleted.");
  };
  const ask = (question: string) => {
    if (!workspace) return;
    const scoped = meetings.filter(item => item.project === workspace.id);
    const user: ChatMessage = { id: nextId(), user: true, text: question, meetingIds: [] };
    const answer: ChatMessage = { id: nextId(), user: false, text: simulateAnswer(question, workspace, scoped, completed), meetingIds: scoped.some(item => item.recorded) ? [] : scoped.slice(0, 2).map(item => item.id) };
    setChats(current => ({ ...current, [workspace.id]: [...(current[workspace.id] || []), user, answer] }));
  };

  return <>
    <AppShell page={location.page} dashboard={dashboard} onNavigate={navigate} onAbout={() => setDialog({ kind: "about" })}>
      {!ready ? <section className="panel empty" role="status"><h1>{busy ? "Opening your local workspace…" : "Your local workspace is unavailable."}</h1>{!busy && <button className="dark" onClick={() => void run({ kind: "load" })}>Try again</button>}</section>
        : meeting ? <MeetingDetail recordings={recordings.filter(item => item.meeting_id === meeting.id)} meeting={meeting} workspaces={workspaces} tab={tab} completed={completed} onTab={setTab} onBack={() => setLocation(current => ({ ...current, meetingId: undefined }))} onAssign={project => {
        void updateMeeting({ ...meeting, project });
      }} onEdit={() => setDialog({ kind: "editMeeting", meeting })} onDelete={() => setDialog({ kind: "deleteMeeting", id: meeting.id })} onWorkspace={openWorkspace} onComplete={complete} />
        : workspace ? <WorkspaceDetail workspace={workspace} meetings={meetings.filter(item => item.project === workspace.id)} completed={completed} messages={chats[workspace.id] || []} onEdit={() => setDialog({ kind: "editWorkspace", workspace })} onDelete={() => setDialog({ kind: "deleteWorkspace", id: workspace.id })} onBack={() => navigate("spaces")} onMeeting={openMeeting} onComplete={complete} onAsk={ask} />
          : location.page === "home" ? <HomePage meetings={meetings} workspaces={workspaces} completed={completed} onComplete={complete} onRecord={record} onMeeting={openMeeting} onWorkspace={openWorkspace} onNavigate={navigate} />
            : location.page === "notes" ? <NoteTakerPage meetings={meetings} workspaces={workspaces} query={query} filter={filter} onQuery={setQuery} onFilter={setFilter} onRecord={record} onMeeting={openMeeting} />
              : <WorkspacesPage meetings={meetings} workspaces={workspaces} onCreate={() => setDialog({ kind: "create" })} onWorkspace={openWorkspace} />}
      {ready && recordings.some(needsRecovery) && <aside className="recovery-notice" role="status"><p>Interrupted audio is retained and needs recovery.</p><button className="pill" disabled={recorder.active || recorder.busy || busy} onClick={() => void recover()}>Recover interrupted recording</button></aside>}
      {ready && location.page === "notes" && !meeting && recordings.some(item => item.meeting_id === null) && <section><h2>Retained & interrupted recordings</h2>{recordings.filter(item => item.meeting_id === null).map(item => <RecordingPlayback key={item.id} recording={item} />)}</section>}
    </AppShell>
    {recorder.active && dialog?.kind !== "record" && <div className="recording-banner" role="status"><span>{recorder.status.state} · {Math.floor(recorder.status.elapsed_ms / 1000)}s{recorder.status.warning && <small className="recording-banner-warning">{recorder.status.warning}</small>}</span><button className="pill" onClick={record}>Open recording controls</button></div>}
    {saveError && dialog?.kind !== "record" && <div className="recording-alert" role="alert"><p>{saveError}</p><button className="pill" onClick={retrySaved}>Retry loading saved recording</button></div>}
    {((recorder.status.error && observedRecording.current === recorder.status.id) || recorder.error) && dialog?.kind !== "record" && <div className="recording-alert" role="alert"><p>{recorder.status.error || recorder.error}</p><button className="pill" disabled={recorder.active || recorder.busy || busy} onClick={() => void recover()}>Check recording recovery</button></div>}
    {busy && <div className="storage-busy" role="status" aria-live="polite">{ready ? "Saving local data…" : "Loading local data…"}</div>}
    {error && <div className="storage-error" role="alert"><p>{error}</p><button className="pill" disabled={busy} onClick={() => void run({ kind: "load" })}>Reload data</button></div>}
    {dialog && <Modal title={dialogTitles[dialog.kind]} onClose={close}>
      <fieldset className="dialog-fields" disabled={busy}>
      {dialog.kind === "about" && <AboutDemo onClose={close} />}
      {dialog.kind === "create" && <CreateWorkspaceForm onSave={createWorkspace} onClose={close} />}
      {dialog.kind === "editWorkspace" && <CreateWorkspaceForm initial={dialog.workspace} onSave={createWorkspace} onClose={close} />}
      {dialog.kind === "editMeeting" && <EditMeetingForm meeting={dialog.meeting} onSave={updateMeeting} onClose={close} />}
      {(dialog.kind === "deleteWorkspace" || dialog.kind === "deleteMeeting") && <><p className="prose">{dialog.kind === "deleteWorkspace" ? "Meetings will become unassigned. Their notes and any recording files will be kept." : "This removes the meeting notes and task completion permanently. Any recording files will be kept."}</p><div className="actions"><button className="pill" onClick={close}>Cancel</button><button className="dark" disabled={busy} onClick={() => void deleteRecord()}>Delete</button></div></>}
      {dialog.kind === "record" && <RecordingPanel key={recordingOpen} recorder={recorder} saving={saving} saveError={saveError} onRetry={retrySaved} onRecovered={() => { void run({ kind: "load" }).then(data => { if (data) { observedRecording.current = null; setDialog(null); navigate("notes"); notify("Recovery checked. Retained recordings are available in Note Taker."); } }); }} />}
      </fieldset>
    </Modal>}
    <div role="status" aria-live="polite">{toast && <div className="toast" key={toast.id}>{toast.text}</div>}</div>
  </>;
}
