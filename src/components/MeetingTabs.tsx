import { meetingSections } from "../data/intelligence";
import type { MeetingTab } from "../data/types";

export function MeetingTabs({ tab, onTab }: { tab: MeetingTab; onTab: (tab: MeetingTab) => void }) {
  const tabs = meetingSections;
  return (
    <div className="tabs" role="tablist" aria-label="Meeting details">
      {tabs.map((item, index) => (
        <button key={item} id={`meeting-tab-${index}`} role="tab"
          aria-selected={tab === item} aria-controls="meeting-tab-panel"
          tabIndex={tab === item ? 0 : -1} className={`tab ${tab === item ? "active" : ""}`}
          onClick={() => onTab(item)}
          onKeyDown={event => {
            let next = index;
            if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
            else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
            else if (event.key === "Home") next = 0;
            else if (event.key === "End") next = tabs.length - 1;
            else return;
            event.preventDefault();
            onTab(tabs[next]);
            document.getElementById(`meeting-tab-${next}`)?.focus();
          }}>
          {item === "Action items" ? "Action Items" : item}
        </button>
      ))}
    </div>
  );
}
