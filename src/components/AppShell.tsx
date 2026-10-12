import type { ReactNode } from "react";
import type { Page } from "../data/types";

const navigation: { page: Page; label: string; symbol: string }[] = [
  { page: "home", label: "Home", symbol: "⌂" },
  { page: "notes", label: "Note Taker", symbol: "▤" },
  { page: "spaces", label: "Workspaces", symbol: "▦" },
];

export function AppShell({ page, dashboard, onNavigate, onAbout, onSettings, children }: {
  page: Page; dashboard: boolean; onNavigate: (page: Page) => void; onAbout: () => void; onSettings: () => void; children: ReactNode;
}) {
  return <div className={dashboard ? "app dashboard" : "app"}>
    <aside>
      <div className="brand">gather<i aria-hidden="true">✳</i></div>
      <div className="eyebrow">MEETING INTELLIGENCE</div>
      <nav aria-label="Main navigation">
        {navigation.map(item => <button key={item.page} className={`nav ${page === item.page ? "active" : ""}`}
          aria-label={item.label} aria-current={page === item.page ? "page" : undefined} onClick={() => onNavigate(item.page)}>
          <span className="icon" aria-hidden="true">{item.symbol}</span><span className="label">{item.label}</span>
        </button>)}
      </nav>
      <div className="side-note"><div className="flower" aria-hidden="true">✳</div>A little more clarity.<br />A lot more possibility.<p className="side-tagline">YOUR IDEAS, CONNECTED.</p></div>
      <div className="profile"><span className="avatar" aria-hidden="true">SA</span><div className="label">Shahd Adel<br /><small>Personal workspace</small></div></div>
    </aside>
    <main className="content">
      <header className="topbar"><span className="eyebrow">YOUR SPACE TO THINK CLEARLY</span>
        <div className="topright"><span className="pill date-pill">◷ &nbsp; Saturday, October 10</span><button className="circle" onClick={onSettings} aria-label="Recording preferences">⚙</button><button className="circle" onClick={onAbout} aria-label="About this demo">?</button><span className="avatar sage" aria-hidden="true">SA</span></div>
      </header>
      <div id="view">{children}</div>
    </main>
  </div>;
}
