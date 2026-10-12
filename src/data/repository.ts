import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Recording } from "./recording";
import type { Completion, Meeting, Workspace } from "./types";

export type Snapshot = { meetings: Meeting[]; workspaces: Workspace[]; completed: Completion; recordings: Recording[] };
export type DataRequest =
  | { kind: "load" }
  | { kind: "create_workspace" | "update_workspace"; workspace: Workspace }
  | { kind: "create_meeting" | "update_meeting"; meeting: Meeting; title_edited?: boolean }
  | { kind: "delete_workspace" | "delete_meeting"; id: number }
  | { kind: "complete"; key: string; value: boolean };

/** Only this adapter knows about the desktop IPC. SQL and file paths stay in Rust. */
export async function dataRequest(request: DataRequest): Promise<Snapshot> {
  if (!isTauri()) throw new Error("Local storage is available in the desktop app. Start it with npm run desktop:dev.");
  return invoke<Snapshot>("data_request", { request });
}
