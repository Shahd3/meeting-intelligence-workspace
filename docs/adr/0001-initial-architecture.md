# ADR 0001: Initial Architecture

Status: Proposed — awaiting review and feasibility validation.

## Context and problem

**Confirmed requirements:** Meeting Intelligence Workspace is an independent single-user Windows desktop learning project for recording meetings, producing AI summaries/action items, organizing projects, and answering questions across meetings. It should permit potential future host integration without depending on company infrastructure or artifacts.

The architecture must connect privileged audio/file operations, durable local records, and potentially slow AI work while keeping intelligence services reusable. The brief does not settle audio sources, AI providers, offline operation, privacy constraints, or retrieval quality targets. This ADR proposes a direction; it does not authorize implementation or finalize unspecified technologies.

## Proposed architecture

Use a modular desktop application: React/TypeScript presentation, a Tauri Rust host for privileged desktop operations, application use cases and domain contracts, replaceable intelligence adapters, and SQLite repositories with separate audio files. Persist job state to support recovery and retries. Keep provider credentials outside the frontend and validate IPC inputs and external outputs at runtime.

Separate recording, transcription, generation, and retrieval so each can fail or be replaced independently. Project-scoped answers should cite persisted transcript evidence and expose insufficient evidence. These are logical boundaries, not a proposal to deploy microservices. See [ARCHITECTURE.md](../ARCHITECTURE.md) for diagrams and responsibilities and [PRODUCT_SCOPE.md](../PRODUCT_SCOPE.md) for scope.

**Assumptions:** One local Windows user owns the workspace; projects are logical groups. Persisting records locally is proposed, while local versus hosted AI remains undecided.

## Technology rationale

### React

**Proposal:** Use React for composable meeting, project, review, and question views. Its component approach supports a UI with shared controls and changing processing state. React is a presentation library; workflows and provider calls should not be coupled to component lifecycles. State management, styling, and build tooling need not be chosen in this ADR. See the official [React introduction](https://react.dev/learn).

### TypeScript

**Proposal:** Use TypeScript for UI code and typed frontend contracts to make refactoring and data-shape mistakes easier to detect. Types are compile-time checks, so runtime validation remains necessary at IPC, persistence, and provider boundaries. Rust-side contracts must also stay compatible. See the [TypeScript Handbook](https://www.typescriptlang.org/docs/handbook/intro).

### Tauri

**Proposal:** Use Tauri for the desktop shell and a Rust core for privileged integration. It allows a web UI with controlled native capabilities and uses Windows WebView2 rather than bundling a browser engine. This fits an initially Windows-focused app, subject to a capture and packaging spike. See Tauri's [process model](https://v2.tauri.app/concept/process-model/).

Trade-offs include learning Rust, maintaining IPC contracts, testing the WebView2 runtime, and verifying suitable audio integration. Tauri does not automatically solve system-audio capture or provide a Node.js backend runtime. The language/runtime for reusable intelligence orchestration remains unresolved; it could live in Rust or an explicitly packaged separate runtime based on validated needs.

### SQLite

**Proposal:** Use embedded SQLite for structured local data because a single-user application does not initially need a managed database server. Relations and transactions suit meeting/project membership, transcripts, output provenance, and jobs. SQLite is commonly used as an [application file format](https://www.sqlite.org/appfileformat.html); its [appropriate-use guidance](https://www.sqlite.org/whentouse.html) describes where a server database is preferable.

Keep audio in app-managed files with database references. Trade-offs include migration management, serialized writes, artifact/database consistency, and coordinated backups. SQLite alone does not settle encryption, semantic retrieval, or cloud synchronization. No vector extension or database library is selected.

### Is Python required for AI processing?

**Proposal:** Do not require Python by default. Hosted transcription/generation can be invoked through a suitable HTTP adapter; local inference may be available through native engines or separate runtimes. The chosen engine's actual integration requirements must determine this choice.

**Conditional possibility:** If a validated local model requires a Python ecosystem library, package an isolated worker behind an intelligence adapter. Assess distribution size, interpreter/model installation, process lifecycle, IPC, resource usage, and upgrades first. Python is neither mandatory nor prohibited. No Python framework, model, provider, or sidecar is selected.

## Key alternatives considered

| Alternative | Why not the current proposal / when to reconsider |
| --- | --- |
| Electron with React/TypeScript | Provides a bundled Chromium and Node.js environment but adds runtime distribution overhead. Reconsider if capture libraries or orchestration substantially benefit from Node.js. |
| Native Windows UI such as .NET/WPF or WinUI | May suit deep Windows integration; introduces a different UI stack and reuse path. Reconsider if capture feasibility or native requirements dominate. |
| Browser-only application | Avoids desktop packaging, but browser permission/lifecycle and audio-source constraints need validation against recording requirements. |
| Server database and hosted backend | Useful for future collaboration or centralized jobs; adds operations, identity, and network dependence outside the initial single-user scope. |
| Required Python service | Useful if an AI engine requires it, but premature without engine selection and creates packaging/process obligations. |
| Embeddings and dedicated vector store immediately | Could improve semantic recall, but adds indexing, model, storage, and migration choices before retrieval quality is measured. |

These are preliminary comparisons, not claims that the alternatives are infeasible or benchmarked.

## Trade-offs and risks

- A modular application is simpler to run than distributed services, but boundaries require discipline to remain reusable.
- Hosted AI may reduce local hardware requirements but introduces disclosure, connectivity, cost, latency, and provider-policy concerns. Local AI introduces model distribution and hardware constraints. Neither option is approved here.
- Audio capture and Windows permissions are major feasibility risks. Validate source combinations, interruptions, device changes, and long recordings before relying on a specific backend.
- Long transcripts require bounded context and retrieval. Generated summaries, action items, and answers can omit or invent facts; provenance and human review help but do not guarantee correctness.
- File writes and database commits are separate operations. Recovery states, migrations, stale-index handling, and deletion/backup semantics require explicit design.
- Compile-time types do not secure IPC or establish output correctness. Apply narrow desktop capabilities and runtime validation; keep transcript content separate from executable instructions.
- Future multi-user integration requires identity, authorization, and isolation decisions beyond this design.

## Decisions that remain unresolved

Before implementation, review:

1. Required audio sources: microphone, Windows system audio, or both; recording limits, formats, and device handling.
2. Local versus hosted AI; acceptable data disclosure, offline expectations, budget, latency, supported languages, and target hardware.
3. Provider/model selection, orchestration language/runtime, adapter protocols, and whether any engine requires Python.
4. Retrieval method, chunking, index storage, citation granularity, quality targets, and behavior with incomplete transcripts.
5. Privacy and lifecycle: credentials, encryption, retention/deletion, user corrections, backup/export, and recording consent UX.
6. Logical folder semantics, meeting membership cardinality, and whether transcript editing is required in the MVP.
7. Job recovery/cancellation policy, packaging prerequisites, schema/migration approach, and supported Windows versions.
8. Any actual future integration contract or host constraints; none are assumed or selected.

## Validation and acceptance path

Review the scope and assumptions with the project owner. After approval to begin implementation, run small synthetic-data feasibility spikes for Windows capture/Tauri packaging, provider adapter/runtime integration, and database/artifact recovery. Evaluate transcription, action extraction, and project questions against known answers, including unrelated projects and no-evidence cases. Measure latency, cost, and hardware usage with representative meeting lengths before selecting models or retrieval infrastructure.

Acceptance of this ADR records an agreed direction; unresolved choices should be documented in follow-up ADRs before dependencies are introduced. [ADR 0002](0002-durable-meeting-processing.md) refines durable processing, recovery, and retention proposals without selecting an AI runtime or provider. This documentation task does not perform those spikes or accept the proposal automatically.
