import React from "react";
import ReactDOM from "react-dom/client";
import "./i18n";
import App from "./App";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { flushProjects, loadProjects } from "./projects";

async function boot() {
  // `?mock` runs the UI in a plain browser with fake photos and a simulated scan (dev only).
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock") && !("__TAURI_INTERNALS__" in window)) {
    await import("./dev/mock");
  }
  void loadProjects();
  // 退出（⌘Q）或关窗前：原生层先拦下来发 "flush-before-quit"，这里把还在防抖里的项目写完，再让它真的退出
  listen("flush-before-quit", () => { void flushProjects().finally(() => invoke("quit_ready").catch(() => { /* not in Tauri */ })); }).catch(() => { /* not in Tauri */ });
  // 浏览器预览（写 localStorage，同步）：页面关掉前也冲一次
  window.addEventListener("pagehide", () => { void flushProjects(); });
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
boot();
