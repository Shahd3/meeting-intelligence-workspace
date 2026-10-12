import type { IntelligenceView } from "./intelligence";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
export type TranscriptSegment = { text: string; start_ms: number | null; end_ms: number | null; sources: { role: string; start_ms: number | null; end_ms: number | null }[] };
export type ProcessingJob = {
  id: string; meeting_id: number; recording_id: string; state: "queued" | "processing" | "completed" | "failed" | "retryable";
  ready: boolean; provider_revision: string | null; stage: string; attempts: number; error: string | null; error_category: string | null; next_attempt_at: number | null;
  checkpoints: number; segments: TranscriptSegment[] | null; warning: string | null;
};
export type TranscriptionConfig = { executable: string; model: string; language: string };
export type RemoteSettings = { base_url: string; model: string; response_format: string; language: string };
export type AISettings = { mode: "local" | "remote"; revision: string; local: TranscriptionConfig | null; remote: RemoteSettings | null; key_saved: boolean; automatic_transcription: boolean; automatic_generation: boolean; output_language: "en" | "ar" | "auto"; generation_format: "json_object" | "json_schema" };
type View = { intelligence: IntelligenceView; ai: { transcription: AISettings | null; summarization: AISettings | null; credential_cleanup_pending: number }; jobs: ProcessingJob[]; config: TranscriptionConfig | null; worker_error: string | null; transcript_meeting_id: number | null };
type Data = { view: View | null; error: string | null; busy: boolean; selectMeeting: (id: number | null) => void; command: (name: string, args: Record<string, unknown>) => Promise<boolean> };
const Context = createContext<Data>({ view: null, error: null, busy: false, selectMeeting: () => {}, command: async () => false });
export const useProcessing = () => useContext(Context);
export function processingLabel(job?: ProcessingJob) {
  if (!job) return "Audio saved";
  if (job.state === "queued" && !job.ready) return "Ready to transcribe";
  if (job.error_category === "consent") return "Upload approval required";
  return ({ queued: "Transcription queued", processing: "Transcribing audio", completed: "", failed: "Transcription needs attention", retryable: "Transcription retry scheduled" })[job.state];
}
export function ProcessingProvider({ children }: { children: ReactNode }) {
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pollError, setPollError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const [meetingId, setMeetingId] = useState<number | null>(null);
  const selectMeeting = useCallback((id: number | null) => setMeetingId(id), []);
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false; let timer: number;
    const poll = async () => {
      try { const next = await invoke<View>("processing_status", { meetingId }); if (!disposed) { setView(next); setPollError(null); } }
      catch (cause) { if (!disposed) setPollError(String(cause)); }
      if (!disposed) timer = window.setTimeout(() => void poll(), 2000);
    };
    void poll(); return () => { disposed = true; window.clearTimeout(timer); };
  }, [meetingId]);
  const command = async (name: string, args: Record<string, unknown>) => {
    if (pending.current) return false;
    pending.current = true; setBusy(true); setError(null);
    try { await invoke(name, args); setView(await invoke<View>("processing_status", { meetingId })); return true; }
    catch (cause) { setError(String(cause)); return false; }
    finally { pending.current = false; setBusy(false); }
  };
  return <Context.Provider value={{ view, error: error || pollError, busy, selectMeeting, command }}>{children}</Context.Provider>;
}
