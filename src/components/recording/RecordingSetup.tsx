import { useProcessing } from "../../data/processing";
import type { AudioDevices } from "../../data/recording";

type Props = {
  devices: AudioDevices | null;
  busy: boolean;
  discovering: boolean;
  title: string;
  microphone: boolean;
  system: boolean;
  micId: string;
  outputId: string;
  setTitle: (value: string) => void;
  setMicrophone: (value: boolean) => void;
  setSystem: (value: boolean) => void;
  setMicId: (value: string) => void;
  setOutputId: (value: string) => void;
  load: () => void;
};

export function RecordingSetup({
  devices, busy, discovering, title, microphone, system, micId, outputId,
  setTitle, setMicrophone, setSystem, setMicId, setOutputId, load,
}: Props) {
  const processing = useProcessing();
  const ai = processing.view?.ai.transcription;
  return <fieldset className="audio-sources" disabled={busy || discovering}>
      <label className="field">Meeting title (optional)<input value={title} placeholder="Leave blank for an AI-generated title" onChange={event => setTitle(event.target.value)} maxLength={90} /></label>
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
  </fieldset>;

}
