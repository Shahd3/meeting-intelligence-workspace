import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";

export type RecordingState = "idle" | "starting" | "recording" | "paused" | "stopping" | "saved" | "failed";
export type RecordingStatus = { state: RecordingState; id: string | null; elapsed_ms: number; meeting_id: number | null; error: string | null; warning: string | null; peak: number };
export type AudioDevice = { id: string; name: string };
export type AudioDevices = { microphones: AudioDevice[]; outputs: AudioDevice[] };
export type AudioSelection = { title: string; microphone: boolean; system: boolean; microphone_device: string | null; system_device: string | null };
export type RecordingTrack = { role: "microphone" | "system"; device_id: string; device_name: string; filename: string; frames: number; sample_rate: number; channels: number };
export type Recording = { id: string; meeting_id: number | null; title: string; state: string; elapsed_ms: number; recovered: boolean; complete: boolean; warning: string | null; created_at: string; tracks: RecordingTrack[] };
const idle: RecordingStatus = { state: "idle", id: null, elapsed_ms: 0, meeting_id: null, error: null, warning: null, peak: 0 };
export const recordingActive = (status: RecordingStatus) => ["starting", "recording", "paused", "stopping"].includes(status.state);
const message = (cause: unknown) => cause instanceof Error ? cause.message : typeof cause === "string" ? cause : "Recording control is unavailable. Audio already captured is retained.";

async function request<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new Error("Audio capture requires the Windows desktop app. Start it with npm run desktop:dev.");
  return invoke<T>(command, args);
}
export const discoverDevices = () => request<AudioDevices>("recording_devices");
// convertFileSrc encodes its entire argument, including slashes. Only the UUID is
// passed to it; the validated source role is a separate protocol path segment.
export const playbackUrl = (recording: Recording, track: RecordingTrack) => `${convertFileSrc(recording.id, "recording")}/${track.role}`;

/** React observes the Rust worker. Unmounting a dialog never owns or stops capture. */
export function useRecording() {
  const [status, setStatus] = useState(idle);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [ready, setReady] = useState(false);
  const pending = useRef(false);
  const revision = useRef(0);
  useEffect(() => {
    let disposed = false;
    let timer: number;
    const poll = async () => {
      try { if (!pending.current) { const version = revision.current; const next = await request<RecordingStatus>("recording_status"); if (!disposed && !pending.current && version === revision.current) { setStatus(next); setReady(true); } } }
      catch (cause) { if (!disposed) setError(message(cause)); }
      if (!disposed) timer = window.setTimeout(() => void poll(), 250);
    };
    void poll();
    return () => { disposed = true; window.clearTimeout(timer); };
  }, []);
  const command = useCallback(async (name: string, args?: Record<string, unknown>) => {
    if (pending.current) return false;
    pending.current = true; revision.current += 1; setBusy(true); setError(null);
    try { await request(name, args); setStatus(await request<RecordingStatus>("recording_status")); return true; }
    catch (cause) { setError(message(cause)); return false; }
    finally { pending.current = false; setBusy(false); }
  }, []);
  return { status, error, busy, ready, active: recordingActive(status),
    start: (selection: AudioSelection) => command("recording_start", { selection }),
    control: (action: "pause" | "resume" | "stop") => command("recording_control", { action }),
    recover: () => command("recording_recover"),
  };
}
export type RecordingController = ReturnType<typeof useRecording>;
