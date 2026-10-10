import type { ReactNode } from "react";
import { destinations, type Destination } from "../app/navigation";

type AppShellProps = {
  activePage: Destination;
  onNavigate: (page: Destination) => void;
  children: ReactNode;
};

export function AppShell({ activePage, onNavigate, children }: AppShellProps) {
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">M</span>
          <span>Meeting Intelligence<br /><strong>Workspace</strong></span>
        </div>
        <nav aria-label="Main navigation">
          {destinations.map((page) => (
            <button
              key={page}
              type="button"
              aria-current={activePage === page ? "page" : undefined}
              onClick={() => onNavigate(page)}
            >
              {page}
            </button>
          ))}
        </nav>
        <p className="sidebar-note">Your space for meeting knowledge.</p>
      </aside>
      <main id="main-content" key={activePage}>
        <header className="page-header">
          <p className="eyebrow">Meeting Intelligence Workspace</p>
          <h1>{activePage}</h1>
        </header>
        {children}
      </main>
    </div>
  );
}
