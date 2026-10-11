import { useCallback, useEffect, useRef, useState } from "react";
import { dataRequest, type DataRequest, type Snapshot } from "./repository";

const empty: Snapshot = { meetings: [], workspaces: [], completed: {} };
export function useLocalData() {
  const [data, setData] = useState(empty);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pending = useRef(false);
  const run = useCallback(async (request: DataRequest) => {
    if (pending.current) return null;
    pending.current = true; setBusy(true); setError(null);
    try {
      const snapshot = await dataRequest(request);
      setData(snapshot); setReady(true);
      return snapshot;
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : typeof cause === "string" ? cause : "Unable to access local data. Please reload and try again.");
      return null;
    } finally { pending.current = false; setBusy(false); }
  }, []);
  useEffect(() => { void run({ kind: "load" }); }, [run]);
  return { ...data, ready, busy, error, run };
}
