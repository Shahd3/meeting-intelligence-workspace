# Configurable AI providers

Implemented: local/remote transcription settings and a remote transcription adapter; independent future summarization configuration. Summarization execution remains outside this feature. Reference-product endpoints/models/credentials are unverified and were not accessed or copied.

## Recording Preferences → AI Processing

**Transcription:** choose Local Whisper or Remote API. Local keeps the existing configured whisper.cpp executable, GGML model, language, fingerprints, and checkpoint behavior. The last saved local paths are retained when switching to remote. No local API key is needed.

For Remote API, enter the API **root**, including its version or gateway prefix (not the full transcription endpoint), a supported model name, response format, and language. The adapter appends `audio/transcriptions` to that root. Use HTTPS; unencrypted HTTP is allowed only on explicit loopback hosts for a local gateway. URLs containing user information, query parameters, or fragments are rejected. No endpoint/model is hardcoded.

Choose `verbose_json` to request segment timestamps, or `json` for text-oriented gateways/models. The adapter sends multipart fields `file` (16kHz mono PCM16 WAV), `model`, `response_format`, optional `language`, and `timestamp_granularities[]=segment` only for verbose JSON. Microphone/system tracks remain independent, with the existing 60-second chunks and common recording timeline. No naive concatenation is introduced.

**Summarization:** use a separate API root, chat model, and Windows Credential Manager key for OpenAI-compatible `chat/completions`. Saving configuration alone sends no meeting content. Enable and save the independent automatic-generation preference to authorize sharing future transcripts. See [AI_MEETING_INTELLIGENCE.md](AI_MEETING_INTELLIGENCE.md) for execution and compatibility limits.

## Credentials and outbound authorization

API keys are stored as app-namespaced generic credentials in the current Windows user's Credential Manager using `CredWriteW`, `CredReadW`, and `CredDeleteW`. The local-machine persistence setting applies to this user's credentials; it is not a plaintext file or a shared machine key. There is no insecure cross-platform fallback. SQLite stores only non-secret configuration and opaque credential references. No credential enumeration or unrelated credential access is performed.

The UI has a temporary uncontrolled password input, cleared when submitting or changing key action. Keys are not kept in React application state, returned in backend snapshots, displayed after saving, serialized in settings/job configurations, or logged. Backend key buffers are zeroized where controlled; HTTP libraries necessarily hold transient authorization-header copies. This feature does not protect against a compromised same-user process or memory inspection.

Keep, Replace/Add, and Delete actions apply when saving. A changed API root cannot silently reuse a saved key: explicitly re-enter its key for that destination or delete it. Local mode retires the transcription API key. A new key is written before the SQLite settings transaction commits; failed writes roll back settings and attempt to remove the new credential. Retired credential deletion is journaled with opaque IDs in migration 0006 and retried at worker startup or the next settings save if OS deletion fails. A crash between OS credential creation and SQLite commit can leave an unreferenced app credential; no audio/request can use it without a live settings reference. Credential Manager and SQLite do not support one distributed atomic transaction.

**Automatically transcribe saved recordings** is a persistent, opt-in preference in AI Processing. Enabling and saving it authorizes processing of future captures with the reviewed provider, including external audio uploads and bounded retries for remote mode. With it off, new recordings wait for a manual Transcribe action; remote confirmation appears only after clicking that action. Opening Meeting Details only reads status and never schedules work or opens a confirmation automatically. No automatic fallback exists. Older recordings, captures already in progress when enabling/changing providers, interrupted captures recovered without a successful save, and unstarted backlog remain manual. With automatic mode enabled, an explicit Transcribe click may process an older recording with the reviewed saved provider without another dialog; it never starts just from opening a screen.

HTTP requests use certificate validation, disabled redirects, disabled system proxies, a 15-second connection deadline, a 120-second total request deadline, and a bounded 4MiB response. Credentials are sent only to the configured destination during an approved recording operation or an explicitly triggered connection test. A provider redirect is reported as configuration failure; neither credentials nor audio is forwarded to its redirect destination. The configured provider may itself retain/process uploaded data under its own policy.

## Connection tests

Tests always use the **saved** settings and matching revision; save edits first. There are no background connection tests.

- Local: checks readable executable/model fingerprints. It does not run a model or prove speech quality.
- Remote transcription: the user must approve key transmission and a **one-second generated silence WAV upload**, which may incur API charges. The real multipart request/response shape is checked without using a meeting recording. This does not establish speech quality or availability of timestamps on actual speech.
- Summarization: explicitly authorize a synthetic chat-completion request using the saved provider, language and JSON mode. No meeting data is sent; API charges may apply. The response must pass structured evidence validation. Model enumeration is not required.

## Queue policy and result compatibility

Migration 0005 adds provider settings and a per-job upload revision; 0006 adds credential cleanup receipts. Existing meetings, recordings, local settings, jobs, checkpoints, and transcripts are preserved. Old local job configuration JSON remains readable, and local checkpoint fingerprints are unchanged.

Migration 0007 adds a provider-revision-bound automatic preference, a recording-row watermark, and a job readiness flag. New finalization inserts or reuses its unique job in the existing recording transaction; only a newly inserted job for a fresh capture after that watermark becomes ready automatically, with its provider snapshot and upload revision. Backfill and duplicate finalization never upgrade old jobs. Unready jobs persist as queued/manual and consume no attempts. The worker scans ready work independently of UI navigation. Started jobs freeze a non-secret provider configuration snapshot. Local jobs continue with that local snapshot. Remote jobs additionally require their approved revision to remain current before every chunk request and final completion. If settings are changed while a request is in flight, that already-authorized request may finish at the old destination, but later requests/result completion stop until explicit rebinding; the job never combines different provider configurations.

Normal Retry uses the frozen configuration/checkpoints. **Transcribe with latest settings** explicitly replaces a failed/manual job snapshot, clears its partial checkpoints, and (for remote) records upload approval, using the saved automatic authorization or a manual confirmation. It preserves original audio and lifetime attempt counters. Active/completed jobs cannot be rebound. Completed transcripts are not silently regenerated or overwritten when settings change. There are three automatic attempts and a five-attempt lifetime budget including manual retry, as in the existing queue.

401/403 are nonretryable authentication/access failures. 429, 408, network timeouts, and 5xx failures use bounded queue retries; numeric `Retry-After` seconds are respected up to fifteen minutes, otherwise the existing backoff applies. Unsupported requests, malformed formats, and invalid timestamp ranges fail visibly instead of automatically retrying a different protocol. Provider error bodies are never shown because they may echo secrets/content. A successful transcription response containing the exact saved key is also rejected before checkpoint or transcript persistence. Cancellation/full app exit stops further work; an already received remote upload cannot be recalled. Remote timeout/crash recovery can repeat a request before checkpointing, so provider-side exactly-once billing is not guaranteed.

Transcript/track timestamps are nullable. When supported, source-relative seconds are validated and mapped to the existing meeting timeline. When absent, text/source identity is persisted with **null times**, displayed as “No timestamps,” and accompanied by a warning. Timing is never guessed from word count or chunk duration. Without segment times, reliable cross-source ordering and echo reduction are unavailable. Timestamped exact-overlap deduplication retains both source references as before. Older numeric timestamp rows deserialize correctly.

## Compatibility and limits

Supported gateways use bearer authentication, the standard audio multipart protocol, and JSON containing a string `text` with optional `segments` entries (`text`, `start`, `end` seconds). Other encodings, streaming, URL-query credentials, custom auth headers, server redirects, alternate field names, and non-OpenAI routes require a future adapter. A gateway may support `json` but reject `verbose_json` or timestamp options; choose its documented format explicitly. Empty text/segments are accepted, while invalid shapes or timestamps are rejected. No assertion of universal gateway compatibility is made.

Sources: [OpenAI-compatible transcription contract](https://developers.openai.com/api/reference/resources/audio/subresources/transcriptions/methods/create), [Windows credential API](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew), and the existing [local runtime documentation](TRANSCRIPTION.md).

## Validation

Checks: TypeScript, frontend build, locked Rust check, formatting, Rust unit tests, the existing recording-flow tests, and the opt-in real local Whisper test described in TRANSCRIPTION.md.

Tests use dummy credentials and loopback mock servers only. They cover OS credential persistence/replacement/deletion, independent settings, endpoint-change restrictions, rollback/cleanup journaling, local checkpoint compatibility, queued revision-specific consent, changed settings, multipart formatting, source timestamps/null timestamps, invalid credentials, rate-limit classification, redirects, unsupported bodies, and model-list-only summarization tests. Existing recording/storage/recovery tests remain in the suite.

No real provider account, user API key, paid remote speech-to-text, or remote meeting content was used. Live endpoint/model compatibility, provider billing/retention, TLS/proxy deployment differences, key deletion failure across OS restarts, and long remote recordings require further user-authorized testing. Generation execution is tested with synthetic loopback providers; real-model quality and gateway compatibility remain user-authorized manual checks.

Verified results for this enhancement: 51 standard Rust tests passed (one opt-in real local test excluded from that default run), the real two-track local Whisper regression test passed separately, and all 7 recording-flow tests passed. TypeScript, production build, Rust compilation, formatting, and Git whitespace checks passed. Native settings UI and a live paid gateway were not manually tested in this enhancement. Tests created/deleted only app-namespaced dummy credentials and contacted only loopback mock servers.

## Automatic-transcription validation

Regression tests cover finalization-time authorization, duplicate saves, exclusion of older captures, ready-only worker claims, manual approval, repeated status reads, preference persistence, reviewed provider changes, unchanged revision preservation, disabling automatic mode, independent summary settings, and an actual backend worker completing a remote job against a loopback mock server without any Meeting Details calls. Existing retry/checkpoint/recovery tests remain active. Disabling automatic mode affects future recordings, not work already authorized or an upload in flight. A source captured before enabling the preference deliberately requires manual action even if saved afterward. Live Groq uploads and interactive Windows Stop & Save were not repeated for this enhancement; no user credentials or recordings were used in tests.

Automatic-transcription enhancement results: 56 standard Rust tests and 7 recording-flow tests passed, along with TypeScript, production build, Rust checks/formatting, and Git whitespace validation. The mock-server background-worker test verified that scheduling and completion do not depend on any UI/status read. Interactive Groq recording and repeated Meeting Details navigation still need human verification; no live API calls were made for this change.

### Successful saves with capture warnings

A nonfatal Windows audio discontinuity sets the historical `recovered` flag, but a successful Stop & Save still has `state=saved`. Automatic queue authorization uses the successful finalization state, not that warning flag. Interrupted captures finalized as failed remain manual. Audio warnings remain visible and source files are retained. The New Recording form reports the saved automatic preference before capture; no preference is enabled implicitly. Retry controls appear only for failed jobs, while retryable jobs wait for their scheduled backend retry.

### Automatic workflow fix validation (2026-10-12)

The finalization regression now reproduces a successful save with `recovered=true`, verifies automatic readiness and revision-bound upload authorization, and verifies that older/interrupted recordings remain manual. Duplicate finalization still creates one job. Standard tests cover settings/revision persistence, backend worker processing through loopback mock HTTP without details calls, retries, recovery, checkpoints, and atomic transcript persistence. The first concurrent full run timed out in the mock-worker integration test; its targeted rerun and the sequential full suite passed. A separate real local Whisper test using the public audio fixture passed, including restart checkpoint reuse.

An isolated native Windows profile with a saved local automatic preference was used for Start Recording → Stop & Save. The SQLite receipt showed finalized saved audio, `ready=1`, and a backend attempt without a manual Transcribe action or upload dialog. This quiet live microphone sample failed local output validation because Whisper returned timestamps outside the captured chunk; audio was retained and Retry was shown. Concurrent desktop input opened details/retried, so unattended list navigation and complete transcript/restart behavior were not verified end-to-end in that UI run. No external provider received audio, and the user's provider settings were not changed. The original user's persisted authorization and exact loaded binary revision could not be conclusively established from the running session.

Remaining manual verification, using a freshly restarted build:

1. Run `npm run desktop:dev` from this checkout after closing the previous app/dev instance. In Recording Preferences → AI Processing, review the selected provider, enable automatic transcription, and save. Confirm the **Saved preference** reports enabled. For a test without external uploads, use a working local Whisper configuration or an isolated loopback mock provider.
2. Start a new recording **after** saving the preference, capture clear known speech, then Stop & Save. Stay on the Note Taker list: verify return to the list and queued/transcribing/completed status without opening details or approving another upload.
3. Open the completed meeting's Transcript tab. Verify timestamp/source references, no Transcribe or Retry controls, and playable original audio.
4. Close and restart the app. Verify the transcript and enabled preference persist. Repeatedly reopen details/list; verify the job identity/attempt count does not change and no second job/upload occurs.
5. Repeat with another new recording. Disable automatic mode before another capture and verify the pre-recording notice plus explicit manual flow. Change provider settings and verify renewed preference review is required; older recordings remain manual.
6. Inject an unavailable provider/rate-limit failure with a mock service: verify scheduled retries need no action, Retry appears only after failure, and audio/checkpoints survive restart. Live Groq verification requires the user's explicit authorization and remains unperformed.

## Meeting intelligence implementation update

Summarization configuration now executes independently through the durable generation queue. Its automatic preference authorizes transcript text, not audio; the synthetic test now uses chat completions with the saved language/structured-output mode instead of model enumeration. See [AI_MEETING_INTELLIGENCE.md](AI_MEETING_INTELLIGENCE.md) for the current contracts, authorization scope, bounded retries, provider compatibility and testing limits.
