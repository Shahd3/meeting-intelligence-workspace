# Local data implementation

This document describes implemented local persistence, not the future processing system proposed in the architecture ADRs.

## Boundary and storage

Rust owns SQLite through `rusqlite` with bundled SQLite. React calls the dedicated `src/data/repository.ts` adapter; only a typed CRUD/task request crosses IPC. SQL and filesystem paths cannot be supplied by the frontend. The Rust repository is independent of Tauri; the command adapter resolves Tauri's per-user app data directory and runs blocking database work off the UI thread.

The file is `workspace.sqlite3` inside `app_data_dir()`; on Windows, normally `%APPDATA%/dev.meetingintelligence.workspace/`. The app creates the directory and database on first data access. No prototype fixtures are automatically inserted. Prior in-memory sessions cannot be recovered.

## Schema and consistency

Migration `src-tauri/migrations/0001.sql` runs transactionally at schema version zero and sets `PRAGMA user_version=1`. Reopening version one does not seed or reset records; newer unsupported versions are rejected. Future schema changes need additional versioned migrations and preservation tests.

- `workspaces`: stable auto-increment ID, name, description, color and display metadata.
- `meetings`: stable auto-increment ID, title, nullable workspace ID, display date/duration, summary, decision and JSON arrays of highlights/actions/participants. These artifacts are still simulated. Newest-first means creation ID order, not a parsed meeting date.
- `workspace_completion` and `meeting_completion`: foreign-key-owned task completion. Meeting action positions currently identify tasks; replacing an action list clears its completion to avoid applying checks to different tasks.

Foreign keys are enabled per connection. `ON DELETE SET NULL` preserves meetings when their workspace is deleted. The UI maps NULL to its existing zero/unassigned value. Completion owned by a deleted record is cascaded; meeting completion survives workspace deletion. Recording files are never touched by these operations, including meeting deletion.

Mutations and returned snapshots share one transaction; UI caches are replaced only after commit. Bound parameters, validated nonempty names/titles, color allowlists, bounded fields/arrays and safe IDs constrain writes. WAL, full synchronization and a five-second busy timeout support local durability and contention. Failed writes roll back and show a sanitized error with reload available; forms remain open. There is no silent mock fallback or destructive database recovery.

## UI and current limits

Home, Note Taker and Workspaces read one persistent snapshot. Detail views expose editing/deletion and meeting reassignment. Deletion requires an explicit confirmation dialog. Loading, empty, saving and error states reuse the existing visual style. Task completion persists; navigation, search, dialog state and simulated assistant history are ephemeral. Mutations clear assistant history to avoid retaining stale project citations.

This implementation targets one user and one active window. It loads all records; pagination and multi-window refresh/conflict resolution are not implemented. IDs from deleted rows are not reused. No real capture, transcripts, providers, job queue, recording catalog, cloud service, encryption or backup UI is implemented. SQLite is not encrypted. For a manual backup, fully exit the app first and copy the database and any remaining WAL/SHM sidecars together; a future recording catalog will require a coordinated artifact backup.

## Validation

Run TypeScript/build checks, `cargo check`, `cargo fmt -- --check`, and `cargo test --lib`, using the manifest path in the README. Repository tests cover CRUD, literal SQL-like input, invalid writes/rollback, assignment/moves, deletion relationships, completion cleanup, recording-file preservation, reopening, repeated initialization and unsupported schema preservation.

Desktop restart check: create a uniquely named workspace, save a simulated meeting assigned to it, fully stop the app and dev process, then launch again. Verify the Home counts, meeting metadata and workspace assignment remain. Test artifacts use synthetic text only. Browser previews intentionally cannot access SQLite.

API references: [rusqlite connection](https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html), [Tauri command boundary](https://v2.tauri.app/develop/calling-rust/), [Tauri per-user paths](https://docs.rs/tauri/latest/tauri/path/struct.PathResolver.html).
