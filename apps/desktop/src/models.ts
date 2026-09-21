import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Models are downloaded when they are first needed, one pack at a time. `ensurePack` is the single door: call it before anything that
 * needs a pack, and it either says "ready" straight away or asks the person (size shown), downloads with progress, and answers.
 */
export type PackId = "standard" | "high" | "ultra" | "search";
export type PackInfo = { id: PackId; installed: boolean };
export type Answer = "ready" | "downloaded" | "cancelled";
export type Gate = { id: PackId; phase: "ask" | "downloading" | "error"; bytes: number; done: number; total: number; error?: string; resolve: (a: Answer) => void };

let packs: PackInfo[] = [];
let gate: Gate | null = null;
const subs = new Set<() => void>();
const notify = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };

export const usePacks = () => useSyncExternalStore(subscribe, () => packs);
export const useGate = () => useSyncExternalStore(subscribe, () => gate);

export async function refreshPacks() {
  try { packs = await invoke<PackInfo[]>("packs_status"); notify(); } catch { /* not running inside the app */ }
}

const setGate = (g: Gate | null) => { gate = g; notify(); };

export async function ensurePack(id: PackId): Promise<Answer> {
  await refreshPacks();
  if (!packs.length || packs.find((p) => p.id === id)?.installed) return "ready";
  let bytes = 0;
  try { bytes = await invoke<number>("pack_size", { id }); } catch (e) {
    return new Promise((resolve) => setGate({ id, phase: "error", bytes: 0, done: 0, total: 0, error: String(e), resolve }));
  }
  return new Promise((resolve) => setGate({ id, phase: "ask", bytes, done: 0, total: bytes, resolve }));
}

export function startDownload() {
  const g = gate;
  if (!g) return;
  setGate({ ...g, phase: "downloading", done: 0, error: undefined });
  invoke("pack_download", { id: g.id })
    .then(async () => { await refreshPacks(); setGate(null); g.resolve("downloaded"); })
    .catch((e) => {
      if (String(e).includes("cancelled")) { setGate(null); g.resolve("cancelled"); }
      else setGate({ ...g, phase: "error", error: String(e) });
    });
}

/** Closes the dialog (before or during a download) and answers "cancelled". */
export function cancelGate() {
  const g = gate;
  if (!g) return;
  if (g.phase === "downloading") { void invoke("pack_cancel"); return; }
  setGate(null);
  g.resolve("cancelled");
}

export async function removePack(id: PackId) {
  try { await invoke("pack_remove", { id }); } finally { await refreshPacks(); }
}

void listen<{ pack: string; done: number; total: number }>("model-progress", (e) => {
  if (gate && gate.phase === "downloading") setGate({ ...gate, done: e.payload.done, total: e.payload.total });
}).catch(() => { /* not running inside the app */ });
void refreshPacks();
