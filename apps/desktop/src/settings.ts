import { useSyncExternalStore } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type Compute = "auto" | "cpu";
export type Theme = "system" | "light" | "dark";
export type Quality = "fast" | "high" | "ultra";
export type EyeMethod = "vision" | "mediapipe";
export type Settings = { compute: Compute; theme: Theme; learn: boolean; quality: Quality; search: boolean; customStyles: boolean; eyeMethod: EyeMethod };
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

let state: Settings = read<Settings>(KEY, { compute: "auto", theme: "system", learn: true, quality: "fast", search: false, customStyles: false, eyeMethod: "vision" });
// High quality needs acceleration, so an old combination of High + CPU-only is brought in line.
if (state.quality !== "fast" && state.compute === "cpu") state = { ...state, compute: "auto" };
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
export const useScanInfo = () => useSyncExternalStore(subscribe, () => info);

if (state.theme !== "system") applyTheme(state.theme);
