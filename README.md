# Meeting Intelligence Workspace

An independent Windows desktop application using React, TypeScript, Vite, Tauri 2, and Rust. The approved gather UI includes Home, Note Taker, Workspaces, meeting details, and a project assistant. Meetings, workspaces, assignment, and task completion are persisted in local SQLite. Windows microphone and system-audio recording, pause/resume, durable WAV storage, recovery, and playback are implemented. AI processing is not connected; older illustrative notes and the explicitly simulated assistant remain available. Assistant conversations last for the current session.

## Windows setup

Install Node.js 22.12 or newer, Rust with the MSVC toolchain, Microsoft C++ Build Tools (Desktop development with C++ and a Windows SDK), and Microsoft Edge WebView2. See the [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). No global npm packages are required.

From the repository root:

```powershell
npm install
npm run desktop:dev
```

The desktop command starts Vite and opens the native window. Stop any active recording, close the window, and stop the terminal command with Ctrl+C when finished. Normal window closure is blocked while capture or finalization is active. Forced exit preserves available partial audio for startup recovery. `npm run dev` renders the frontend in a browser, but persistence requires the desktop host and displays a clear unavailable message there. If PowerShell blocks npm.ps1, use `npm.cmd` instead of changing execution policy.

## Checks and builds

```powershell
npm run typecheck
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
npm run desktop:build
```

Frontend output is written to `dist/`; Rust output is under `src-tauri/target/`. Installer bundling is disabled for this foundation. `desktop:build` builds the desktop executable without creating a release installer. After lockfiles exist, use `npm ci` and add `--locked` to Cargo checks for repeatable dependency resolution.

## Structure

```text
src/
  app/          Application composition and navigation
  components/   Shared presentation components
  pages/        Main views and meeting/workspace details
  data/         Types, desktop repository adapter, local state, simulations
  styles/       Approved visual design and responsive CSS
src-tauri/
  src/          Rust entry points, SQLite repositories, and recording/ WASAPI service/tests
  migrations/   Versioned SQLite schema
  tauri.conf.json
docs/           Architecture, decisions, and product scope
```

React presentation imports no desktop APIs. The dedicated repository adapter invokes a narrow Rust command; the frontend cannot submit SQL or database paths. Local data, environment files, and credential files are ignored; dependency lockfiles belong in source control.

The database starts empty at `%APPDATA%/dev.meetingintelligence.workspace/workspace.sqlite3` (resolved by Tauri). Create a workspace, then start a recording from Home or Note Taker. Each New Recording opens with an empty title and zero timer, unless capture is already active. Enter a title; Audio Settings lets you enable sources and choose devices (Windows defaults are recommended). Start, pause/resume if needed, then Stop & Save. Successful saving refreshes the meeting list and returns directly to Note Taker. Recovery appears contextually when retained audio needs attention. Review the saved meeting to assign it to a workspace and play each track. No transcript or summary is generated. Audio is stored separately under Tauri app_local_data_dir()/recordings (normally %LOCALAPPDATA%/dev.meetingintelligence.workspace/recordings), never in SQLite. Edit/delete controls are on detail screens. Deleting a workspace unassigns its meetings; no operation deletes recording files. See [Local data implementation](docs/LOCAL_DATA.md) for schema, migration, and persistence details.

See [Windows recording implementation and limitations](docs/WINDOWS_RECORDING.md) for recovery, audio device requirements, storage size, and manual validation. Captured audio is retained indefinitely; automatic retention and encryption are not implemented. Windows desktop microphone permissions must already allow capture; the app does not change system settings.

See [Architecture](docs/ARCHITECTURE.md), [Product scope](docs/PRODUCT_SCOPE.md), and [Durable processing proposal](docs/adr/0002-durable-meeting-processing.md) for the future direction.
