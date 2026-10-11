import { useEffect, useRef, useState } from "react";
import type { ChatMessage, Meeting, Workspace } from "../data/types";

const suggestions = [
  ["Decisions so far ↗", "What decisions have we made?"], ["Pending tasks ↗", "What tasks are pending?"],
  ["What changed? ↗", "What changed between meetings?"], ["Our timeline ↗", "What is our timeline?"],
];

export function ProjectAssistant({ workspace, meetings, messages, onAsk, onMeeting }: {
  workspace: Workspace; meetings: Meeting[]; messages: ChatMessage[]; onAsk: (question: string) => void; onMeeting: (id: number) => void;
}) {
  const [question, setQuestion] = useState("");
  const container = useRef<HTMLDivElement>(null);
  useEffect(() => { if (container.current) container.current.scrollTop = container.current.scrollHeight; }, [messages]);
  const ask = (value: string) => { if (value.trim()) { onAsk(value.trim()); setQuestion(""); } };
  return <section className="panel assistant"><div className="assistant-head"><div className="section flush"><h2>Connect the dots ✦</h2><span className={`tag ${workspace.color}`}>Workspace AI</span></div><small>Analyzing {workspace.name} · {meetings.length} meetings</small></div>
    <div className="messages" ref={container} role="log" aria-label="Workspace assistant conversation" aria-relevant="additions">
      <div className="bubble"><span className="eyebrow">GATHER / YOUR PROJECT COMPANION</span><p className="spaced">Let’s find the thread. I can help explore decisions, next steps, and what’s changed across <strong>{workspace.name}</strong>.</p></div>
      {messages.map(message => <div key={message.id} className={`bubble ${message.user ? "user" : ""}`}><span className="sr-only">{message.user ? "You: " : "Simulated assistant: "}</span>{message.text}
        {!!message.meetingIds.length && <div className="references"><small>MEETING REFERENCES</small>{message.meetingIds.map(id => {
          const meeting = meetings.find(item => item.id === id);
          return meeting && <button className="ref" key={id} onClick={() => onMeeting(id)}>{meeting.title} · {meeting.date} ↗</button>;
        })}</div>}
      </div>)}
    </div>
    <div className="suggestions">{suggestions.map(([label, value]) => <button key={value} onClick={() => ask(value)}>{label}</button>)}</div>
    <form className="chat" onSubmit={event => { event.preventDefault(); ask(question); }}><input placeholder="Ask about this workspace…" aria-label="Workspace question" value={question} onChange={event => setQuestion(event.target.value)} maxLength={1000} required /><button className="dark send" aria-label="Send question">↑</button></form><small className="assistant-disclaimer">Simulated assistant · Meeting references included</small>
  </section>;
}
