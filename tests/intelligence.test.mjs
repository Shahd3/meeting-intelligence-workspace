import test from "node:test";
import assert from "node:assert/strict";
import { direction, minutes, intelligenceLabel } from "../src/data/intelligence.ts";
const point = (text) => ({ text, evidence: [{ segment_id: "transcript:0", quote: "Explicit test statement" }] });
const result = { id: "result", transcript_id: "transcript", notes: { schema_version: 1, suggested_title: point("Suggested title"), executive_summary: point("Overview"), topics: [{ heading: "Late workstream", points: [point("Specific point")] }], decisions: [point("Agreed outcome")], action_items: [{ description: point("Test functionality"), owner: null, owner_evidence: [], deadline: null, deadline_evidence: [] }], follow_ups: [], open_questions: [point("Unresolved proposal")], attendees: [] }, coverage: { segment_count: 1, analyzed_segments: 1, chunk_count: 1, source_warning: null }, actions: [] };
test("Minutes are deterministic, preserve user title, and do not invent missing details", () => {
 const meeting = { title: "User-provided title", date: "2026-10-12", duration: "01:00" };
 const document = minutes(meeting, result, null);
 assert.equal(document, minutes(meeting, result, null));
 assert.match(document, /^# User-provided title/);
 assert.match(document, /Attendance not established/);
 assert.match(document, /Not assigned/); assert.match(document, /Not stated/);
 assert.match(document, /Late workstream/); assert.match(document, /Unresolved proposal/);
 assert.match(document, /transcript:0/);
});
test("Attendee corrections replace generated presentation without mutating results", () => {
 const document = minutes({ title: "Meeting", date: "", duration: "" }, result, ["Confirmed attendee"]);
 assert.match(document, /Confirmed attendee/); assert.equal(result.notes.attendees.length, 0);
});
test("Arabic uses RTL and normal English stays LTR", () => {
 assert.equal(direction("ملخص الاجتماع"), "rtl"); assert.equal(direction("Meeting overview"), "ltr");
 assert.match(minutes({title:"اجتماع",date:"",duration:""}, {...result,notes:{...result.notes,executive_summary:point("ملخص الاجتماع")}},null), /ملخص الاجتماع/);
});
test("Minutes escape Markdown and action-table separators", () => {
 const document=minutes({title:"<script>| unsafe",date:"",duration:""},result,null);
 assert.ok(!document.includes("# <script>")); assert.ok(document.includes("\\|"));
 assert.equal(intelligenceLabel("retryable"),"AI analysis retry scheduled");
});
