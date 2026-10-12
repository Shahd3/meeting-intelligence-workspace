import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { RecordingWidget } from "./components/RecordingWidget";
import { App } from "./app/App";
import "./styles/global.css";

const widget = isTauri() && getCurrentWebviewWindow().label === "recording-widget";
if (widget) document.body.classList.add("widget-window");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {widget ? <RecordingWidget /> : <App />}
  </StrictMode>,
);
