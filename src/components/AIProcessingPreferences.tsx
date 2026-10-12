import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { useProcessing, type AISettings, type ProcessingJob } from "../data/processing";
type Kind = "transcription" | "summarization";
export function AIProcessingPreferences() {
  const data = useProcessing();
  const [kind, setKind] = useState<Kind>("transcription");
  return <section className="transcription-preferences">
    <h3>AI Processing</h3>
    <div className="ai-settings-tabs" aria-label="AI settings section">
      {(["transcription", "summarization"] as const).map(item => <button type="button" className={kind === item ? "dark" : "pill"} aria-pressed={kind === item} key={item} onClick={() => setKind(item)}>{item === "transcription" ? "Transcription" : "Summarization"}</button>)}
    </div>
    {kind === "transcription" && data.view && <p role="status" className="muted">Saved preference: {data.view.ai.transcription?.automatic_transcription ? "automatic transcription enabled" : "automatic transcription disabled — enable and save it below before recording"}.</p>}
    {kind === "summarization" && data.view && <p role="status" className="muted">Saved preference: automatic meeting intelligence {data.view.ai.summarization?.automatic_generation ? "enabled" : "disabled"}.</p>}
    {!data.view ? <p role="status">Loading AI settings…</p> : <AIForm key={`${kind}-${data.view.ai[kind]?.revision || "new"}`} kind={kind} saved={data.view.ai[kind]} />}
    {!!data.view?.ai.credential_cleanup_pending && <p role="alert">Retired credential cleanup is pending. Save settings again to retry; retired credentials cannot authorize new requests.</p>}
    {(data.error || data.view?.worker_error || data.view?.intelligence.worker_error) && <p className="storage-error" role="alert">{data.error || data.view?.worker_error || data.view?.intelligence.worker_error}</p>}
  </section>;
}
function AIForm({ kind, saved }: { kind: Kind; saved: AISettings | null }) {
  const data = useProcessing();
  const local = saved?.local || data.view?.config;
  const [mode, setMode] = useState(saved?.mode || (kind === "transcription" ? "local" : "remote"));
  const [executable, setExecutable] = useState(local?.executable || "");
  const [modelPath, setModelPath] = useState(local?.model || "");
  const [language, setLanguage] = useState(saved?.remote?.language || local?.language || "auto");
  const [baseUrl, setBaseUrl] = useState(saved?.remote?.base_url || "");
  const [model, setModel] = useState(saved?.remote?.model || "");
  const [format, setFormat] = useState(saved?.remote?.response_format || "verbose_json");
  const [outputLanguage, setOutputLanguage] = useState(saved?.output_language || "en");
  const [generationFormat, setGenerationFormat] = useState(saved?.generation_format || "json_object");
  const [automatic, setAutomatic] = useState((kind === "transcription" ? saved?.automatic_transcription : saved?.automatic_generation) || false);
  const [automaticReviewed, setAutomaticReviewed] = useState(false);
  const settingsChanged = () => { setAutomatic(false); setAutomaticReviewed(false); };
  const [keyAction, setKeyAction] = useState("keep");
  const [testApproved, setTestApproved] = useState(false);
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  // The key is an uncontrolled, temporary password input. It is never stored in
  // React state/context, loaded from the backend, or echoed after saving.
  const keyInput = useRef<HTMLInputElement>(null);
  const save = async () => {
    setResult(null);
    const apiKey = keyInput.current?.value || null;
    if (keyInput.current) keyInput.current.value = "";
    await data.command("ai_settings_save", { request: {
      kind, mode, local: mode === "local" ? { executable, model: modelPath, language } : null,
      remote: mode === "remote" ? { base_url: baseUrl, model, language: kind === "transcription" ? language : "auto", response_format: kind === "transcription" ? format : "json" } : null,
      automatic_transcription: kind === "transcription" && automatic, automatic_generation: kind === "summarization" && automatic, automatic_reviewed: automaticReviewed, output_language: outputLanguage, generation_format: generationFormat,
      key_action: mode === "local" ? "delete" : keyAction, api_key: keyAction === "replace" ? apiKey : null,
    } });
  };
  const test = async () => {
    if (!saved || (saved.mode === "remote" && !testApproved)) return;
    setTesting(true); setResult(null);
    try { setResult(await invoke<string>("ai_connection_test", { kind, revision: saved.revision, authorized: true })); }
    catch (cause) { setResult(String(cause)); }
    finally { setTesting(false); }
  };
  return <>
    {kind === "summarization" && <p className="muted">OpenAI-compatible chat completions generate evidence-backed meeting notes. This provider is independent of transcription. Saving configuration alone sends no meeting content.</p>}
    <form onSubmit={event => { event.preventDefault(); void save(); }}>
      {kind === "transcription" && <label>Transcription provider<select className="select" value={mode} onChange={event => { settingsChanged(); setMode(event.target.value as "local" | "remote"); if (keyInput.current) keyInput.current.value = ""; }}><option value="local">Local Whisper</option><option value="remote">Remote API</option></select></label>}
      {mode === "local" ? <>
        <p className="muted">Audio stays on this device. Use a trusted whisper.cpp executable and compatible GGML model; no API key or Python runtime is required.</p>
        <label>whisper-cli.exe path<input required value={executable} onChange={event => { settingsChanged(); setExecutable(event.target.value); }} placeholder="Absolute local executable path" /></label>
        <label>GGML model path<input required value={modelPath} onChange={event => { settingsChanged(); setModelPath(event.target.value); }} placeholder="Absolute local model path" /></label>
      </> : <>
        <label>API base URL<input required type="url" value={baseUrl} onChange={event => { settingsChanged(); setBaseUrl(event.target.value); }} placeholder="HTTPS API root, including its version path" /></label>
        <label>Model name<input required value={model} onChange={event => { settingsChanged(); setModel(event.target.value); }} placeholder="Provider-supported model name" /></label>
        <label>API key action<select className="select" value={keyAction} onChange={event => { settingsChanged(); setKeyAction(event.target.value); if (keyInput.current) keyInput.current.value = ""; }}><option value="keep">Keep saved key</option><option value="replace">Replace / add key</option><option value="delete">Delete saved key</option></select></label>
        {keyAction === "replace" && <label>New API key<input ref={keyInput} required type="password" autoComplete="off" spellCheck={false} placeholder="Stored securely in Windows Credential Manager" /></label>}
        <p className="muted">{saved?.key_saved ? "A key is saved; its value is never displayed." : "No saved key."} Transcription and summarization keys are separate. Changing the endpoint requires explicitly entering its key again or deleting the old one.</p>
        {kind === "transcription" && <label>Response format<select className="select" value={format} onChange={event => { settingsChanged(); setFormat(event.target.value); }}><option value="verbose_json">Verbose JSON — request segment timestamps</option><option value="json">JSON — text; timestamps may be unavailable</option></select></label>}
        {kind === "transcription" && <p className="ai-upload-notice">Remote processing uploads audio to your configured service and may incur charges. Enable automatic transcription below to authorize new recordings, or start a manual transcription with confirmation.</p>}
      </>}
      {kind === "transcription" && <label>Language<input required value={language} onChange={event => { settingsChanged(); setLanguage(event.target.value); }} placeholder="auto, en, ar…" /></label>}
      {kind === "transcription" && <>
        <label className="preference-toggle"><span><strong>Automatically transcribe saved recordings</strong><small>Process new recordings through the background queue</small></span><input type="checkbox" role="switch" checked={automatic} onChange={event => { setAutomatic(event.target.checked); setAutomaticReviewed(event.target.checked); }} /></label>
        <p className={mode === "remote" ? "ai-upload-notice" : "muted"}>{mode === "remote" ? `Enabling and saving authorizes automatic audio uploads and bounded retries to ${baseUrl || "the configured external transcription service"} using ${model || "the selected model"}. Charges may apply.` : "Enabling and saving starts local transcription automatically after recording finalization."} Older recordings and captures already in progress remain manual. Provider changes turn this preference off until you review and save it again.</p>
      </>}
      {kind === "summarization" && <>
        <label>Output language<select className="select" value={outputLanguage} onChange={event => { settingsChanged(); setOutputLanguage(event.target.value as "en" | "ar" | "auto"); }}><option value="en">English</option><option value="ar">Arabic</option><option value="auto">Match transcript</option></select></label>
        <label>Structured output<select className="select" value={generationFormat} onChange={event => { settingsChanged(); setGenerationFormat(event.target.value as "json_object" | "json_schema"); }}><option value="json_object">JSON object (gateway compatibility)</option><option value="json_schema">Strict JSON schema (provider support required)</option></select></label>
        <label className="preference-toggle"><span><strong>Automatically generate meeting intelligence</strong><small>Analyze new completed transcripts in the background</small></span><input type="checkbox" role="switch" checked={automatic} onChange={event => { setAutomatic(event.target.checked); setAutomaticReviewed(event.target.checked); }} /></label>
        <p className="ai-upload-notice">Enabling and saving authorizes sharing transcript text and evidence references with {baseUrl || "the configured summarization service"} using {model || "the selected model"}, including bounded retries. Charges may apply. Existing recordings remain manual. Provider or generation-option changes require renewed review.</p>
      </>}
      <p className="muted">Started jobs retain their settings. Use latest settings in a failed job to restart with a new provider; partial checkpoints are cleared, and original audio is retained.</p>
      <button className="pill" disabled={data.busy || testing}>Save settings</button>
    </form>
    {saved && <div className="ai-connection-test">
      <p className="muted">Tests use saved settings{saved.remote ? `: ${saved.remote.base_url} · ${saved.remote.model}` : ""}. Save edits first.</p>
      {saved.mode === "remote" && <label className="ai-consent"><input type="checkbox" checked={testApproved} onChange={event => setTestApproved(event.target.checked)} />{kind === "transcription" ? "I authorize a one-second synthetic silence upload and API key transmission to this saved provider. The test may incur API charges." : "I authorize a synthetic chat-completion request and sending this saved API key to the selected provider. No meeting data is used. API charges may apply."}</label>}
      <button type="button" className="pill" disabled={testing || data.busy || (saved.mode === "remote" && !testApproved)} onClick={() => void test()}>{testing ? "Testing…" : "Test saved connection"}</button>
      {result && <p role="status">{result}</p>}
    </div>}
  </>;
}
export function UploadApproval({ job }: { job: ProcessingJob }) {
  const data = useProcessing();
  const saved = data.view?.ai.transcription;
  const [confirm, setConfirm] = useState(false);
  const [approved, setApproved] = useState(false);
  useEffect(() => { setConfirm(false); setApproved(false); }, [saved?.revision, job.id]);
  if (!saved || !["failed", "queued"].includes(job.state) || job.attempts >= 5) return null;
  const changed = job.provider_revision !== saved.revision;
  if (job.ready && job.state === "queued") return null;
  if (job.ready && !changed) return null;
  const remote = saved.mode === "remote";
  const start = () => void data.command("processing_rebind", { id: job.id, revision: saved.revision, authorize: remote }).then(ok => { if (ok) setConfirm(false); });
  const begin = () => { if (remote && !saved.automatic_transcription) setConfirm(true); else start(); };
  return <div className="manual-transcription">
    {!confirm ? <>
      <button className="pill" disabled={data.busy || (remote && !saved.key_saved)} onClick={begin}>{job.state === "failed" ? "Retry with latest settings" : "Transcribe"}</button>
      {remote && <small> Uses {saved.remote?.base_url} · {saved.remote?.model}. External upload and charges may apply.</small>}
      {remote && !saved.key_saved && <p>Save an API key in AI Processing settings before transcribing.</p>}
    </> : <div className="ai-upload-notice">
      <p>Upload this recording’s microphone/system audio to {saved.remote?.base_url} using {saved.remote?.model}? Charges may apply. Any incompatible partial checkpoints will be cleared; original audio stays saved.</p>
      <label className="ai-consent"><input type="checkbox" checked={approved} onChange={event => setApproved(event.target.checked)} />I approve this recording’s upload and bounded retries.</label>
      <button className="pill" disabled={data.busy || !approved} onClick={start}>Confirm & Transcribe</button>
      <button className="text" disabled={data.busy} onClick={() => { setConfirm(false); setApproved(false); }}>Cancel</button>
    </div>}
  </div>;
}
