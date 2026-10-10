# Product Scope

Status: Proposed MVP boundaries for review.

## Classification

**Confirmed requirement** means supplied by the project brief or README. **Proposed decision** means recommended but awaiting review. **Assumption** means a premise needing confirmation. **Future possibility** means an optional direction without an initial delivery commitment. The [architecture overview](ARCHITECTURE.md) and [initial ADR](adr/0001-initial-architecture.md) use the same labels.

## Product goals

**Confirmed requirements:** A single-user Windows desktop application that records meetings, generates AI summaries and action items, organizes meetings into project folders, and answers questions across multiple meetings. Reliable processing/recovery and folder-level AI question answering are MVP requirements. Development should teach professional engineering practices. The intelligence core must remain independent of the Windows desktop shell for possible future host integration.

## Initial MVP functionality

The core product goals are confirmed; the following delivery details are **proposed**:

| Capability | Proposed initial behavior | Review or validation example |
| --- | --- | --- |
| Recording | Explicit start/stop controls, visible recording state, saved audio, and clear capture failures. Source support remains open. | A stopped recording remains playable after restarting the app; interrupted capture exposes its recovery status. |
| Meeting library | Save and reopen meeting metadata, recording references, transcripts, and generated outputs locally. | Restart preserves a meeting and its processing state. |
| Transcription | Process a saved recording and expose progress and failure/retry behavior. | A failed provider call retains the recording and can be retried without duplicating results. |
| Reliable processing and recovery | Preserve audio progressively; use backend-owned durable jobs, checkpoints, bounded retries, and restart reconciliation. | Later-stage failure reuses successful checkpoints; closing the UI does not cancel backend work. |
| Summaries, action items, and decisions | Generate a summary and structured facts with transcript evidence; show outputs for user review. Unknown owners/dates stay unknown. | Compare outputs with a synthetic transcript containing known decisions, tasks, and absent deadlines. |
| Project folders | Create named logical project groups and assign meetings; membership rules remain open. | A project view lists its assigned meetings consistently after restart. |
| Project questions | Answer across available transcripts in a selected project with source references and insufficient-evidence feedback. | A question spanning two meetings cites relevant passages and excludes unrelated project material. |

A usable MVP must make recording and processing failures visible and preserve successful work. Citation links to transcript passages are proposed; timestamp playback depends on engine output. Transcript correction, output editing, answer history, and deletion UX need review before inclusion. These examples guide later acceptance tests and do not claim current implementation.

## Engineering acceptance criteria

The following are **proposed measurable criteria** for the confirmed reliability and folder-question requirements; none represents verified implementation. [ADR 0002](adr/0002-durable-meeting-processing.md) defines the proposed mechanisms.

- **Capture recovery:** Interrupt capture and restart. Preserve original partial audio, classify recoverability, and identify any incomplete source. A normally stopped recording is valid and playable after restart while retained; crash-loss tolerance must be agreed and measured.
- **Finalization recovery:** Interrupt between file finalization and database registration. Reconciliation produces one finalized recording/job pair or a visible recovery-required outcome; it never silently loses valid audio or queues an active writer.
- **UI independence:** Unmount or close the UI while the backend stays alive; processing continues. Exit the entire app and restart; durable jobs resume safely without claiming continuous execution during shutdown.
- **Retries:** Simulate transient errors and corrected permanent errors. Enforce agreed per-stage limits/backoff, retain source audio, show safe actionable status, and reuse unchanged successful checkpoints. Manual retry cannot bypass configured limits silently.
- **Idempotency:** Repeat enqueue and simulate lost responses after persistence commit. Store one logical result for the operation, and do not repeat generation when a valid checkpoint exists. Stale workers cannot commit after ownership changes.
- **Traceability:** Every generated action/decision and factual answer citation resolves to an eligible meeting and current transcript segment/span. Preserve available timestamps; absent timestamps are explicit. Reject nonexistent/stale references and validate factual support using synthetic ground truth.
- **Project isolation:** With two projects containing similar terms, select one and confirm retrieval, generation context, and answer citations exclude the other. Reassign/delete a meeting and verify stale indexes cannot reintroduce it; recheck membership before returning the answer.
- **No evidence:** Ask an unsupported question and receive insufficient-evidence feedback rather than an invented answer. Partial transcript coverage is visible.
- **Retention:** Test the selected policy and interrupted deletion. Success alone does not delete audio; incomplete jobs remain protected. Removed audio disables playback clearly while transcript citations remain valid; full meeting deletion invalidates derived indexes.

Policy values and quality thresholds require owner approval and feasibility tests before implementation acceptance.

## Single-user assumptions

- **Confirmed:** Initial operation is single-user on Windows.
- **Assumption:** One local Windows profile owns one workspace; application accounts and roles are unnecessary initially.
- **Assumption:** Project folders are logical groupings stored in the application. Nested folders, multiple membership, and filesystem mirroring are not assumed.
- **Proposed decision:** Keep saved meeting records and audio local, with SQLite for structured data and separate audio files. This does not guarantee offline AI or prohibit external processing; provider locality and permission to send recordings/transcripts remain unresolved.
- **Assumption:** The user initiates recording and reviews generated content. Recording consent UX and use policies require review before real-world use.

## Future intelligence features

**Confirmed follow-on priorities:** Cross-meeting decision evolution, project timelines, and advanced project intelligence are important after the MVP. Their precise scope and acceptance targets remain open.

**Future possibilities** for those priorities and other enhancements, subject to separate requirements and evaluation:

- Semantic retrieval using embeddings, better reranking, and richer source navigation.
- Speaker diarization and speaker labeling.
- Decisions, commitments, and unresolved-question tracking across meetings.
- Project timelines, topic trends, contradictions, and changes in decisions.
- Follow-up suggestions and action-item lifecycle tracking.
- Questions spanning explicitly selected projects and additional document sources.
- Local/offline inference if it meets privacy, hardware, and quality needs.

None of these possibilities selects a provider, model, framework, or vector database. Basic project question answering is part of the confirmed product direction; advanced retrieval methods remain optional choices.

## Future integration considerations

**Confirmed requirement:** Potential future integration must not create a current dependency on company infrastructure. Core processing and project-query contracts must not import Windows capture or desktop UI concerns.

**Proposed decision:** Define reusable use cases and data contracts around recording artifacts, transcripts, summaries, action items, project queries, citations, and structured errors. Keep desktop capture, UI, storage, provider credentials, and host identity behind replaceable adapters. Use independent synthetic examples; no company artifacts are needed.

**Future possibilities:** An integration adapter could expose these use cases through an agreed API or embedded module, map external project IDs, or replace local persistence and AI providers. Transport, authentication, tenant isolation, data ownership, and synchronization require explicit future agreements. No current endpoint or integration contract is assumed.

## Questions requiring owner review

Agree on audio-source support, offline/privacy expectations, language and hardware targets, meeting lengths, AI cost/latency budgets, folder membership, editing requirements, and retention/deletion/backup behavior. Then validate capture feasibility and intelligence quality with synthetic meetings, cross-meeting questions, project-isolation cases, and absent-evidence questions. Architecture proposals remain open until that review and the relevant feasibility checks.
