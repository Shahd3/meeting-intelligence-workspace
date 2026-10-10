import { useState } from "react";
import { AppShell } from "../components/AppShell";
import { DashboardPage } from "../pages/DashboardPage";
import { MeetingsPage } from "../pages/MeetingsPage";
import { ProjectsPage } from "../pages/ProjectsPage";
import { SettingsPage } from "../pages/SettingsPage";
import type { Destination } from "./navigation";

const pages = {
  Dashboard: DashboardPage,
  Meetings: MeetingsPage,
  Projects: ProjectsPage,
  Settings: SettingsPage,
};

export function App() {
  const [activePage, setActivePage] = useState<Destination>("Dashboard");
  const Page = pages[activePage];

  return (
    <AppShell activePage={activePage} onNavigate={setActivePage}>
      <Page />
    </AppShell>
  );
}
