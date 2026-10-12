# ADR 0004: Independent durable meeting intelligence

Status: Implemented initial slice; provider quality and native end-to-end validation remain subject to testing.

The existing queue safely transcribes separate source recordings. Generation requires its own authorization, request/checkpoint budget, and results so failures or regeneration never repeat transcription or overwrite user data.

Use a dedicated Rust worker and SQLite generation queue, independent OpenAI-compatible chat settings, OS-backed credentials, immutable evidence snapshots, versioned validated JSON, stage checkpoints, and atomic result receipts. Minutes are rendered deterministically from the structured result. Suggested titles never override user titles. Attendee corrections and stable action completion are persisted separately from generation text.

Generation authorization is revision-bound and applies to future captures after a saved watermark. Existing transcripts require explicit user action. Provider changes stop old jobs before subsequent requests/result publication; already sent requests cannot be recalled. Three automatic/five lifetime attempts and 64 total requests bound each job. Bounded hierarchical analysis covers the entire input within supported limits and fails without truncation above them.

Rejected alternatives: UI-owned execution, a combined STT/generation retry lifecycle, free-text/Markdown as authoritative domain storage, raw malformed-output success fallback, inferred participant identity from audio tracks, automatic original-audio deletion, and mandatory Python/model infrastructure.

Trade-offs: separate tables/locks and result revisions add complexity; sequential generation limits throughput; character limits are not exact token budgets; exact source quotes validate traceability rather than semantic truth; regeneration preserves old completion history but does not infer task equivalence. The initial workflow handles one transcript revision at a time, not silently aggregated meetings. JSON object mode maximizes compatibility while local validation remains mandatory; strict JSON Schema is opt-in.

Remaining decisions: measured model quality/language thresholds, configurable token/context/deadline budgets, larger-meeting support, history browsing/task reconciliation, edited transcript versioning, customizable templates, formal Minutes review/export formats, and coordinated backup/retention policies. See [AI Meeting Intelligence](../AI_MEETING_INTELLIGENCE.md) for contracts, limits, and validation steps.
