import { MeetingTranscript } from "../Transcription";

export function Transcript({ meetingId, evidenceId }: {
  meetingId: number;
  evidenceId: string | null;
}) {
  return <MeetingTranscript meetingId={meetingId} evidenceId={evidenceId} />;
}
