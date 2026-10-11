import type { Meeting, Workspace } from "./types";

export function formatTime(seconds: number) {
  return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

export function createDemoMeeting(id: number, title: string, project: number, seconds: number): Meeting {
  return {
    id, title, project, date: new Date().toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" }), duration: formatTime(seconds),
    summary: "The team reviewed progress, clarified priorities, and agreed to share an updated plan before the next check-in. This is a sample summary of the simulated recording.",
    highlights: ["Reviewed project progress.", "Clarified priorities and assigned an owner.", "Agreed to circulate the updated plan."],
    decision: "Share the updated project plan before the next check-in.",
    actions: ["Share the updated project plan"], speakers: ["Shahd", "Lina", "Omar"],
  };
}

export function simulateAnswer(question: string, workspace: Workspace, meetings: Meeting[], completed: Record<string, boolean>) {
  if (!meetings.length) return "There are no meetings in this workspace yet. Assign a conversation from Note Taker to start exploring.";
  const latest = meetings[0];
  if (/pending|task|next/i.test(question)) {
    const actions = [...new Set(meetings.flatMap(meeting => meeting.actions.filter((_, index) => !completed[`${meeting.id}-${index}`])))];
    const next = workspace.task && !completed[`workspace-${workspace.id}`] ? workspace.task : actions[0];
    if (!next) return "There are no pending tasks in these sample notes.";
    return `The next step is ${next}, owned by ${workspace.owner}, due ${workspace.due}. Your meetings also captured: ${actions.join("; ") || "No other pending actions."}`;
  }
  if (/chang|between/i.test(question)) return meetings.length > 1
    ? `Earlier, the team agreed: ${meetings[meetings.length - 1].decision} The latest conversation moved this forward: ${latest.decision}`
    : `There is one conversation so far, so a comparison is not available yet. The current agreement is: ${latest.decision}`;
  if (/timeline|when|date/i.test(question)) return `The latest conversation was ${latest.title} on ${latest.date}. No verified project deadline is available in this simulation.`;
  return `For ${workspace.name}, the latest agreement is: ${latest.decision} ${latest.summary}`;
}
