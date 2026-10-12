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

The database starts empty at `%APPDATA%/dev.meetingintelligence.workspace/workspace.sqlite3` (resolved by Tauri). Create a workspace, then start a recording from Home or Note Taker. Each New Recording opens with an empty title and zero timer, unless capture is already active. Optionally enter a title; unnamed recordings use Untitled Meeting until successful AI analysis supplies a title. Manual titles and edits are preserved. Audio Settings lets you enable sources and choose devices (Windows defaults are recommended). Start, pause/resume if needed, then Stop & Save. Successful saving refreshes the meeting list and returns directly to Note Taker. Recovery appears contextually when retained audio needs attention. Review the saved meeting to assign it to a workspace and play each track. Transcription and meeting intelligence depend on separately configured and authorized providers. Audio is stored separately under Tauri app_local_data_dir()/recordings (normally %LOCALAPPDATA%/dev.meetingintelligence.workspace/recordings), never in SQLite. Edit/delete controls are on detail screens. Deleting a workspace unassigns its meetings; no operation deletes recording files. See [Local data implementation](docs/LOCAL_DATA.md) for schema, migration, and persistence details.

See [Windows recording implementation and limitations](docs/WINDOWS_RECORDING.md) for recovery, audio device requirements, storage size, and manual validation. Captured audio is retained indefinitely; automatic retention and encryption are not implemented. Windows desktop microphone permissions must already allow capture; the app does not change system settings.

See [Architecture](docs/ARCHITECTURE.md), [Product scope](docs/PRODUCT_SCOPE.md), and [Durable processing proposal](docs/adr/0002-durable-meeting-processing.md) for the future direction.

## Floating recording controls and optional meeting suggestions

During capture, a compact draggable widget stays above ordinary desktop windows, including while the main app is minimized. Pause/Resume and Stop & Save control the existing Rust recording session. Saving hides the widget; errors retain access to recovery.

Use the gear button → Recording preferences to opt into local automatic meeting detection (disabled initially). Likely Teams, Zoom, or Google Meet calls can trigger a suggestion. Start Recording opens setup for your title and audio choices; capture always requires your confirmation. Detection reads window identity and existing microphone-session metadata, never microphone audio. See [Desktop widget and detection](docs/DESKTOP_WIDGET.md) for signal limitations, privacy boundaries, and validation.

### Local transcription

Configure a trusted Windows `whisper-cli.exe` and compatible GGML model through Recording preferences → AI Processing → Transcription → Local Whisper. Recognition is local, requires no Python/cloud account, and runs through a persistent backend queue after recording finalization. Previously finalized recordings receive manual jobs at startup; automatic authorization does not upload older recordings. Jobs, retries, source checkpoints, and timestamped transcripts survive restart; original audio is retained. See [TRANSCRIPTION.md](docs/TRANSCRIPTION.md) for runtime setup, tests, and limitations. Structured notes and action items now follow transcription through the independent generation queue; workspace Q&A remains future work.

### Configurable AI providers

Recording Preferences → AI Processing supports Local Whisper or a user-configured OpenAI-compatible transcription API. Enable and save Automatically transcribe saved recordings to authorize future captures with the selected provider, including remote uploads. Older recordings stay manual; with automatic mode off, Transcribe requires a one-time remote confirmation after clicking. API keys use Windows Credential Manager; transcription and summarization settings/keys are separate. Connection tests are optional, explicit, and disclose possible API charges. Summarization uses a separate authorized chat-completion provider and durable generation worker. See [AI_PROVIDERS.md](docs/AI_PROVIDERS.md) for setup and compatibility.

## AI meeting intelligence

Configure **Recording Preferences → AI Processing → Summarization** with your own chat-completions URL, model and securely saved key. Select output language/JSON mode, review transcript sharing, enable **Automatically generate meeting intelligence**, and save before starting a new recording. Transcription and generation permissions are independent. New authorized transcripts produce structured notes in the background; existing transcripts require explicit Generate. Meeting Details includes source-linked notes, persistent actions, attendee corrections, regeneration, and deterministic Minutes with copy/Markdown export. Original audio, transcripts and user titles are preserved.

See [AI_MEETING_INTELLIGENCE.md](docs/AI_MEETING_INTELLIGENCE.md) for setup, supported limits, privacy boundaries and manual verification. No specific LLM is bundled or assumed.

Additional tests: `node --experimental-strip-types --test tests/intelligence.test.mjs`; Rust generation tests are included in `cargo test --manifest-path src-tauri/Cargo.toml`.

Meeting Details offers Summary, Action Items and Transcript. **Generate MoM** saves professional Markdown minutes directly to Downloads without another AI call or overwriting existing files. Recording recovery/retained audio is available through Recording Preferences and contextual recovery notices.
