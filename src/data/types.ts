export type Color = "pink" | "sage" | "lav" | "yellow" | "peach";
export type Workspace = {
  id: number; name: string; description: string; color: Color; symbol: string;
  overview: string; decision?: string | null; task: string; owner: string; due: string;
};
export type Meeting = {
  recorded?: boolean; id: number; title: string; project: number; date: string; duration: string;
  summary: string; highlights: string[]; decision: string; actions: string[]; speakers: string[];
};
export type Completion = Record<string, boolean>;
export type Page = "home" | "notes" | "spaces";
export type MeetingTab = "Summary" | "Highlights" | "Decisions" | "Action items" | "Transcript" | "Minutes";
export type ChatMessage = { id: number; user: boolean; text: string; meetingIds: number[] };
