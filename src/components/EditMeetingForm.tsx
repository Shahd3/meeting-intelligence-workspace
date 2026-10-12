import { useState } from "react";
import type { Meeting } from "../data/types";

export function EditMeetingForm({ meeting, onSave, onClose }: { meeting: Meeting; onSave: (meeting: Meeting, titleEdited: boolean) => void; onClose: () => void }) {
  const [titleEdited, setTitleEdited] = useState(false);
  const [title, setTitle] = useState(meeting.title);
  const [summary, setSummary] = useState(meeting.summary);
  return <form onSubmit={event => { event.preventDefault(); if (title.trim()) onSave({ ...meeting, title: title.trim(), summary }, titleEdited); }}>
    <label className="field">Meeting title<input required maxLength={90} value={title} onChange={event => { setTitleEdited(true); setTitle(event.target.value); }} /></label>
    <label className="field">Summary<textarea rows={6} maxLength={100000} value={summary} onChange={event => setSummary(event.target.value)} /></label>
    <div className="actions"><button className="pill" type="button" onClick={onClose}>Cancel</button><button className="dark" disabled={!title.trim()}>Save meeting ↗</button></div>
  </form>;
}
