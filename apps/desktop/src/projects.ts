import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { State } from "./types";

/** A project is a named set of folders plus the decisions made on its photos. Analysis results live in the backend cache. */
export type Project = {
  id: string;
  name: string;
  folders: string[];
  overrides: Record<string, State>;
  /** 手动打的星级（1–5），按照片路径；没打过的不在表里。和保留 / 淘汰互不影响，写回 Lightroom 时优先于应用的评分。 */
  stars?: Record<string, number>;
  /** Legacy: the old keep quota. Ignored now; see `keepCap`. */
  keepRatio: number;
  /** At most this share of the photos is kept (a ceiling for when you want fewer). Unset = keep every good photo. */
  keepCap?: number;
  /** true = of a group of similar shots only the best can be kept (the others go to review or are rejected). Off by default. */
  onePerGroup?: boolean;
  count: number;
  /** Up to four thumbnail paths shown on the project card. */
  cover?: string[];
  /** Default sort for this project. */
  sort?: "group" | "order" | "time" | "score";
  /** Which selection style judges this project (unset = the default style). */
  style?: string;
  /** How picks favour coverage: "diverse" (default) spreads keeps across scenes, "best" just takes the top scores. */
  pickMode?: "diverse" | "best";
  /** Free text the user keeps about the shoot (who, where, what to look for). */
  note?: string;
  /** Search on or off for this project; when unset it follows the app setting. */
  search?: boolean;
  /** false = this project ignores the learned taste model. */
  learn?: boolean;
  updated: number;
  /** Set when the project is in the recycle bin (ms timestamp). Its photo files are never touched. */
  trashed?: number;
};
/** The recycle bin only holds what was deleted within this many days; older items are removed when the app opens. */
export const TRASH_DAYS = 7;
type Store = { projects: Project[]; activeId: string | null };
/**
 * Whether the projects have been read yet, and what went wrong: `saveError` is the last write that failed (null when the last one went
 * through), `loadError` is "corrupt" when the file was there but unreadable (set aside, started empty) or the backend's message when it
 * could not be read at all; in that case `readOnly` is true and nothing is written this session, so the file is not replaced.
 */
export type ProjectsStatus = { loaded: boolean; saveError: string | null; loadError: string | null; readOnly: boolean };

/*
 * The projects are the person's own work (hours of keep / reject decisions), so they are kept by the backend in
 * `projects.json`, written whole and atomically, never in the WebView's storage: that has a quota of a few MB that a
 * handful of large projects can hit, and a write that hits it fails silently. The WebView's storage is only read once,
 * to carry over the projects of versions before this one, and in the browser preview (where there is no backend).
 */
const KEY = "fg.projects";
const subs = new Set<() => void>();
const notify = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };

let store: Store = { projects: [], activeId: null };
let status: ProjectsStatus = { loaded: false, saveError: null, loadError: null, readOnly: false };
/** Without the app's backend (the plain browser preview) the WebView's storage is all there is. */
const backend = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** The stored form, checked and with the recycle bin's expired items removed; null when it is not a projects store at all. */
function parse(raw: string): { store: Store; purged: boolean } | null {
  try {
    const v = JSON.parse(raw) as Store;
    if (!Array.isArray(v.projects)) return null;
    const cutoff = Date.now() - TRASH_DAYS * 86400_000;
    const projects = v.projects.filter((p) => !p.trashed || p.trashed > cutoff);
    const active = projects.find((p) => p.id === v.activeId && !p.trashed) ? v.activeId : null;
    return { store: { projects, activeId: active ?? null }, purged: projects.length !== v.projects.length };
  } catch {
    return null;
  }
}
function fromLocalStorage() {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? parse(raw) : null;
  } catch {
    return null;
  }
}

/**
 * Reads the projects; called once at start-up, after the browser preview has installed its stand-in backend, so that the
 * preview goes through the same two commands as the app. Nothing that shows projects renders before it is done.
 */
let loading: Promise<void> | null = null;
export function loadProjects(): Promise<void> {
  return (loading ??= load());
}
async function load() {
  let got: { store: Store; purged: boolean } | null = null;
  let carriedOver = false;
  let loadError: string | null = null;
  let readOnly = false;
  if (!backend) {
    got = fromLocalStorage();
  } else {
    try {
      const raw = await invoke<string | null>("projects_load");
      if (raw == null) {
        // first run of this version: whatever the previous one kept in the WebView's storage becomes the file
        got = fromLocalStorage();
        carriedOver = !!got;
      } else {
        got = parse(raw);
        if (!got) {
          // the file is there but is not a projects store: set it aside for repair by hand, start empty
          try { await invoke("projects_quarantine"); } catch (e) { loadError = String(e); readOnly = true; }
          if (!readOnly) loadError = "corrupt";
        }
      }
    } catch (e) {
      // the file could not be read at all (permissions, a disk error): nothing is written this session, or the next write would replace it
      loadError = String(e);
      readOnly = true;
    }
  }
  store = got?.store ?? { projects: [], activeId: null };
  status = { loaded: true, saveError: null, loadError, readOnly };
  notify();
  // what the launch-time clean-up removed, and anything carried over, is written back right away
  if (!readOnly && (carriedOver || got?.purged)) void persist();
}

/* Writes go one at a time: a change made while one is in flight is written once it is done, as the latest state. */
let running: Promise<void> | null = null;
let again = false;
function persist(): Promise<void> {
  if (status.readOnly) {
    // the file could not be read at start-up; refusing to write keeps it from being replaced, and the person is told
    if (status.saveError !== status.loadError) { status = { ...status, saveError: status.loadError }; notify(); }
    return Promise.resolve();
  }
  if (!backend) {
    try {
      localStorage.setItem(KEY, JSON.stringify(store));
      if (status.saveError) { status = { ...status, saveError: null }; notify(); }
    } catch (e) {
      status = { ...status, saveError: String(e) }; notify();
    }
    return Promise.resolve();
  }
  if (running) { again = true; return running; }
  running = (async () => {
    try {
      do {
        again = false;
        await invoke("projects_save", { json: JSON.stringify(store) });
      } while (again);
      if (status.saveError) { status = { ...status, saveError: null }; notify(); }
    } catch (e) {
      // the state on screen is still right; the person is told, and the next change tries again
      status = { ...status, saveError: String(e) }; notify();
    } finally {
      running = null;
    }
  })();
  return running;
}

/**
 * 写盘有 300 ms 的尾随防抖（审查第二批）：连着标记几十张只写最后一次，不再每点一下整份 projects.json 全量写一遍。
 * 界面上的状态马上变（notify 不等写盘）；退出、关窗前由 `flushProjects` 冲刷掉还没写的那次。
 */
const SAVE_DELAY = 300;
let timer: number | null = null;
function schedule() {
  if (timer !== null) window.clearTimeout(timer);
  timer = window.setTimeout(() => { timer = null; void persist(); }, SAVE_DELAY);
}
/** 把还在防抖里的修改马上写下去，并等正在进行的那次写完。 */
export function flushProjects(): Promise<void> {
  if (timer !== null) { window.clearTimeout(timer); timer = null; return persist(); }
  return running ?? Promise.resolve();
}
function commit(next: Store) {
  store = next;
  notify();
  schedule();
}

export const useProjects = () => useSyncExternalStore(subscribe, () => store);
export const useProjectsStatus = () => useSyncExternalStore(subscribe, () => status);
export const getStore = () => store;

export function createProject(name: string, folders: string[]): Project {
  const p: Project = { id: `p${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`, name, folders, overrides: {}, keepRatio: 0.3, count: 0, updated: Date.now() };
  commit({ projects: [...store.projects, p], activeId: p.id });
  return p;
}
export function updateProject(id: string, patch: Partial<Omit<Project, "id">>) {
  commit({ ...store, projects: store.projects.map((p) => (p.id === id ? { ...p, ...patch, updated: Date.now() } : p)) });
}
/** Soft delete: the project moves to the recycle bin and can be restored. */
export function trashProject(id: string) {
  commit({ projects: store.projects.map((p) => (p.id === id ? { ...p, trashed: Date.now() } : p)), activeId: store.activeId === id ? null : store.activeId });
}
export function restoreProject(id: string) {
  commit({ ...store, projects: store.projects.map((p) => { if (p.id !== id) return p; const { trashed: _t, ...rest } = p; return rest; }) });
}
/** Permanent removal of the project record (never the photos on disk). */
/**
 * 彻底删掉的项目：它们文件夹里照片的分析结果和缩略图也一起清掉（还在的项目——含回收站里的——用着的文件夹不动）。
 * 分析结果是按照片存的，不清的话再用同一个文件夹建项目会直接拿到旧结果（作者 2026-10-23）。
 */
function forgetRemoved(removed: Project[], remaining: Project[]) {
  const dirs = removed.flatMap((p) => p.folders);
  if (!dirs.length) return;
  invoke("forget_folders", { dirs, keep: remaining.flatMap((p) => p.folders) }).catch(() => { /* not in Tauri */ });
}
export function purgeProject(id: string) {
  const left = store.projects.filter((p) => p.id !== id);
  forgetRemoved(store.projects.filter((p) => p.id === id), left);
  commit({ projects: left, activeId: store.activeId === id ? null : store.activeId });
}
export function emptyTrash() {
  const left = store.projects.filter((p) => !p.trashed);
  forgetRemoved(store.projects.filter((p) => p.trashed), left);
  commit({ ...store, projects: left });
}
export function setActive(id: string | null) { commit({ ...store, activeId: id }); }

/** Background clean-up for a long-running session: drops recycle-bin items older than TRASH_DAYS. */
export function purgeExpired() {
  const cutoff = Date.now() - TRASH_DAYS * 86400_000;
  if (store.projects.some((p) => p.trashed && p.trashed <= cutoff)) {
    const left = store.projects.filter((p) => !p.trashed || p.trashed > cutoff);
    forgetRemoved(store.projects.filter((p) => p.trashed && p.trashed <= cutoff), left);
    commit({ ...store, projects: left });
  }
}
