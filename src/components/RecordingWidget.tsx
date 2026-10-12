import { invoke } from "@tauri-apps/api/core";
import { useDesktopWidget } from "../data/desktopWidget";
import { formatTime } from "../data/simulation";
import { useRecording } from "../data/recording";
import { useState } from "react";

export function RecordingWidget() {
  const desktop = useDesktopWidget(500);
  const recorder = useRecording();
  const [dragError, setDragError] = useState<string | null>(null);
  const view = desktop.view;
  const status = recorder.ready ? recorder.status : view?.recording;
  const suggestion = view?.mode === "suggestion" && !recorder.active;
  const failed = status?.state === "failed";
  const busy = desktop.busy || recorder.busy;
  const error = recorder.error || desktop.error || dragError || (failed ? status?.error : null);
  return <main className={`floating-widget${view?.mode === "recording" ? " compact" : ""}`} aria-label="Meeting recording widget">
    <header className="widget-drag" onPointerDown={event => {
      if (event.button !== 0 || (event.target as HTMLElement).closest("button")) return;
      event.preventDefault();
      void invoke("desktop_widget_drag").catch(cause => setDragError(String(cause)));
    }}>
      <span className="widget-brand">gather<span aria-hidden="true">✳</span></span>
      <span className={`widget-indicator ${suggestion ? "suggestion" : status?.state === "paused" ? "paused" : ""}`}><i />{suggestion ? "Likely meeting" : failed ? "Audio retained" : status?.state === "stopping" ? "Saving…" : status?.state === "paused" ? "Paused" : "Recording"}</span>
    </header>
    {!view || !status ? <p role="status">Connecting to recording controls…</p>
      : suggestion ? <section className="widget-suggestion">
        <h1>Meeting detected.<br />Would you like to record?</h1>
        <p className="muted">{view.suggestion?.application} · Heuristic suggestion</p>
        <p className="widget-evidence" title={view.suggestion?.explanation}>Review your title and audio settings before capture starts.</p>
        <div className="widget-actions"><button className="dark" disabled={busy} onClick={() => void desktop.command("desktop_widget_open", { setup: true })}>Start Recording</button><button className="text" disabled={busy} onClick={() => void desktop.command("desktop_widget_dismiss")}>Dismiss</button></div>
      </section>
        : <section className="widget-capture">
          <div className="widget-timer" role="timer" aria-label={`${Math.floor(status.elapsed_ms / 1000)} seconds captured`}>{formatTime(Math.floor(status.elapsed_ms / 1000))}</div>
          {failed ? <><p className="widget-evidence">Your captured audio is retained. Open the app to check recovery.</p><button className="dark" disabled={busy} onClick={async () => {
            if (await desktop.command("desktop_widget_open", { setup: false })) await desktop.command("desktop_widget_acknowledge");
          }}>Open recovery</button></>
            : <div className="widget-actions">
              {(status.state === "recording" || status.state === "paused") && <><button className="pill" disabled={busy} onClick={() => void recorder.control(status.state === "paused" ? "resume" : "pause")}>{status.state === "paused" ? "Resume" : "Pause"}</button><button className="dark" disabled={busy} onClick={() => void recorder.control("stop")}>Stop & Save</button></>}
              {(status.state === "stopping" || status.state === "saved") && <p role="status">Finalizing and saving your audio…</p>}
            </div>}
          {status.warning && <p className="widget-evidence" role="status">{status.warning}</p>}
        </section>}
    {error && <p className="widget-error" role="alert">{error}</p>}
  </main>;
}
