import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function MeetingDetailHeader({ meeting, notesReady, onBack }: {
  meeting: { id: number; title: string };
  notesReady: boolean;
  onBack: () => void;
}) {
  const [exporting, setExporting] = useState(false);
  const [exportMessage, setExportMessage] = useState<string | null>(null);
  const exportMinutes = async () => {
    setExporting(true);
    setExportMessage(null);
    try {
      const name = await invoke<string>("intelligence_download", { meetingId: meeting.id });
      setExportMessage(`Minutes saved to Downloads: ${name}`);
    } catch (cause) {
      setExportMessage(String(cause));
    } finally {
      setExporting(false);
    }
  };
  return <>
    <div className="detail-heading">
      <button className="circle" onClick={onBack} aria-label="Back">←</button>
      <div>
        <span className="eyebrow">MEETING NOTEBOOK / CONVERSATION</span>
        <h1>{meeting.title}</h1>
      </div>
      {notesReady && <span className="tag notes-ready">Notes ready</span>}
      <button className="pill mom-button" disabled={!notesReady || exporting}
        title={notesReady ? "Save Minutes of Meeting to Downloads" : "Meeting notes must be ready before generating MoM"}
        onClick={() => void exportMinutes()}>
        {exporting ? "Saving MoM…" : "Generate MoM"}
      </button>
    </div>
    {exportMessage && <p role="status">{exportMessage}</p>}
  </>;
}
