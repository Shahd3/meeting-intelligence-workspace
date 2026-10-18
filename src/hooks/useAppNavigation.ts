import { useCallback, useEffect, useRef, useState } from "react";
import type { Meeting, Workspace, MeetingTab, Page } from "../data/types";

type Location = { page: Page; meetingId?: number; workspaceId?: number };

export function useAppNavigation({ meetings, workspaces, ready }: { meetings: Meeting[]; workspaces: Workspace[]; ready: boolean }) {
  const [location, setLocation] = useState<Location>({ page: "home" });
  const [tab, setTab] = useState<MeetingTab>("Summary");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");

  const previousView = useRef("");
  const meeting = meetings.find(item => item.id === location.meetingId);
  const workspace = workspaces.find(item => item.id === location.workspaceId);
  const dashboard = ready && location.page === "home" && !meeting && !workspace;
  const viewKey = `${location.page}:${location.meetingId || ""}:${location.workspaceId || ""}`;


  useEffect(() => {
    document.body.classList.toggle("dashboard", dashboard);
    return () => document.body.classList.remove("dashboard");
  }, [dashboard]);
  useEffect(() => {
    if (previousView.current && previousView.current !== viewKey) {
      window.scrollTo(0, 0);
      const heading = document.querySelector<HTMLElement>("#view h1");
      heading?.setAttribute("tabindex", "-1"); heading?.focus({ preventScroll: true });
    }
    previousView.current = viewKey;
  }, [viewKey]);


  const navigate = useCallback((page: Page) => { setLocation({ page }); setQuery(""); setFilter("all"); }, []);
  const openMeeting = (id: number) => { setLocation(current => ({ ...current, meetingId: id })); setTab("Summary"); };
  const openWorkspace = (id: number) => setLocation({ page: "spaces", workspaceId: id });

  const backFromMeeting = () => setLocation(current => ({ ...current, meetingId: undefined }));
  return { location, tab, setTab, query, setQuery, filter, setFilter, meeting, workspace, dashboard, navigate, openMeeting, openWorkspace, backFromMeeting };
}
