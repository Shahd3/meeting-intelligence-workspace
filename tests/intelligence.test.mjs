import test from "node:test";
import assert from "node:assert/strict";
import { direction, minutes, intelligenceLabel, meetingNotesReady, meetingSections, meaningfulText, meaningfulPoints } from "../src/data/intelligence.ts";
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

test("Ready requires persisted notes and rejects pending/failed processing", () => {
 const stt=[{meeting_id:1,state:"completed"}]; const result=[{meeting_id:1,state:"completed",has_result:true}];
 assert.equal(meetingNotesReady(1,stt,result),true);
 assert.equal(meetingNotesReady(1,stt,[{...result[0],has_result:false}]),false);
 for(const state of ["queued","processing","retryable","failed"]){assert.equal(meetingNotesReady(1,[{meeting_id:1,state}],result),false);assert.equal(meetingNotesReady(1,stt,[{...result[0],state}]),false);}
 assert.equal(meetingNotesReady(1,stt,[...result,{meeting_id:1,state:"failed",has_result:false}]),false);
 assert.equal(meetingNotesReady(1,stt,[{meeting_id:1,state:"retryable",has_result:false},...result]),false);
 assert.deepEqual(meetingSections,["Summary","Action items","Transcript"]);
});

test("Optional subsections hide empty content without changing stored points", () => {
 for (const text of [null, undefined, "", "  ", "None.", "N/A.", "No explicit decisions were documented.", "No attendees identified.", "No follow-ups documented."]) assert.equal(meaningfulText(text), false, String(text));
 for (const text of ["We decided not to launch.", "No new contracts until review.", "قرار مؤكد"]) assert.equal(meaningfulText(text), true);
 const source=[point("None."),point("Agreed to test")];
 assert.deepEqual(meaningfulPoints(source),[source[1]]);assert.equal(source.length,2);assert.deepEqual(meaningfulPoints(null),[]);
});
