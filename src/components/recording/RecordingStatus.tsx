import type { RecordingStatus as Status } from "../../data/recording";
import { formatTime } from "../../data/simulation";
import { RecordingWaveform } from "./RecordingWaveform";

export function RecordingStatus({ status, saving, active }: {
  status: Status;
  saving: boolean;
  active: boolean;
}) {
  const seconds = Math.floor(status.elapsed_ms / 1000);
  const label = saving || status.state === "stopping" || status.state === "saved"
    ? "Finalizing audio & saving…"
    : status.state === "idle" ? "Ready to record" : status.state;
  return <>
    <span className="tag pink"><i className="dot" /><span role="status">{label}</span></span>
    <div className="timer" aria-label={`${seconds} seconds captured`}>{formatTime(seconds)}</div>
    {active && <>
      <p className="muted">
        {status.state === "paused"
          ? "Paused time is excluded from your recording."
          : "Audio is being preserved on this device."}
      </p>
      <RecordingWaveform peak={status.peak} />
    </>}
  </>;
}
