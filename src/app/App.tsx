import { useEffect, useRef, useState } from "react";
import { AppShell } from "../components/AppShell";
import { AboutDemo, CreateWorkspaceForm, RecordingDemo, SaveMeetingForm } from "../components/DemoDialogs";
import { Modal } from "../components/Modal";
import { useLocalData } from "../data/useLocalData";
import { EditMeetingForm } from "../components/EditMeetingForm";
import { createDemoMeeting, simulateAnswer } from "../data/simulation";
import type { ChatMessage, Color, Meeting, Workspace, MeetingTab, Page } from "../data/types";
import { HomePage } from "../pages/HomePage";
import { MeetingDetail } from "../pages/MeetingDetail";
import { NoteTakerPage } from "../pages/NoteTakerPage";
import { WorkspaceDetail } from "../pages/WorkspaceDetail";
import { WorkspacesPage } from "../pages/WorkspacesPage";

type Location = { page: Page; meetingId?: number; workspaceId?: number };
type Dialog = { kind: "about" | "create" | "record" } | { kind: "review"; seconds: number } | { kind: "editWorkspace"; workspace: Workspace } | { kind: "editMeeting"; meeting: Meeting } | { kind: "deleteWorkspace" | "deleteMeeting"; id: number };
const dialogTitles = { about: "A thoughtful space to gather.", create: "A new place for your ideas.", record: "Make room for the conversation.", review: "Captured. Now, connect it.", editWorkspace: "Edit workspace", editMeeting: "Edit meeting", deleteWorkspace: "Delete workspace?", deleteMeeting: "Delete meeting?" };

export function App() {
  const { meetings, workspaces, completed, ready, busy, error, run } = useLocalData();
  const [location, setLocation] = useState<Location>({ page: "home" });
  const [tab, setTab] = useState<MeetingTab>("Summary");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [chats, setChats] = useState<Record<number, ChatMessage[]>>({});
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [toast, setToast] = useState<{ id: number; text: string } | null>(null);
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
  const record = () => setDialog({ kind: "record" });
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
  const saveMeeting = async (title: string, project: number) => {
    if (dialog?.kind !== "review") return;
    const data = await run({ kind: "create_meeting", meeting: createDemoMeeting(0, title, project, dialog.seconds) });
    if (!data) return;
    setDialog(null); setChats({}); setLocation({ page: "notes", meetingId: data.meetings[0].id }); setTab("Summary"); notify("Meeting saved locally. Sample summary ready.");
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
    const answer: ChatMessage = { id: nextId(), user: false, text: simulateAnswer(question, workspace, scoped, completed), meetingIds: scoped.slice(0, 2).map(item => item.id) };
    setChats(current => ({ ...current, [workspace.id]: [...(current[workspace.id] || []), user, answer] }));
  };

  return <>
    <AppShell page={location.page} dashboard={dashboard} onNavigate={navigate} onAbout={() => setDialog({ kind: "about" })}>
      {!ready ? <section className="panel empty" role="status"><h1>{busy ? "Opening your local workspace…" : "Your local workspace is unavailable."}</h1>{!busy && <button className="dark" onClick={() => void run({ kind: "load" })}>Try again</button>}</section>
        : meeting ? <MeetingDetail meeting={meeting} workspaces={workspaces} tab={tab} completed={completed} onTab={setTab} onBack={() => setLocation(current => ({ ...current, meetingId: undefined }))} onAssign={project => {
        void updateMeeting({ ...meeting, project });
      }} onEdit={() => setDialog({ kind: "editMeeting", meeting })} onDelete={() => setDialog({ kind: "deleteMeeting", id: meeting.id })} onWorkspace={openWorkspace} onComplete={complete} />
        : workspace ? <WorkspaceDetail workspace={workspace} meetings={meetings.filter(item => item.project === workspace.id)} completed={completed} messages={chats[workspace.id] || []} onEdit={() => setDialog({ kind: "editWorkspace", workspace })} onDelete={() => setDialog({ kind: "deleteWorkspace", id: workspace.id })} onBack={() => navigate("spaces")} onMeeting={openMeeting} onComplete={complete} onAsk={ask} />
          : location.page === "home" ? <HomePage meetings={meetings} workspaces={workspaces} completed={completed} onComplete={complete} onRecord={record} onMeeting={openMeeting} onWorkspace={openWorkspace} onNavigate={navigate} />
            : location.page === "notes" ? <NoteTakerPage meetings={meetings} workspaces={workspaces} query={query} filter={filter} onQuery={setQuery} onFilter={setFilter} onRecord={record} onMeeting={openMeeting} />
              : <WorkspacesPage meetings={meetings} workspaces={workspaces} onCreate={() => setDialog({ kind: "create" })} onWorkspace={openWorkspace} />}
    </AppShell>
    {busy && <div className="storage-busy" role="status" aria-live="polite">{ready ? "Saving local data…" : "Loading local data…"}</div>}
    {error && <div className="storage-error" role="alert"><p>{error}</p><button className="pill" disabled={busy} onClick={() => void run({ kind: "load" })}>Reload data</button></div>}
    {dialog && <Modal title={dialogTitles[dialog.kind]} onClose={close}>
      <fieldset className="dialog-fields" disabled={busy}>
      {dialog.kind === "about" && <AboutDemo onClose={close} />}
      {dialog.kind === "create" && <CreateWorkspaceForm onSave={createWorkspace} onClose={close} />}
      {dialog.kind === "editWorkspace" && <CreateWorkspaceForm initial={dialog.workspace} onSave={createWorkspace} onClose={close} />}
      {dialog.kind === "editMeeting" && <EditMeetingForm meeting={dialog.meeting} onSave={updateMeeting} onClose={close} />}
      {(dialog.kind === "deleteWorkspace" || dialog.kind === "deleteMeeting") && <><p className="prose">{dialog.kind === "deleteWorkspace" ? "Meetings will become unassigned. Their notes and any recording files will be kept." : "This removes the meeting notes and task completion permanently. Any recording files will be kept."}</p><div className="actions"><button className="pill" onClick={close}>Cancel</button><button className="dark" disabled={busy} onClick={() => void deleteRecord()}>Delete</button></div></>}
      {dialog.kind === "record" && <RecordingDemo onStop={seconds => setDialog({ kind: "review", seconds })} />}
      {dialog.kind === "review" && <SaveMeetingForm seconds={dialog.seconds} workspaces={workspaces} onSave={saveMeeting} onClose={close} />}
      </fieldset>
    </Modal>}
    <div role="status" aria-live="polite">{toast && <div className="toast" key={toast.id}>{toast.text}</div>}</div>
  </>;
}
