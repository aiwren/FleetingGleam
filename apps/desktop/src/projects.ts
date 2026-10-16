import { useSyncExternalStore } from "react";
import { State } from "./types";

/** A project is a named set of folders plus the decisions made on its photos. Analysis results live in the backend cache. */
export type Project = {
  id: string;
  name: string;
  folders: string[];
  overrides: Record<string, State>;
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

const KEY = "fg.projects";
const subs = new Set<() => void>();
const notify = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };

let purgedOnLoad = false;
function load(): Store {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) {
      const v = JSON.parse(raw) as Store;
      if (Array.isArray(v.projects)) {
        // Anything that has sat in the recycle bin for TRASH_DAYS is removed for good, right as the app opens.
        const cutoff = Date.now() - TRASH_DAYS * 86400_000;
        const projects = v.projects.filter((p) => !p.trashed || p.trashed > cutoff);
        purgedOnLoad = projects.length !== v.projects.length;
        const active = projects.find((p) => p.id === v.activeId && !p.trashed) ? v.activeId : null;
        return { projects, activeId: active ?? null };
      }
    }
  } catch { /* storage may be unavailable */ }
  return { projects: [], activeId: null };
}
let store: Store = load();
// If the launch-time clean-up removed anything, write that back so the old records are really gone.
if (purgedOnLoad) { try { localStorage.setItem(KEY, JSON.stringify(store)); } catch { /* ignore */ } }
function commit(next: Store) {
  store = next;
  try { localStorage.setItem(KEY, JSON.stringify(store)); } catch { /* ignore */ }
  notify();
}

export const useProjects = () => useSyncExternalStore(subscribe, () => store);
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
export function purgeProject(id: string) {
  commit({ projects: store.projects.filter((p) => p.id !== id), activeId: store.activeId === id ? null : store.activeId });
}
export function emptyTrash() {
  commit({ ...store, projects: store.projects.filter((p) => !p.trashed) });
}
export function setActive(id: string | null) { commit({ ...store, activeId: id }); }

/** Background clean-up for a long-running session: drops recycle-bin items older than TRASH_DAYS. */
export function purgeExpired() {
  const cutoff = Date.now() - TRASH_DAYS * 86400_000;
  if (store.projects.some((p) => p.trashed && p.trashed <= cutoff)) {
    commit({ ...store, projects: store.projects.filter((p) => !p.trashed || p.trashed > cutoff) });
  }
}
