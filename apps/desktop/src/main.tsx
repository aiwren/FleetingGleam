import React from "react";
import ReactDOM from "react-dom/client";
import "./i18n";
import App from "./App";
import { loadProjects } from "./projects";

async function boot() {
  // `?mock` runs the UI in a plain browser with fake photos and a simulated scan (dev only).
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock") && !("__TAURI_INTERNALS__" in window)) {
    await import("./dev/mock");
  }
  void loadProjects();
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
boot();
