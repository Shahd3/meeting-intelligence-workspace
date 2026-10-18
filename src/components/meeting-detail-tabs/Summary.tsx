import type { Meeting } from "../../data/types";
import { IntelligenceContent } from "../MeetingIntelligence";

export function Summary({ meeting, onEvidence }: {
  meeting: Meeting;
  onEvidence: (id: string) => void;
}) {
  if (meeting.recorded) {
    return <IntelligenceContent meeting={meeting} tab="Summary" onEvidence={onEvidence} />;
  }
  return <>
    <p className="prose">{meeting.summary}</p>
    <div className="callout">
      <span className="eyebrow">THE TAKEAWAY</span>
      <p className="prose">{meeting.decision}</p>
    </div>
    {meeting.highlights.map((point, index) => <p className="prose" key={index}>{point}</p>)}
    <h3>In the room</h3>
    <div className="participants">
      {meeting.speakers.map(speaker => <span className="pill" key={speaker}>{speaker}</span>)}
    </div>
  </>;
}
