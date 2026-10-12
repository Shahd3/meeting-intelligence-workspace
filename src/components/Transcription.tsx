import { UploadApproval } from "./AIProcessingPreferences";
import { processingLabel, useProcessing, type ProcessingJob } from "../data/processing";
import { formatTime } from "../data/simulation";
export { AIProcessingPreferences as TranscriptionPreferences } from "./AIProcessingPreferences";
export function TranscriptionStatus({ job }: { job: ProcessingJob }) {
  const data = useProcessing();
  return <div className="transcription-status" role="status">
    <span className="tag sage">{processingLabel(job)}</span>
    {job.state === "processing" && <small> {job.stage.replaceAll("_", " ")} · {job.checkpoints} audio chunks checkpointed</small>}
    {job.state === "queued" && job.ready && <small> Waiting for the local backend worker.</small>}
    {job.error && <p>{job.error}</p>}
    {job.state === "retryable" && job.next_attempt_at && <small>Next attempt: {new Date(job.next_attempt_at * 1000).toLocaleTimeString()}</small>}
    {job.warning && <p>{job.warning}</p>}
    {(job.ready && job.state === "failed") && <button className="pill" disabled={data.busy || job.attempts >= 5} onClick={() => void data.command("processing_retry", { id: job.id })}>Retry transcription</button>}
    {job.attempts >= 5 && job.state === "failed" && <p>The five-attempt limit was reached. Audio is retained.</p>}
    <UploadApproval job={job} />
    {data.error && <p role="alert">{data.error}</p>}
  </div>;
}
export function MeetingTranscript({ meetingId }: { meetingId: number }) {
  const { view, error } = useProcessing();
  const jobs = view?.jobs.filter(job => job.meeting_id === meetingId) || [];
  if (error) return <p role="alert">{error}</p>;
  if (!view || view.transcript_meeting_id !== meetingId) return <p role="status">Loading transcription status…</p>;
  if (!jobs.length) return <p className="muted">No finalized audio is available for transcription.</p>;
  return <div className="transcript persisted-transcript">{jobs.map(job => <section key={job.id}>
    {job.state === "completed" && !job.segments?.length && <p>No speech was recognized. Silent audio produces an empty transcript.</p>}
    {job.segments?.map((segment, index) => <div className="speech" key={`${job.id}-${index}`}><small>{segment.start_ms === null || segment.end_ms === null ? "No timestamps" : `${formatTime(Math.floor(segment.start_ms / 1000))}–${formatTime(Math.floor(segment.end_ms / 1000))}`}</small><div><h3>{segment.sources.map(source => source.role === "system" ? "System audio" : "Microphone").join(" + ")}</h3><p>{segment.text}</p></div></div>)}
    {job.state !== "completed" && <p className="muted">The full transcript appears after all source tracks finish. Saved audio remains available above.</p>}
  </section>)}</div>;
}
