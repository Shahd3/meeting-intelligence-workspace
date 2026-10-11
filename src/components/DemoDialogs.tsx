import { useEffect, useState } from "react";
import { formatTime } from "../data/simulation";
import type { Color, Workspace } from "../data/types";

export function CreateWorkspaceForm({ onSave, onClose, initial }: { onSave: (name: string, description: string, color: Color) => void; onClose: () => void; initial?: Workspace }) {
  const [name, setName] = useState(initial?.name || "");
  const [description, setDescription] = useState(initial?.description || "");
  const [color, setColor] = useState<Color>(initial?.color || "pink");
  return <><p className="muted">Give your project a home. The context will follow.</p><form onSubmit={event => { event.preventDefault(); if (name.trim()) onSave(name.trim(), description.trim(), color); }}>
    <label className="field">Workspace name<input placeholder="e.g. Website redesign" value={name} onChange={event => setName(event.target.value)} required maxLength={50} /></label>
    <label className="field">A little about this project<textarea rows={3} placeholder="What are you working toward?" value={description} onChange={event => setDescription(event.target.value)} maxLength={180} /></label>
    <label className="field">Color identity<select value={color} onChange={event => setColor(event.target.value as Color)}><option value="pink">Soft pink</option><option value="sage">Sage green</option><option value="lav">Lavender</option><option value="yellow">Butter yellow</option><option value="peach">Peach</option></select></label>
    <div className="actions"><button type="button" className="pill" onClick={onClose}>Cancel</button><button className="dark" disabled={!name.trim()}>{initial ? "Save workspace ↗" : "Create workspace ↗"}</button></div>
  </form></>;
}

export function RecordingDemo({ onStop }: { onStop: (seconds: number) => void }) {
  const [seconds, setSeconds] = useState(0);
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    if (paused) return;
    const timer = window.setInterval(() => setSeconds(value => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [paused]);
  return <div className={`record ${paused ? "paused" : ""}`}><span className="tag pink"><i className="dot" /><span role="status">{paused ? "Paused" : "Recording simulation"}</span></span>
    <div className="timer" aria-label={`${seconds} seconds elapsed`}>{formatTime(seconds)}</div><p className="muted">Your ideas are finding their rhythm.</p><div className="wave" aria-hidden="true">{Array.from({ length: 40 }, (_, index) => <i key={index} />)}</div>
    <button className="pill" onClick={() => setPaused(value => !value)}>{paused ? "▶ Resume" : "Ⅱ Pause"}</button>{" "}<button className="dark" onClick={() => onStop(seconds)}>■ Stop & review</button><p className="muted recording-disclaimer">Demo only · No microphone access or audio capture</p>
  </div>;
}

export function SaveMeetingForm({ seconds, workspaces, onSave, onClose }: {
  seconds: number; workspaces: Workspace[]; onSave: (title: string, project: number) => void; onClose: () => void;
}) {
  const [title, setTitle] = useState("Weekly project check-in");
  const [project, setProject] = useState(0);
  return <><div className="callout recording-result"><span className="tip-symbol">✓</span><h2>Your conversation is ready.</h2><p className="muted">{formatTime(seconds)} captured · Sample summary generated</p></div>
    <form onSubmit={event => { event.preventDefault(); if (title.trim()) onSave(title.trim(), project); }}>
      <label className="field">Meeting title<input value={title} onChange={event => setTitle(event.target.value)} required maxLength={90} /></label>
      <label className="field">Workspace (optional)<select value={project} onChange={event => setProject(Number(event.target.value))}><option value="0">Unassigned</option>{workspaces.map(workspace => <option key={workspace.id} value={workspace.id}>{workspace.name}</option>)}</select></label>
      <div className="actions"><button type="button" className="pill" onClick={onClose}>Discard demo</button><button className="dark" disabled={!title.trim()}>Save & review ↗</button></div>
    </form>
  </>;
}

export function AboutDemo({ onClose }: { onClose: () => void }) {
  return <><p className="prose">A thoughtful home for Meeting Intelligence Workspace. Explore Home, capture a simulated meeting in Note Taker, and connect conversations in Workspaces.</p><div className="callout prose">Recording, generated summaries, and assistant responses are simulated. Meetings, workspaces, and task completion are saved locally. Assistant conversations last for this session. No meeting data leaves this app.</div><button className="dark" onClick={onClose}>Let’s explore ↗</button></>;
}
