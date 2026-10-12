# Floating recording widget and local meeting suggestions

## Implemented boundaries

A single native Tauri window (`recording-widget`, 340 × 240 logical pixels) is created hidden, independent of the main window. It is undecorated, always on top of ordinary windows, draggable from its header, and omitted from the taskbar. A Rust-owned coordinator observes the existing recorder every 500 ms and shows recording/paused/saving controls after capture starts successfully. It hides the window after a successful save. Capture/finalization failures retain a visible recovery entry point; opening recovery restores the main app. Closing a suggestion never issues a recording control command.

The widget uses the existing `recording_status` and `recording_control` contracts. There is one capture worker and one recording identity, regardless of how many webviews display controls. A Rust Stop latch, serialized by the existing session mutex, makes repeated Stop requests idempotent even if the worker already consumed the channel message. The latch is reset only for the next recording. Main and widget timers read the recorder's captured duration, including its existing pause exclusions. Normal main-window close and application exit are blocked during capture/finalization; inactive main-window close exits both windows. Interrupted-recording recovery, WAV files, playback, and recording/meeting relationships are unchanged.

During active recording, pause, and saving, the window uses a compact 280 × 140 logical-pixel layout, with only normal padding below the controls. Suggestions and recovery retain the roomier 340 × 240 layout. Additional recording warnings remain accessible through scrolling.

## Optional detection

Detection is **off by default**. Enable or disable it through the gear button → Recording preferences → Automatic meeting detection. SQLite migration 0003 stores only this boolean preference. While enabled and not recording, a background thread checks every five seconds. Two consecutive qualifying scans are required before a suggestion appears (roughly 5–10 seconds after identification). No microphone stream is opened for detection; WASAPI session enumeration reads activity state and process identity metadata only.

All suggestions are **heuristic**. Confirmed observations such as a visible window or Windows reporting an active microphone session do not confirm meeting attendance, sound, consent, or a particular tab's microphone use.

| Application | Qualifying signals | Limitations |
| --- | --- | --- |
| Microsoft Teams | A visible window owned by `ms-teams.exe` or `teams.exe`, plus an active capture session attributed to that executable | Calls, device tests, or voice-message capture can qualify. Some versions attribute audio to a helper process, which can be missed. A running process alone never qualifies. |
| Zoom | A visible `zoom.exe` window plus attributed microphone activity, or an exact `Zoom Meeting`/`Zoom Webinar` title | The exact-title fallback also works for muted meetings, but is an English window-title heuristic. Localized/version-specific titles may be missed. An idle Zoom Workplace window alone never qualifies. |
| Google Meet in Chrome/Edge | A visible browser window whose active-tab title starts with `Meet -`, `Meet –`, or `Meet —` and a standard call code, plus an active microphone session attributed to the same browser executable | Background tabs, customized titles, and helper-owned sessions may be missed. Activity is correlated at browser-executable level, not at tab level; another tab or browser instance using the microphone can cause a false positive, including on a pre-join page. |

Unavailable microphone-session metadata results in fewer detections; only Zoom's exact meeting-window fallback can still qualify. Unsupported applications produce no suggestion. Detection is local and works only while the application process runs; it is not a Windows service, startup task, browser extension, or calendar integration.

## Suggestions, suppression, and privacy

The shared widget asks, “Meeting detected. Would you like to record?” and identifies the app and heuristic basis. Start Recording restores the main window and opens the existing fresh setup form, with title and audio settings for user review. It does **not** start capture. Dismiss suppresses that window identity. When capture starts, the same native window changes to recording controls; no second window or capture process is created.

Suppression survives muting while the identified window remains present. Identified windows are also suppressed during/after manual recording, preventing a fresh prompt for the just-recorded call. Suppression is in memory; closing the relevant window, changing the recognized Meet call code, restarting the app, or explicitly toggling detection off/on can allow a later prompt. Teams/Zoom windows reused across different calls can therefore suppress a later call until their window disappears. This favors avoiding duplicate prompts over detecting every call.

Only supported applications' top-level titles are read transiently for classification. Titles, call codes, process paths, and activity observations are not logged or persisted. Suppression stores opaque in-memory keys derived from window/process identity and, for Meet, its call code. No tab contents, microphone samples, meeting content, credentials, network requests, or company integrations are used.

## Validation commands

```powershell
npm.cmd run typecheck
npm.cmd run build
node --experimental-strip-types --test tests/recording-flow.test.mjs
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked
```

Native checks should cover repeated recordings, minimized main-window controls, dragging, simultaneous Stop requests, disabled detection, idle supported apps, suggestion dismissals, setup handoff, failure recovery, and safe close. Actual Teams/Zoom/Meet calls, localized titles, unavailable session metadata, live audio-device changes, storage failures, and multiple monitor/scaling configurations require separate manual verification. A synthetic window fixture can validate the window-title heuristic and widget actions, but does not establish compatibility with real meeting clients.

## Validation performed for this change

- TypeScript checks, production frontend build, locked Rust compilation checks, Rust formatting, and Git whitespace checks passed.
- All 32 Rust unit tests and all 7 recording-flow frontend tests passed. These cover detection classification/debounce/suppression, widget mode selection, persisted opt-in preferences, migration preservation, idempotent Stop, and existing recording finalization/recovery/storage behavior.
- Native Windows microphone recordings verified automatic widget appearance, synchronized timers, widget Pause, main-window minimization, native dragging, widget Stop & Save, widget hiding, immediate Note Taker list refresh, and consecutive recordings reusing the same widget with a fresh setup form.
- A synthetic local executable/window fixture verified the Zoom exact-title heuristic, suggestion wording, Dismiss, duplicate suppression across multiple scans, disabled detection, and Start Recording opening an empty 00:00 setup form without starting capture. An idle fixture window and running idle Teams/browser applications did not produce a suggestion. This fixture did not use Zoom services or capture audio.
- Main-window close during capture was blocked with a warning while recording continued. Inactive close exited both app windows. The owned development server and synthetic fixture were stopped after validation; detection was disabled in the test launch's profile. Test recordings remain in application storage, outside Git.

Real Teams/Zoom/Meet calls were not joined or tested. Forced process termination, storage/device failure injection, playback regression, and multiple-monitor/scaling checks were not repeated in this change; automated recovery checks passed. The tool-managed development launch used a different application-data context from the earlier user launch, so these observations do not establish a same-profile native restart test. Browser microphone attribution is coarse, helper processes and localized titles can be missed, and suggestions remain heuristic.
