import { useSyncExternalStore } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type Compute = "auto" | "cpu";
export type Theme = "system" | "light" | "dark";
export type Quality = "fast" | "high" | "ultra";
export type EyeMethod = "vision" | "mediapipe";
/**
 * `compute`, `quality`, `search`, `customStyles` and `eyeMethod` used to be choices on the settings page. The app decides those itself now
 * (strategy.ts); they stay here only so that what an earlier version stored keeps its meaning. `tier` is the analysis the app settled on:
 * one for the whole app (the learned taste, search and grouping all compare embeddings, which only agree when one model made them), null until
 * the first analysis.
 */
export type Settings = { compute: Compute; theme: Theme; learn: boolean; quality: Quality; search: boolean; customStyles: boolean; eyeMethod: EyeMethod; tier: Quality | null; /** counts the times a better model was chosen on purpose (the project in view is looked at again then) */ tierRev: number };
/** What the last analysis actually ran on, reported by the backend. */
export type ScanInfo = { cpu_only: boolean; clip: string | null; model: string | null; vision: boolean; cores: number };

const KEY = "fg.settings";
const INFO_KEY = "fg.scaninfo";
const subs = new Set<() => void>();
const notify = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };

function read<T>(key: string, fallback: T): T {
  try { const raw = localStorage.getItem(key); return raw ? { ...fallback, ...JSON.parse(raw) } : fallback; } catch { return fallback; }
}
function write(key: string, v: unknown) { try { localStorage.setItem(key, JSON.stringify(v)); } catch { /* storage may be unavailable */ } }

let state: Settings = read<Settings>(KEY, { compute: "auto", theme: "system", learn: true, quality: "fast", search: false, customStyles: false, eyeMethod: "vision", tier: null, tierRev: 0 });
// Someone who has analysed photos before keeps the model their results were made with (changing it would redo everything).
(() => {
  try {
    if ("tier" in JSON.parse(localStorage.getItem(KEY) ?? "{}")) return;
    const projects = (JSON.parse(localStorage.getItem("fg.projects") ?? "{}") as { projects?: { cover?: string[] }[] }).projects ?? [];
    state = { ...state, tier: projects.some((p) => (p.cover?.length ?? 0) > 0) ? state.quality : null };
    write(KEY, state);
  } catch { /* storage may be unavailable */ }
})();
let info: ScanInfo | null = (() => { try { const raw = localStorage.getItem(INFO_KEY); return raw ? (JSON.parse(raw) as ScanInfo) : null; } catch { return null; } })();

/** Forces light/dark on the page and on the native window (so the translucent sidebar follows too). */
export function applyTheme(t: Theme) {
  const root = document.documentElement;
  if (t === "system") root.removeAttribute("data-theme"); else root.setAttribute("data-theme", t);
  try { getCurrentWindow().setTheme(t === "system" ? null : t).catch(() => { /* not in Tauri */ }); } catch { /* not in Tauri */ }
}

export function setSettings(patch: Partial<Settings>) {
  state = { ...state, ...patch };
  write(KEY, state);
  if (patch.theme) applyTheme(state.theme);
  notify();
}
export const getSettings = () => state;
export function setScanInfo(i: ScanInfo) { info = i; write(INFO_KEY, i); notify(); }

export const useSettings = () => useSyncExternalStore(subscribe, () => state);
/** 设置变化的通知（云备份靠它知道该上传了）。 */
export const subscribeSettings = subscribe;
/* Bumped when the thumbnail cache is cleared: the project in view reads its folders again (and finds everything needs analysing). */
let cacheRev = 0;
export function bumpCacheRev() { cacheRev += 1; notify(); }
export const useCacheRev = () => useSyncExternalStore(subscribe, () => cacheRev);
export const useScanInfo = () => useSyncExternalStore(subscribe, () => info);

if (state.theme !== "system") applyTheme(state.theme);
