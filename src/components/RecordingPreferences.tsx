import { useDesktopWidget } from "../data/desktopWidget";
export function RecordingPreferences() {
  const desktop = useDesktopWidget();
  return <section className="recording-preferences">
    <p className="prose">Recording controls float above your desktop during capture. Suggestions are optional and never start recording for you.</p>
    <label className="preference-toggle"><span><strong>Automatic meeting detection</strong><small>Local suggestions for supported meeting apps</small></span><input type="checkbox" role="switch" checked={desktop.view?.detection_enabled ?? false} disabled={!desktop.view || desktop.busy} onChange={event => void desktop.command("desktop_detection_set", { enabled: event.target.checked })} /></label>
    <p className="muted">Checks supported app windows and existing microphone activity every five seconds. No audio is captured for detection. Signals are heuristic; some meetings may not be identified.</p>
    <p className="muted">Teams and Zoom can qualify with an active microphone session. An explicit Zoom meeting window can also suggest a meeting. Google Meet requires a recognizable call tab in Chrome or Edge plus browser microphone activity.</p>
    {(desktop.error || desktop.view?.detection_error || desktop.view?.widget_error) && <p className="recording-error" role="alert">{desktop.error || desktop.view?.detection_error || desktop.view?.widget_error}</p>}
  </section>;
}
