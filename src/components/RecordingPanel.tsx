import { useEffect, useRef, useState } from "react";
import { discoverDevices, type AudioDevices, type RecordingController } from "../data/recording";
import { sessionView, unavailableSources } from "../data/recordingFlow";
import { useProcessing } from "../data/processing";
import { formatTime } from "../data/simulation";

export function RecordingPanel({ recorder, onRecovered, saving, saveError, onRetry }: { recorder: RecordingController; onRecovered: () => void; saving: boolean; saveError: string | null; onRetry: () => void }) {
  const processing = useProcessing();
  const ai = processing.view?.ai.transcription;
  const [devices, setDevices] = useState<AudioDevices | null>(null);
  const [deviceError, setDeviceError] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [microphone, setMicrophone] = useState(true);
  const [system, setSystem] = useState(false);
  const [micId, setMicId] = useState("");
  const [outputId, setOutputId] = useState("");
  const [discovering, setDiscovering] = useState(false);
  const [attempted, setAttempted] = useState(false);
  const initialId = useRef(recorder.status.id);
  const [sessionId, setSessionId] = useState<string | null>(recorder.active ? recorder.status.id : null);
  const load = async (initial = false) => {
    setDiscovering(true); setDeviceError(null);
    try {
      const found = await discoverDevices(); setDevices(found);
      if (initial) { setMicrophone(found.microphones.length > 0); setSystem(found.microphones.length === 0 && found.outputs.length > 0); }
    } catch (cause) { setDeviceError(String(cause)); }
    finally { setDiscovering(false); }
  };
  useEffect(() => { void load(true); }, []);
  useEffect(() => { if (recorder.status.id && (recorder.active || (attempted && recorder.status.id !== initialId.current))) setSessionId(recorder.status.id); }, [recorder.active, recorder.status.id, attempted]);
  const status = sessionView(recorder.status, sessionId);
  const active = recorder.active || saving || status.state === "saved";
  const failed = status.state === "failed";
  const seconds = Math.floor(status.elapsed_ms / 1000);
  const selection = { title, microphone, system, microphone_device: micId || null, system_device: outputId || null };
  const { microphone: missingMic, system: missingOutput } = unavailableSources(selection, devices);
  return <div className={`record ${status.state !== "recording" ? "paused" : ""} ${!active ? "record-setup" : ""}`}>
    <span className="tag pink"><i className="dot" /><span role="status">{saving || status.state === "stopping" || status.state === "saved" ? "Finalizing audio & saving…" : status.state === "idle" ? "Ready to record" : status.state}</span></span>
    <div className="timer" aria-label={`${seconds} seconds captured`}>{formatTime(seconds)}</div>
    {active && <><p className="muted">{status.state === "paused" ? "Paused time is excluded from your recording." : "Audio is being preserved on this device."}</p><div className="wave" aria-hidden="true" style={{ opacity: status.peak > 0.005 ? 1 : 0.35 }}>{Array.from({ length: 40 }, (_, index) => <i key={index} />)}</div></>}
    {!active && !failed && <fieldset className="audio-sources" disabled={recorder.busy || discovering}>
      <label className="field">Meeting title<input value={title} placeholder="Give this conversation a name" onChange={event => setTitle(event.target.value)} required maxLength={90} /></label>
      <p className="muted" role="status">{!processing.view ? "Checking saved transcription preferences…" : ai?.automatic_transcription ? ai.mode === "remote" ? "Automatic transcription is enabled. After saving, audio will be uploaded to your authorized transcription provider in the background." : "Automatic local transcription is enabled. Processing starts after saving without uploading audio." : "Automatic transcription is not authorized. Enable and save it in Recording Preferences → AI Processing before starting; this recording will otherwise require manual transcription."}</p>
      {processing.error && <p role="alert">Unable to verify transcription preferences: {processing.error}</p>}
      <div className="audio-summary"><span>Microphone <strong>{microphone ? "Enabled" : "Off"}</strong></span><span>System audio <strong>{system ? "Enabled" : "Off"}</strong></span></div>
      <details className="audio-settings"><summary>Audio Settings</summary>
        <label className="source-toggle"><input type="checkbox" checked={microphone} onChange={event => setMicrophone(event.target.checked)} /> Record my microphone</label>
        {microphone && <label className="field">Microphone<select value={micId} onChange={event => setMicId(event.target.value)}><option value="">Default microphone (recommended)</option>{micId && !devices?.microphones.some(device => device.id === micId) && <option value={micId}>Selected microphone unavailable</option>}{devices?.microphones.map(device => <option key={device.id} value={device.id}>{device.name}</option>)}</select></label>}
        <label className="source-toggle"><input type="checkbox" checked={system} onChange={event => setSystem(event.target.checked)} /> Record sound playing on my computer</label>
        {system && <label className="field">Speakers or headphones to capture<select value={outputId} onChange={event => setOutputId(event.target.value)}><option value="">Default speakers or headphones (recommended)</option>{outputId && !devices?.outputs.some(device => device.id === outputId) && <option value={outputId}>Selected output unavailable</option>}{devices?.outputs.map(device => <option key={device.id} value={device.id}>{device.name}</option>)}</select></label>}
        <p className="muted">System audio captures sound from the selected speakers or headphones. Each enabled source is saved separately. Unavailable sources will prevent recording from starting.</p>
        <button className="text" disabled={discovering} onClick={() => void load()}>Refresh devices</button>
      </details>
    </fieldset>}
    {!active && !failed && devices && !microphone && !system && <p className="muted">Enable at least one audio source in Audio Settings.</p>}
    {discovering && <p role="status">Finding audio devices…</p>}
    {deviceError && !active && <p className="recording-error" role="alert">{deviceError}</p>}
    {!active && !failed && devices && (missingMic || missingOutput) && <p className="recording-error" role="alert">{missingMic ? "The selected microphone is unavailable. " : ""}{missingOutput ? "The selected system audio output is unavailable. " : ""}Choose an available device in Audio Settings.</p>}
    {(((attempted || active || failed || !recorder.ready) && recorder.error) || status.error || saveError) && <p className="recording-error" role="alert">{saveError || recorder.error || status.error}</p>}
    {status.warning && <p className="recording-warning" role="status">{status.warning}</p>}
    <div className="actions recording-controls">
      {!active && !failed && <button className="dark" disabled={!recorder.ready || recorder.busy || discovering || !!deviceError || !devices || !title.trim() || (!microphone && !system) || missingMic || missingOutput} onClick={async () => { setAttempted(true); await recorder.start({ title, microphone, system, microphone_device: micId || null, system_device: outputId || null }); }}>Start Recording</button>}
      {!saving && (status.state === "recording" || status.state === "paused") && <><button className="pill" disabled={recorder.busy} onClick={() => void recorder.control(status.state === "paused" ? "resume" : "pause")}>{status.state === "paused" ? "▶ Resume" : "Ⅱ Pause"}</button><button className="dark" disabled={recorder.busy} onClick={() => void recorder.control("stop")}>■ Stop & Save</button></>}
      {saveError && <button className="pill" onClick={onRetry}>Retry loading saved recording</button>}
      {failed && <div className="recovery-notice"><p>Captured audio is retained. Check recovery before starting a new session.</p><button className="pill" disabled={recorder.busy} onClick={async () => { if (await recorder.recover()) onRecovered(); }}>Recover retained audio</button></div>}
    </div>
    <p className="muted recording-disclaimer">{active ? "You can navigate or hide these controls while capture continues. Stop before closing the app." : "Audio saved locally · Up to 4 hours per recording · Transcription follows your saved preferences"}</p>
  </div>;
}
