import { useEffect, useRef, useState } from "react";
import { discoverDevices, type AudioDevices, type RecordingController } from "../data/recording";
import { sessionView, unavailableSources } from "../data/recordingFlow";
import { RecordingStatus } from "./recording/RecordingStatus";
import { RecordingSetup } from "./recording/RecordingSetup";
import { RecordingControls } from "./recording/RecordingControls";

export function RecordingPanel({ recorder, onRecovered, saving, saveError, onRetry }: { recorder: RecordingController; onRecovered: () => void; saving: boolean; saveError: string | null; onRetry: () => void }) {
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
  const selection = { title, microphone, system, microphone_device: micId || null, system_device: outputId || null };
  const { microphone: missingMic, system: missingOutput } = unavailableSources(selection, devices);
  return <div className={`record ${status.state !== "recording" ? "paused" : ""} ${!active ? "record-setup" : ""}`}>
    <RecordingStatus status={status} saving={saving} active={active} />
    {!active && !failed && <RecordingSetup
      devices={devices} busy={recorder.busy} discovering={discovering}
      title={title} microphone={microphone} system={system} micId={micId} outputId={outputId}
      setTitle={setTitle} setMicrophone={setMicrophone} setSystem={setSystem}
      setMicId={setMicId} setOutputId={setOutputId} load={() => void load()}
    />}
    {!active && !failed && devices && !microphone && !system && <p className="muted">Enable at least one audio source in Audio Settings.</p>}
    {discovering && <p role="status">Finding audio devices…</p>}
    {deviceError && !active && <p className="recording-error" role="alert">{deviceError}</p>}
    {!active && !failed && devices && (missingMic || missingOutput) && <p className="recording-error" role="alert">{missingMic ? "The selected microphone is unavailable. " : ""}{missingOutput ? "The selected system audio output is unavailable. " : ""}Choose an available device in Audio Settings.</p>}
    {(((attempted || active || failed || !recorder.ready) && recorder.error) || status.error || saveError) && <p className="recording-error" role="alert">{saveError || recorder.error || status.error}</p>}
    {status.warning && <p className="recording-warning" role="status">{status.warning}</p>}
    <RecordingControls
      state={status.state} active={active} saving={saving} busy={recorder.busy}
      startDisabled={!recorder.ready || recorder.busy || discovering || !!deviceError || !devices || (!microphone && !system) || missingMic || missingOutput}
      saveError={saveError} onRetry={onRetry}
      onStart={async () => { setAttempted(true); await recorder.start(selection); }}
      onControl={action => void recorder.control(action)}
      onRecover={async () => { if (await recorder.recover()) onRecovered(); }}
    />
    <p className="muted recording-disclaimer">{active ? "You can navigate or hide these controls while capture continues. Stop before closing the app." : "Audio saved locally · Up to 4 hours per recording · Transcription follows your saved preferences"}</p>
  </div>;
}
