import type { Meeting } from "./types";
export type Evidence = { segment_id: string; quote: string };
export type Point = { text: string; evidence: Evidence[] };
export type Topic = { heading: string; points: Point[] };
export type Action = { description: Point; owner: string | null; owner_evidence: Evidence[]; deadline: string | null; deadline_evidence: Evidence[] };
export type Notes = { schema_version: number; suggested_title: Point; executive_summary: Point; topics: Topic[]; decisions: Point[]; action_items: Action[]; follow_ups: Point[]; open_questions: Point[]; attendees: { name: string; role: string | null; evidence: Evidence[] }[] };
export type IntelligenceResult = { id: string; transcript_id: string; notes: Notes; coverage: { segment_count: number; analyzed_segments: number; chunk_count: number; source_warning: string | null }; actions: { id: string; ordinal: number; done: boolean }[] };
export type IntelligenceView = { previews: { meeting_id: number; summary: string; topics: string; decision: string; actions: { id: string; text: string; owner: string | null; deadline: string | null; done: boolean }[] }[]; jobs: { has_result: boolean; id: string; meeting_id: number; transcript_id: string; state: string; stage: string; attempts: number; requests: number; error: string | null; next_attempt_at: number | null }[]; result: IntelligenceResult | null; transcripts: { id: string; segment_count: number }[]; attendees_override: string[] | null; worker_error: string | null };
export function direction(text: string): "rtl" | "ltr" { return /[\u0600-\u06ff\u0750-\u077f\u08a0-\u08ff]/u.test(text) ? "rtl" : "ltr"; }
export function intelligenceLabel(state: string) { return ({ queued: "AI analysis queued", processing: "Analyzing meeting", retryable: "AI analysis retry scheduled", failed: "AI analysis failed", completed: "Notes ready" } as Record<string, string>)[state] || "AI analysis unavailable"; }
export function minutes(meeting: Pick<Meeting, "title" | "date" | "duration">, result: IntelligenceResult, attendees: string[] | null): string {
 const n = result.notes;
 const clean = (s: string) => s.replace(/[\r\n]+/g, " ").replace(/([\\`*_{}[\]<>#|])/g, "\\$1");
 const points = (items: Point[]) => items.length ? items.map(p => `- ${clean(p.text)}`).join("\n") : "None documented.";
 const names = attendees ?? n.attendees.map(a => a.role ? `${a.name} (${a.role})` : a.name);
 return [`# ${clean(meeting.title)}`, "AI-generated draft · Minutes of Meeting", `${clean(meeting.date)} · ${clean(meeting.duration)}`, "## Attendees", names.length ? names.map(clean).join(", ") : "Attendance not established.", "## Executive summary", clean(n.executive_summary.text) || "No substantive summary available.", "## Discussion", ...n.topics.map(t => `### ${clean(t.heading)}\n${points(t.points)}`), "## Decisions", points(n.decisions), "## Action items", n.action_items.length ? "| Action | Owner | Deadline |\n| --- | --- | --- |\n" + n.action_items.map(a => `| ${clean(a.description.text)} | ${clean(a.owner ?? "Not assigned")} | ${clean(a.deadline ?? "Not stated")} |`).join("\n") : "No actions documented.", "## Follow-ups", points(n.follow_ups), "## Open questions and unresolved proposals", points(n.open_questions), "## Source and coverage", `Transcript ${result.transcript_id}. ${result.coverage.analyzed_segments}/${result.coverage.segment_count} source segments processed. Processing coverage does not guarantee factual completeness.`, result.coverage.source_warning || "", "## Evidence", ...[n.executive_summary,n.suggested_title,...n.topics.flatMap(t=>t.points),...n.decisions,...n.follow_ups,...n.open_questions,...n.action_items.flatMap(a=>[a.description,{text: "",evidence:[...a.owner_evidence,...a.deadline_evidence]}]),...n.attendees.map(a=>({text:a.name,evidence:a.evidence}))].flatMap(p=>p.evidence).filter((e,i,all)=>all.findIndex(other=>other.segment_id===e.segment_id&&other.quote===e.quote)===i).map(e=>`- ${clean(e.segment_id)}: “${clean(e.quote)}”`)].filter(Boolean).join("\n\n");
}

export function meetingNotesReady(meetingId: number, transcription: { meeting_id: number; state: string }[], generation: { meeting_id: number; state: string; has_result: boolean }[]): boolean {
 const latest = generation.filter(job => job.meeting_id === meetingId).at(-1);
 return !!latest && latest.state === "completed" && latest.has_result && !generation.some(job => job.meeting_id === meetingId && ["queued", "processing", "retryable"].includes(job.state)) && !transcription.some(job => job.meeting_id === meetingId && job.state !== "completed");
}
export const meetingSections = ["Summary", "Action items", "Transcript"] as const;

/** Presentation-only empty detection. Never rewrite persisted AI or user data. */
export function meaningfulText(value: unknown): value is string {
 if (typeof value !== "string") return false;
 const text=value.trim().replace(/^[\s*#_•-]+|[\s*#_.:;!]+$/g, "").toLowerCase();
 if (!text || /^(?:n\/?a|none|nil|null|not available|not applicable|not stated|not identified|none identified|none mentioned|unknown|غير متوفر|لا يوجد|لم يذكر)$/.test(text)) return false;
 return !/^(?:no|none)(?: explicit| confirmed)? (?:attendees?|participants?|decisions?|action items?|actions?|follow[- ]?ups?|open questions?|unresolved questions?|discussion points?|substantive topics?)(?: (?:were |was |are |have been )?(?:identified|documented|found|mentioned|available|captured|recorded)(?: yet)?)?$/.test(text);
}
export function meaningfulPoints(points: Point[] | null | undefined): Point[] { return (points || []).filter(p=>meaningfulText(p?.text)); }
