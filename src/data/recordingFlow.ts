import type { AudioDevices, AudioSelection, Recording, RecordingStatus } from "./recording";

export function sessionView(status: RecordingStatus, sessionId: string | null): RecordingStatus {
  const active = ["starting", "recording", "paused", "stopping"].includes(status.state);
  return active || (sessionId !== null && sessionId === status.id) ? status : { state: "idle", id: null, elapsed_ms: 0, meeting_id: null, error: null, warning: null, peak: 0 };
}
export function needsRecovery(recording: Recording): boolean {
  return recording.state === "failed" && recording.tracks.some(track => track.filename.endsWith(".part"));
}

export function unavailableSources(selection: AudioSelection, devices: AudioDevices | null) {
  const available = (items: AudioDevices["microphones"] | undefined, id: string | null) => !!items?.length && (!id || items.some(device => device.id === id));
  return {
    microphone: selection.microphone && !available(devices?.microphones, selection.microphone_device),
    system: selection.system && !available(devices?.outputs, selection.system_device),
  };
}

/** Navigation is allowed only after both finalization and the matching list refresh. */
export function savedMeetingAvailable(status: RecordingStatus, meetings: { id: number }[] | undefined): boolean {
  return status.state === "saved" && status.meeting_id !== null && !!meetings?.some(meeting => meeting.id === status.meeting_id);
}
