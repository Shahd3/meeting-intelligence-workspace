import { useState } from "react";
import { playbackUrl, type Recording } from "../data/recording";

export function RecordingPlayback({ recording }: { recording: Recording }) {
  const [failed, setFailed] = useState<Record<string, boolean>>({});
  const tracks = recording.tracks.filter(track => track.frames > 0 && !track.filename.endsWith(".part"));
  return <section className="recording-playback panel" aria-label={`Recording: ${recording.title}`}>
    <div className="section"><h2>Conversation audio</h2><span className={`tag ${recording.complete ? "sage" : "yellow"}`}>{recording.complete ? "Saved locally" : "Incomplete · review audio"}</span></div>
    {recording.warning && <p className="recording-warning">{recording.warning}</p>}
    {tracks.length ? tracks.map(track => <div className="audio-track" key={track.role}><label htmlFor={`audio-${recording.id}-${track.role}`}>{track.role === "microphone" ? "Microphone" : "System audio"} · {track.device_name}</label><audio id={`audio-${recording.id}-${track.role}`} aria-label={`${track.role} recording`} controls preload="metadata" src={playbackUrl(recording, track)} onError={() => setFailed(current => ({ ...current, [track.role]: true }))} />{failed[track.role] && <p role="alert">Audio could not be opened. Its file may be unavailable; retry recovery from recording controls.</p>}</div>) : <p className="muted">No finalized audio is available yet. Any partial files are retained; try recovery from recording controls.</p>}
    <p className="muted">Separate tracks share the same active recording timeline. Playback does not combine them.</p>
  </section>;
}
