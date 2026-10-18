import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useRecording } from "../data/recording";
import { useDesktopWidget } from "../data/desktopWidget";
import { useProcessing } from "../data/processing";
import { savedMeetingAvailable } from "../data/recordingFlow";
import type { useLocalData } from "../data/useLocalData";
import type { Page } from "../data/types";

type Options = Pick<ReturnType<typeof useLocalData>, "ready" | "busy" | "run"> & {
  dismiss: () => void;
  openRecording: () => void;
  navigate: (page: Page) => void;
  notify: (text: string) => void;
};

export function useRecordingFlow({ ready, busy, run, dismiss, openRecording, navigate, notify }: Options) {
  const processing = useProcessing();
  const titleReceipts = processing.view?.intelligence.jobs.filter(j => j.state === "completed" && j.has_result).map(j => j.id).join("|") || "";
  const refreshedTitles = useRef("");
  useEffect(() => { if (ready && !busy && titleReceipts && refreshedTitles.current !== titleReceipts) { refreshedTitles.current = titleReceipts; void run({ kind: "load" }); } }, [ready, busy, titleReceipts, run]);
  const recorder = useRecording();
  const desktop = useDesktopWidget();

  const [recordingOpen, setRecordingOpen] = useState(0);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const observedRecording = useRef<string | null>(null);
  const refreshedRecording = useRef<string | null>(null);
  const refreshPending = useRef(false);
  const [retrySave, setRetrySave] = useState(0);
  useEffect(() => {
    if (recorder.active && recorder.status.id) observedRecording.current = recorder.status.id;
    const id = recorder.status.id;
    if (!id || recorder.active || !["saved", "failed"].includes(recorder.status.state) || refreshedRecording.current === id || busy || refreshPending.current) return;
    refreshedRecording.current = id;
    refreshPending.current = true;
    const returnToLibrary = recorder.status.state === "saved" && observedRecording.current === id;
    if (returnToLibrary) { setSaving(true); setSaveError(null); }
    void run({ kind: "load" }).then(data => {
      if (returnToLibrary) {
        if (savedMeetingAvailable(recorder.status, data?.meetings)) {
          dismiss(); navigate("notes");
          notify("Recording saved. Your meeting is ready in Note Taker.");
          observedRecording.current = null; setSaving(false);
        } else {
          setSaveError("Audio was saved, but the meeting list could not be refreshed. Retry to reopen your library.");
        }
      }
    }).finally(() => { refreshPending.current = false; });
  }, [recorder.status.id, recorder.status.state, recorder.status.meeting_id, recorder.active, busy, run, retrySave, dismiss, navigate, notify]);
  const retrySaved = () => { refreshedRecording.current = null; setRetrySave(value => value + 1); };

  const onRecovered = async () => {
    if (await run({ kind: "load" })) {
      observedRecording.current = null;
      setSaving(false);
      setSaveError(null);
      dismiss(); navigate("notes");
      notify("Recovery checked. Saved meetings are available in Note Taker.");
    }
  };
  const recover = async () => {
    if (await recorder.recover()) await onRecovered();
  };
  const record = useCallback(() => { setRecordingOpen(value => value + 1); openRecording(); }, [openRecording]);
  useEffect(() => {
    if (!desktop.view?.setup_requested) return;
    void invoke<boolean>("desktop_widget_take_setup").then(requested => {
      if (requested) record();
    }).catch(cause => notify(String(cause)));
  }, [desktop.view?.setup_requested, record, notify]);

  return { recorder, desktop, recordingOpen, saving, saveError, retrySaved, record, recover, onRecovered, observedRecording };
}
