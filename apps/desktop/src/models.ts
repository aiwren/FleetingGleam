import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Models are downloaded when they are first needed, one pack at a time. `ensurePack` is the single door: call it before anything that
 * needs a pack, and it either says "ready" straight away or asks the person (size shown), downloads with progress, and answers.
 */
export type PackId = "standard" | "high" | "ultra" | "search" | "eyes" | "faces" | "scenetext";
export type PackInfo = { id: PackId; installed: boolean; bundled?: boolean };
export type Answer = "ready" | "downloaded" | "cancelled";
/**
 * `id` names the dialog's text: a pack, or "bundle" when several are fetched under one question (`ids` says which, `sizes` how big each is).
 * `hidden`：下载转到了后台（对话框收起，侧栏显示进度，点一下叫回来）；出错时对话框自己回来。
 */
export type Gate = { id: PackId | "bundle"; ids?: PackId[]; sizes?: number[]; phase: "ask" | "downloading" | "error"; bytes: number; done: number; total: number; error?: string; hidden?: boolean; resolve: (a: Answer) => void };

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

/** Whether a pack is on this machine (as of the last look). */
export const packInstalled = (id: PackId) => !!packs.find((p) => p.id === id)?.installed;

/**
 * 核心同时只下一个包：已经有一个在问 / 在下时，后来的先等它结束（转到后台的那个叫回前台，让人知道在等什么），再问自己的。
 */
async function idle() {
  while (gate) {
    if (gate.hidden) setGate({ ...gate, hidden: false });
    await new Promise<void>((r) => { const un = subscribe(() => { if (!gate) { un(); r(); } }); });
  }
}

export async function ensurePack(id: PackId): Promise<Answer> {
  await idle();
  await refreshPacks();
  if (!packs.length || packs.find((p) => p.id === id)?.installed) return "ready";
  let bytes = 0;
  try { bytes = await invoke<number>("pack_size", { id }); } catch (e) {
    return new Promise((resolve) => setGate({ id, phase: "error", bytes: 0, done: 0, total: 0, error: String(e), resolve }));
  }
  return new Promise((resolve) => setGate({ id, phase: "ask", bytes, done: 0, total: bytes, resolve }));
}

/**
 * Several packs under one question: the person is told the whole size once and agrees once. What is already here is left out; answers "ready"
 * when nothing is missing.
 */
export async function ensurePacks(ids: PackId[]): Promise<Answer> {
  await idle();
  await refreshPacks();
  if (!packs.length) return "ready";
  const missing = [...new Set(ids)].filter((id) => !packs.find((p) => p.id === id)?.installed);
  if (!missing.length) return "ready";
  if (missing.length === 1) return ensurePack(missing[0]);
  const sizes: number[] = [];
  for (const id of missing) {
    try { sizes.push(await invoke<number>("pack_size", { id })); } catch (e) {
      return new Promise((resolve) => setGate({ id: "bundle", ids: missing, phase: "error", bytes: 0, done: 0, total: 0, error: String(e), resolve }));
    }
  }
  const bytes = sizes.reduce((a, b) => a + b, 0);
  return new Promise((resolve) => setGate({ id: "bundle", ids: missing, sizes, phase: "ask", bytes, done: 0, total: bytes, resolve }));
}

export function startDownload() {
  const g = gate;
  if (!g) return;
  listenProgress();
  setGate({ ...g, phase: "downloading", done: 0, error: undefined });
  const ids = g.ids ?? [g.id as PackId];
  // the packs one after the other; the progress of the one being fetched is added to the sizes of those already done
  (async () => {
    let before = 0;
    for (let i = 0; i < ids.length; i++) {
      base = before;
      await invoke("pack_download", { id: ids[i] });
      before += g.sizes?.[i] ?? 0;
    }
  })()
    .then(async () => { base = 0; await refreshPacks(); setGate(null); g.resolve("downloaded"); })
    .catch((e) => {
      base = 0;
      if (String(e).includes("cancelled")) { setGate(null); g.resolve("cancelled"); }
      else setGate({ ...g, phase: "error", error: String(e), hidden: false });
    });
}
/** Bytes of the packs of a bundle that are already down, while the next one is fetched. */
let base = 0;

/** 下载转到后台：对话框收起，应用照常可用；下完了等着的那一步照常接着走。 */
export function hideGate() { if (gate?.phase === "downloading") setGate({ ...gate, hidden: true }); }
/** 把后台的下载叫回前台。 */
export function showGate() { if (gate?.hidden) setGate({ ...gate, hidden: false }); }

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

/** 下载进度的监听在第一次下载时才挂上（模块载入时宿主可能还没就绪，浏览器里的 mock 就是这样）。 */
let listening = false;
function listenProgress() {
  if (listening) return;
  listening = true;
  void listen<{ pack: string; done: number; total: number }>("model-progress", (e) => {
    if (gate && gate.phase === "downloading") setGate({ ...gate, done: gate.ids ? Math.min(gate.bytes, base + e.payload.done) : e.payload.done, total: gate.ids ? gate.bytes : e.payload.total });
  }).catch(() => { listening = false; /* not running inside the app */ });
}
void refreshPacks();
