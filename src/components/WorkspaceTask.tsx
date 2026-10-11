import type { Workspace } from "../data/types";

export function WorkspaceTask({ workspace, checked, onChange }: { workspace: Workspace; checked: boolean; onChange: (key: string, value: boolean) => void }) {
  if (!workspace.task) return <p className="muted">No outstanding tasks yet.</p>;
  const key = `workspace-${workspace.id}`;
  return <div className="task"><input type="checkbox" id={key} checked={checked} onChange={event => onChange(key, event.target.checked)} />
    <label htmlFor={key}>{workspace.task}<small>{workspace.owner} · {workspace.name}</small></label><span className="tag">{workspace.due}</span>
  </div>;
}
