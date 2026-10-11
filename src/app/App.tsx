import { useEffect, useRef, useState } from "react";
import { AppShell } from "../components/AppShell";
import { AboutDemo, CreateWorkspaceForm, RecordingDemo, SaveMeetingForm } from "../components/DemoDialogs";
import { Modal } from "../components/Modal";
import { initialMeetings, initialWorkspaces } from "../data/mockData";
import { createDemoMeeting, simulateAnswer } from "../data/simulation";
import type { ChatMessage, Color, Completion, MeetingTab, Page } from "../data/types";
import { HomePage } from "../pages/HomePage";
import { MeetingDetail } from "../pages/MeetingDetail";
import { NoteTakerPage } from "../pages/NoteTakerPage";
import { WorkspaceDetail } from "../pages/WorkspaceDetail";
import { WorkspacesPage } from "../pages/WorkspacesPage";

type Location = { page: Page; meetingId?: number; workspaceId?: number };
type Dialog = { kind: "about" | "create" | "record" } | { kind: "review"; seconds: number };
const dialogTitles = { about: "A thoughtful space to gather.", create: "A new place for your ideas.", record: "Make room for the conversation.", review: "Captured. Now, connect it." };

export function App() {
  const [meetings, setMeetings] = useState(initialMeetings);
  const [workspaces, setWorkspaces] = useState(initialWorkspaces);
  const [location, setLocation] = useState<Location>({ page: "home" });
  const [tab, setTab] = useState<MeetingTab>("Summary");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [completed, setCompleted] = useState<Completion>({});
  const [chats, setChats] = useState<Record<number, ChatMessage[]>>({});
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [toast, setToast] = useState<{ id: number; text: string } | null>(null);
  const ids = useRef(Date.now());
  const previousView = useRef("");
  const nextId = () => ++ids.current;
  const meeting = meetings.find(item => item.id === location.meetingId);
  const workspace = workspaces.find(item => item.id === location.workspaceId);
  const dashboard = location.page === "home" && !meeting && !workspace;
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
  const complete = (key: string, value: boolean) => setCompleted(current => ({ ...current, [key]: value }));
  const record = () => setDialog({ kind: "record" });
  const close = () => setDialog(null);
  const createWorkspace = (name: string, description: string, color: Color) => {
    const id = nextId();
    setWorkspaces(current => [...current, { id, name, description: description || "A new chapter, ready to begin.", color, symbol: "✳", overview: "A new space to collect conversations and build shared understanding.", task: "", owner: "Shahd", due: "Oct 14" }]);
    close(); navigate("spaces"); notify("Workspace created. Add your first meeting.");
  };
  const saveMeeting = (title: string, project: number) => {
    if (dialog?.kind !== "review") return;
    const id = nextId();
    setMeetings(current => [createDemoMeeting(id, title, project, dialog.seconds), ...current]);
    close(); setLocation({ page: "notes", meetingId: id }); setTab("Summary"); notify("Meeting saved. Your summary is ready.");
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
      {meeting ? <MeetingDetail meeting={meeting} workspaces={workspaces} tab={tab} completed={completed} onTab={setTab} onBack={() => setLocation(current => ({ ...current, meetingId: undefined }))} onAssign={project => {
        setMeetings(current => current.map(item => item.id === meeting.id ? { ...item, project } : item));
        setChats({}); notify("Workspace assignment updated");
      }} onWorkspace={openWorkspace} onComplete={complete} />
        : workspace ? <WorkspaceDetail workspace={workspace} meetings={meetings.filter(item => item.project === workspace.id)} completed={completed} messages={chats[workspace.id] || []} onBack={() => navigate("spaces")} onMeeting={openMeeting} onComplete={complete} onAsk={ask} />
          : location.page === "home" ? <HomePage meetings={meetings} workspaces={workspaces} completed={completed} onComplete={complete} onRecord={record} onMeeting={openMeeting} onWorkspace={openWorkspace} onNavigate={navigate} />
            : location.page === "notes" ? <NoteTakerPage meetings={meetings} workspaces={workspaces} query={query} filter={filter} onQuery={setQuery} onFilter={setFilter} onRecord={record} onMeeting={openMeeting} />
              : <WorkspacesPage meetings={meetings} workspaces={workspaces} onCreate={() => setDialog({ kind: "create" })} onWorkspace={openWorkspace} />}
    </AppShell>
    {dialog && <Modal title={dialogTitles[dialog.kind]} onClose={close}>
      {dialog.kind === "about" && <AboutDemo onClose={close} />}
      {dialog.kind === "create" && <CreateWorkspaceForm onSave={createWorkspace} onClose={close} />}
      {dialog.kind === "record" && <RecordingDemo onStop={seconds => setDialog({ kind: "review", seconds })} />}
      {dialog.kind === "review" && <SaveMeetingForm seconds={dialog.seconds} workspaces={workspaces} onSave={saveMeeting} onClose={close} />}
    </Modal>}
    <div role="status" aria-live="polite">{toast && <div className="toast" key={toast.id}>{toast.text}</div>}</div>
  </>;
}
