# Meeting Intelligence Workspace

An independent Windows desktop application foundation using React, TypeScript, Vite, Tauri 2, and Rust. Dashboard, Meetings, Projects, and Settings currently contain placeholders. Recording, AI processing, persistence, and project intelligence are not implemented.

## Windows setup

Install Node.js 22.12 or newer, Rust with the MSVC toolchain, Microsoft C++ Build Tools (Desktop development with C++ and a Windows SDK), and Microsoft Edge WebView2. See the [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). No global npm packages are required.

From the repository root:

```powershell
npm install
npm run desktop:dev
```

The desktop command starts Vite and opens the native window. Close the window and stop the terminal command with Ctrl+C when finished. For frontend-only work, use `npm run dev`; no desktop APIs are required by the presentation components. If PowerShell blocks npm.ps1, use `npm.cmd` instead of changing execution policy.

## Checks and builds

```powershell
npm run typecheck
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
npm run desktop:build
```

Frontend output is written to `dist/`; Rust output is under `src-tauri/target/`. Installer bundling is disabled for this foundation. `desktop:build` builds the desktop executable without creating a release installer. After lockfiles exist, use `npm ci` and add `--locked` to Cargo checks for repeatable dependency resolution.

## Structure

```text
src/
  app/          Application composition and navigation
  components/   Shared presentation components
  pages/        Four placeholder pages
  styles/       Shared CSS
src-tauri/
  src/          Rust desktop entry points
  tauri.conf.json
docs/           Architecture, decisions, and product scope
```

React presentation imports no desktop APIs. Privileged desktop integration belongs in `src-tauri`; future frontend adapters should remain separate from presentation components. No native commands or plugin permissions are added yet. Local data, environment files, and credential files are ignored; dependency lockfiles belong in source control.

See [Architecture](docs/ARCHITECTURE.md), [Product scope](docs/PRODUCT_SCOPE.md), and [Durable processing proposal](docs/adr/0002-durable-meeting-processing.md) for the future direction.
