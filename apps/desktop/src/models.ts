import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Models are downloaded when they are first needed, one pack at a time. `ensurePack` is the single door: call it before anything that
 * needs a pack, and it either says "ready" straight away or asks the person (size shown), downloads with progress, and answers.
 */
export type PackId = "standard" | "high" | "ultra" | "search" | "eyes" | "faces" | "scenetext";
export type PackInfo = { id: PackId; installed: boolean; bundled?: boolean };
/** `standard`：问的是大模型，人选了"先用标准模型"（只在 `offerStandard` 时出现）。 */
export type Answer = "ready" | "downloaded" | "cancelled" | "standard";
/**
 * `id` names the dialog's text: a pack, or "bundle" when several are fetched under one question (`ids` says which, `sizes` how big each is).
 * `hidden`：下载转到了后台（对话框收起，侧栏显示进度，点一下叫回来）；出错时对话框自己回来。
 */
export type Gate = { id: PackId | "bundle"; ids?: PackId[]; sizes?: number[]; phase: "ask" | "downloading" | "error"; bytes: number; done: number; total: number; error?: string; hidden?: boolean; offerStandard?: boolean; resolve: (a: Answer) => void };

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
 * 核心同时只下一个包，所以"问 + 下载"整体排成一条 Promise 链：后来的等前面那个答完（下完、取消或选了标准模型）再问自己的，
 * 排着的每一个都拿到自己的回答（审查第 47 条：以前两个以上排队时，后一个覆盖前一个的对话框，前一个永远等不到结果）。
 * 排队时前面那个要是转在后台，叫回前台，让人知道在等什么。
 */
let queue: Promise<unknown> = Promise.resolve();
function serial<T>(job: () => Promise<T>): Promise<T> {
  if (gate?.hidden) setGate({ ...gate, hidden: false });
  const run = queue.then(job, job);
  queue = run.catch(() => undefined);
  return run;
}

/** 眼下还缺哪些包（先刷新一次已装列表）；不在应用里（拿不到列表）时当作都不缺。 */
async function missingOf(ids: PackId[]): Promise<PackId[]> {
  await refreshPacks();
  if (!packs.length) return [];
  return [...new Set(ids)].filter((id) => !packs.find((p) => p.id === id)?.installed);
}

export async function ensurePack(id: PackId, offerStandard = false): Promise<Answer> {
  return ensurePacks([id], offerStandard);
}

/**
 * Several packs under one question: the person is told the whole size once and agrees once. What is already here is left out; answers "ready"
 * when nothing is missing.
 * 先算缺哪些：都在就马上返回，不管别处是不是正在下别的包（审查第 46 条：后台下搜索模型时，分析要的模型都在也被卡住）；
 * 真要下载才排进队里等前一个答完，轮到时再算一遍（前一个可能正好把它装上了）。
 */
export async function ensurePacks(ids: PackId[], offerStandard = false): Promise<Answer> {
  if (!(await missingOf(ids)).length) return "ready";
  return serial(async () => {
    const missing = await missingOf(ids);
    return missing.length ? ask(missing, offerStandard) : "ready";
  });
}

/** 问一句（大小写清楚）再下载；回答在下完 / 取消时才给。只在队里调用，同一时间只有一个。 */
async function ask(missing: PackId[], offerStandard: boolean): Promise<Answer> {
  const big = (id: PackId) => id === "high" || id === "ultra";
  if (missing.length === 1) {
    const id = missing[0];
    let bytes = 0;
    try { bytes = await invoke<number>("pack_size", { id }); } catch (e) {
      return new Promise((resolve) => setGate({ id, phase: "error", bytes: 0, done: 0, total: 0, error: String(e), resolve }));
    }
    return new Promise((resolve) => setGate({ id, phase: "ask", bytes, done: 0, total: bytes, offerStandard: offerStandard && big(id), resolve }));
  }
  const sizes: number[] = [];
  for (const id of missing) {
    try { sizes.push(await invoke<number>("pack_size", { id })); } catch (e) {
      return new Promise((resolve) => setGate({ id: "bundle", ids: missing, phase: "error", bytes: 0, done: 0, total: 0, error: String(e), resolve }));
    }
  }
  const bytes = sizes.reduce((a, b) => a + b, 0);
  return new Promise((resolve) => setGate({ id: "bundle", ids: missing, sizes, phase: "ask", bytes, done: 0, total: bytes, offerStandard: offerStandard && missing.some(big), resolve }));
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

/** 询问框里选了"先用标准模型"：不下这一份，等着的一方换成标准模型再问。 */
export function chooseStandard() {
  const g = gate;
  if (!g || g.phase !== "ask") return;
  setGate(null);
  g.resolve("standard");
}

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
