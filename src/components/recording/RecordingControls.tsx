import type { RecordingState } from "../../data/recording";

type Props = {
  state: RecordingState;
  active: boolean;
  saving: boolean;
  busy: boolean;
  startDisabled: boolean;
  saveError: string | null;
  onStart: () => void;
  onControl: (action: "pause" | "resume" | "stop") => void;
  onRetry: () => void;
  onRecover: () => void;
};

export function RecordingControls({
  state, active, saving, busy, startDisabled, saveError, onStart, onControl, onRetry, onRecover,
}: Props) {
  return <div className="actions recording-controls">
    {!active && state !== "failed" && (
      <button className="dark" disabled={startDisabled} onClick={onStart}>Start Recording</button>
    )}
    {!saving && (state === "recording" || state === "paused") && <>
      <button className="pill" disabled={busy} onClick={() => onControl(state === "paused" ? "resume" : "pause")}>
        {state === "paused" ? "▶ Resume" : "Ⅱ Pause"}
      </button>
      <button className="dark" disabled={busy} onClick={() => onControl("stop")}>■ Stop & Save</button>
    </>}
    {saveError && <button className="pill" onClick={onRetry}>Retry loading saved recording</button>}
    {state === "failed" && <div className="recovery-notice">
      <p>Captured audio is retained. Check recovery before starting a new session.</p>
      <button className="pill" disabled={busy} onClick={onRecover}>Recover retained audio</button>
    </div>}
  </div>;
}
