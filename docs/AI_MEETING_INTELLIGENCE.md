# AI Meeting Intelligence

Implemented independently in Rust, React, and SQLite. No reference source, prompt, credentials, endpoint, or asset was transferred. Python and a fixed LLM model are not required.

## Setup and authorization

Recording Preferences → AI Processing → Summarization configures a separate OpenAI-compatible API base URL, chat model, and Windows Credential Manager key. Select English, Arabic, or Match transcript and JSON object or strict JSON Schema output. JSON object is the initial gateway compatibility default; all responses still undergo local schema/evidence validation. Strict JSON Schema requires provider support. The endpoint must accept `POST chat/completions`, `messages`, `response_format`, and `max_tokens`. Unsupported parameters fail visibly; no silent model/endpoint fallback exists.

Enable and save **Automatically generate meeting intelligence** to authorize transcript sharing and bounded retries with that specific configuration revision. This is separate from transcription/audio upload permission. It applies to captures started after saving, not existing transcripts or already registered captures. Provider/key/language/format changes require renewed review. Unchanged settings preserve the revision. Saving credentials alone enables no processing. The synthetic chat test is user-triggered, uses the saved generation format/language, contains no meeting data, and may incur charges.

Completed eligible transcription atomically registers its immutable evidence snapshot and generation job. Meeting Details never starts automatic processing. Missing generation authorization leaves transcription usable and allows an explicit manual Generate action. No per-recording confirmation appears for authorized automatic work. Full app exit stops workers; jobs reconcile at restart. A local server can be configured using a loopback HTTP URL; other destinations require HTTPS.

## Contracts and storage

Migration 0008 keeps recordings, transcripts, user titles, legacy meeting fields, and transcription checkpoints intact. Existing transcripts receive immutable revision snapshots and stable `transcript-job-id:segment-index` evidence IDs without scheduling uploads. New tables store independent generation preferences, jobs, validated checkpoints, versioned structured results, active result selection, stable action identities/completion, and user-confirmed attendee overrides. Source files are never deleted by intelligence processing.

Schema version 1 contains suggested title, executive summary, workstream topics with detailed points, decisions, actions, follow-ups, open questions, and evidence-supported attendees. Facts reference original segment IDs and exact quotes. Owners/deadlines are nullable and require separate evidence containing their verbatim wording; dates are not guessed or normalized. Mentioned names do not establish attendance; audio tracks are not people. Unsupported schema versions, missing fields, excessive output, nonexistent references, and quotes absent from source are rejected. Structural validation does not prove that every paraphrase is entailed, a person was present, or every material topic was retained; human review and model-specific quality evaluation remain necessary.

The user title is never replaced automatically. Suggested titles are displayed separately. Each recording transcript is analyzed separately; multiple recordings are not silently combined. Attendee corrections are meeting-level user assertions, remain separate from generated evidence, and survive regeneration. Regeneration creates a new immutable result; the previous successful result remains selected until replacement commits. Old action completion rows remain in SQLite and are not guessed onto newly generated actions. A history-selection UI and semantic task reconciliation are future work.

## Worker and recovery

A dedicated Rust worker owns an exclusive `intelligence.lock`, scans SQLite every 500ms, and claims jobs transactionally. States are queued, processing, retryable, failed, completed. Ownership fences checkpoint/result writes. Transient transport, throttling, and provider failures permit three automatic attempts with 20/40-second backoff and numeric Retry-After (maximum 900 seconds). Explicit Retry permits up to five lifetime job attempts. There are no adapter retry loops. Configuration/authentication/malformed/refused/truncated output failures require correction. Provider changes stop further requests and publication until the user reviews current settings and explicitly regenerates. In-flight requests cannot be recalled.

Validated stage checkpoints are keyed by request input, provider revision, output language/format, and pipeline version. Restart reclaims interrupted jobs only after acquiring the execution lock and reuses matching checkpoints. One transaction commits result, action IDs, selected-result pointer, and completion receipt. Older delayed jobs cannot replace a newer selected generation. Generation failures never discard an already committed transcript; retry/regeneration never call transcription. Provider-side exactly-once billing is not promised if a response is lost before checkpoint persistence.

## Long input and compatibility limits

Input is split at segment boundaries with Unicode-safe splitting of an unusually long segment, retaining its evidence ID. Initial chunks contain at most 10,000 text characters; at most 32 chunks/320,000 characters and 20,000 segments are supported. Intermediate findings are synthesized hierarchically with a 24,000-character serialized findings budget, at most five reduction levels, and a persistent 64-request cap across retries. No findings are dropped to fit a request. Oversized/nonreducing synthesis fails safely and retains the source. Empty input completes locally without a provider request.

The request reserves up to 4096 output tokens, a 120-second request timeout, a 15-second connect timeout, and a 1MiB response cap. Character budgets are conservative bounds, not exact model token accounting or a guarantee of context compatibility. Select a model with adequate context; small contexts or slower local servers may reject/time out. No raw-text fallback, reasoning-field fallback, tool execution, or unknown provider options are used. Every chunk/reduction checkpoint validates evidence. Coverage reports source segments processed, not a semantic completeness percentage. Very long meetings and complex oversized extracted findings can exceed this initial supported budget and remain actionable failures.

## UI and Minutes

Home/Note Taker show separate transcript and intelligence status. The generated read model also supplies Home task counts/completion and decisions plus library summary/topic search, without overwriting legacy meeting columns. Meeting Details preserves the pastel design and offers Summary, Highlights, Decisions, Action items, Transcript, and Minutes. Notes include collapsible workstreams, evidence navigation, unknown attendee/owner/deadline states, persistent task completion, attendee corrections, and explicit regeneration. Arabic output uses RTL presentation.

Minutes are a deterministic presentation of the saved structured result plus user-entered meeting metadata and confirmed attendee overrides: title/date/duration, attendees, overview, discussion, decisions, actions, follow-ups, and unresolved questions. Copy produces Markdown text with evidence; Export Markdown uses a native Windows Save dialog with overwrite confirmation. No second LLM request generates Minutes. Minutes remain AI-generated drafts. PDF/DOCX export, per-meeting chat, custom templates, editable generated prose, and workspace Q&A are outside this feature.

## Validation and manual checks

Automated tests use synthetic text, dummy credentials, and loopback HTTP only. They cover atomic automatic enqueue/exclusion of older recordings, duplicate prevention, structured/evidence validation, missing owners/deadlines/attendees, authorization independence, provider changes, checkpoint reuse across restart, transient recovery, stale ownership, completion rollback, regeneration/history preservation, migration from schema 7, Unicode long-input planning, deterministic Minutes, Markdown escaping, and RTL selection. A real Rust worker runs against a mock chat endpoint without any Meeting Details interaction. Existing recording/transcription tests remain active.

Manual desktop checks:

1. Use an isolated debug profile (`MIW_DEV_DATA_DIR` absolute path) and local mock/synthetic provider. Configure independent transcription and summarization providers; save each automatic preference. Reopen Preferences to verify saved states. Synthetic connection test should exercise the selected chat format.
2. Record known speech, Stop & Save, remain on Note Taker: verify transcription then analysis status without Generate/dialog/navigation. Open details after completion and check all sections/source links.
3. Complete a task, correct attendees, restart: verify results, completion, corrections, title, transcript and playback persist without duplicate jobs/requests.
4. Regenerate: keep the old result visible while processing, preserve confirmed attendees and old completion history, avoid another STT request. Change provider/format: confirm renewed review and no continued old-provider requests.
5. Test Arabic/mixed-language layout, small Windows window sizes, keyboard tabs, source focus, clipboard, native Save dialog cancellation/overwrite/Markdown content.
6. Inject 401, 429, timeout, malformed JSON, invalid quote, missing timestamps, empty transcript, long late-topic input, and restart during extraction/synthesis. Failures must retain audio and transcript; Retry appears only on failed jobs.

No real external generation requests or user meeting data have been used by development validation. Model accuracy, the user's selected gateway, native clipboard/export behavior, and full record-through-generation desktop workflow must be explicitly verified before claiming production compatibility.

### Development validation results

- 67 Rust tests passed with one opt-in real Whisper test excluded in the standard run; that real local Whisper/checkpoint regression also passed separately; four Minutes/RTL tests and seven recording-flow tests passed. TypeScript, production frontend build, Rust checks/formatting, and desktop executable build passed at the validation checkpoints; final checks include the generated read-model integration.
- A native Windows app launched against an ignored isolated database containing only independently generated synthetic fixtures and no configured provider. Structured Summary and Minutes displayed, including Arabic RTL content; Copy reported success. The native Markdown Save dialog opened, but a desktop-automation geometry error prevented confirming a completed export in that attempt. Export cancellation/overwrite and end-to-end recording → real transcription → generation remain manual checks.
- Synthetic backend tests exercised the actual Rust generation worker, Windows dummy credential fixture, loopback HTTP, SQLite receipts/restart persistence, and checkpoint reuse without Meeting Details calls. No real recordings/transcripts were transmitted externally. The standalone reference repository was not read or modified during implementation.

## Changed-file inventory

Created:

- `src-tauri/migrations/0008.sql`
- `src-tauri/src/intelligence/mod.rs`
- `src-tauri/src/intelligence/contract.rs`
- `src-tauri/src/intelligence/chat.rs`
- `src-tauri/src/intelligence/worker.rs`
- `src-tauri/src/intelligence/export.rs`
- `src-tauri/src/intelligence/tests.rs`
- `src/components/MeetingIntelligence.tsx`
- `src/data/intelligence.ts`
- `tests/intelligence.test.mjs`
- `docs/AI_MEETING_INTELLIGENCE.md`
- `docs/adr/0004-meeting-intelligence.md`

Updated:

- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/processing/mod.rs`
- `src-tauri/src/processing/settings.rs`
- `src-tauri/src/processing/ai_tests.rs`
- `src-tauri/src/storage.rs`
- `src-tauri/src/storage/tests.rs`
- `src/components/AIProcessingPreferences.tsx`
- `src/components/MeetingRow.tsx`
- `src/components/Transcription.tsx`
- `src/data/processing.tsx`
- `src/data/types.ts`
- `src/pages/HomePage.tsx`
- `src/pages/MeetingDetail.tsx`
- `src/pages/NoteTakerPage.tsx`
- `src/styles/global.css`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/AI_PROVIDERS.md`

Ignored test profiles/build output are outside this source inventory. No changes were staged or committed.
