import { useCallback, useRef, useState } from "react";
import { simulateAnswer } from "../data/simulation";
import type { ChatMessage, Color, Meeting, Workspace, Page } from "../data/types";
import type { useLocalData } from "../data/useLocalData";

type Dialog = { kind: "about" | "settings" | "create" | "record" } | { kind: "editWorkspace"; workspace: Workspace } | { kind: "editMeeting"; meeting: Meeting } | { kind: "deleteWorkspace" | "deleteMeeting"; id: number };
export const dialogTitles = { settings: "Recording preferences", about: "A thoughtful space to gather.", create: "A new place for your ideas.", record: "Make room for the conversation.", editWorkspace: "Edit workspace", editMeeting: "Edit meeting", deleteWorkspace: "Delete workspace?", deleteMeeting: "Delete meeting?" };


type Options = Pick<ReturnType<typeof useLocalData>, "busy" | "run" | "meetings" | "completed"> & {
  workspace: Workspace | undefined;
  navigate: (page: Page) => void;
  notify: (text: string) => void;
};

export function useDialogs({ busy, run, meetings, completed, workspace, navigate, notify }: Options) {
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [chats, setChats] = useState<Record<number, ChatMessage[]>>({});
  const ids = useRef(Date.now());
  const nextId = () => ++ids.current;
  const dismiss = useCallback(() => setDialog(null), []);
  const openRecording = useCallback(() => setDialog({ kind: "record" }), []);
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
  const updateMeeting = async (updated: Meeting, titleEdited = false) => {
    if (!await run({ kind: "update_meeting", meeting: updated, title_edited: titleEdited })) return;
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

  return { dialog, setDialog, close, dismiss, openRecording, chats, createWorkspace, updateMeeting, deleteRecord, ask };
}
