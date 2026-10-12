import { invoke, isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { RecordingStatus } from "./recording";
export type DesktopWidgetView = {
  mode: "hidden" | "suggestion" | "recording" | "failed";
  recording: RecordingStatus;
  suggestion: { application: string; explanation: string } | null;
  detection_enabled: boolean;
  detection_error: string | null;
  widget_error: string | null;
  setup_requested: boolean;
};
export function useDesktopWidget(interval = 1000) {
  const [view, setView] = useState<DesktopWidgetView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let timer: number;
    const poll = async () => {
      try {
        const next = await invoke<DesktopWidgetView>("desktop_widget_status");
        if (!disposed) { setView(next); }
      } catch (cause) { if (!disposed) setError(String(cause)); }
      if (!disposed) timer = window.setTimeout(() => void poll(), interval);
    };
    void poll();
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [interval]);
  const command = useCallback(async (name: string, args?: Record<string, unknown>) => {
    if (pending.current) return false;
    pending.current = true; setBusy(true); setError(null);
    try { await invoke(name, args); setView(await invoke<DesktopWidgetView>("desktop_widget_status")); return true; }
    catch (cause) { setError(String(cause)); return false; }
    finally { pending.current = false; setBusy(false); }
  }, []);
  return { view, error, busy, command };
}
