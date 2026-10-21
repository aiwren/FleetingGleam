import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { currentTier } from "./strategy";

/** One learned taste ("style"): its own labels, reference photos and model. Everything a project learns goes into the default one. */
export type PrefsInfo = {
  labels: number; keeps: number; rejects: number; refs: number; refs_from: string; active: boolean;
  /** examples the weights were trained on (0 while not in use), how much it may move a score now, and the marks on the way: see prefs.rs */
  trained: number; strength: number; max_strength: number; min_per_class: number; half_at: number; full_at: number;
};
export type StyleInfo = { id: string; name: string; is_default: boolean; info: PrefsInfo };

let styles: StyleInfo[] = [];
let revision = 0;
const subs = new Set<() => void>();
const notify = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };

export const useStyles = () => useSyncExternalStore(subscribe, () => styles);
/** Bumped whenever a learned taste changes, so open projects can offer to re-evaluate. */
export const usePrefsRevision = () => useSyncExternalStore(subscribe, () => revision);
export const bumpPrefsRevision = () => { revision++; notify(); };

const set = (list: unknown) => { if (Array.isArray(list)) { styles = list as StyleInfo[]; notify(); } };

export async function refreshStyles() {
  try { set(await invoke<StyleInfo[]>("styles_list")); } catch { /* not running inside the app */ }
}
/** Creates a style and returns its id (the one that was not in the list before), or null if it could not be made. */
export const createStyle = async (name: string): Promise<string | null> => {
  try {
    const before = new Set(styles.map((s) => s.id));
    set(await invoke("style_create", { name }));
    return styles.find((s) => !before.has(s.id))?.id ?? null;
  } catch { return null; }
};
export const renameStyle = async (id: string, name: string) => { try { set(await invoke("style_rename", { id, name })); } catch { /* ignore */ } };
export const deleteStyle = async (id: string) => { try { set(await invoke("style_delete", { id })); bumpPrefsRevision(); } catch { /* ignore */ } };
export const setDefaultStyle = async (id: string) => { try { set(await invoke("style_set_default", { id })); bumpPrefsRevision(); } catch { /* ignore */ } };

/** Learns a style from a folder of past picks. `onProgress` gets 0..1. */
export async function learnFromFolder(dir: string, style: string | undefined, onProgress: (p: number) => void): Promise<boolean> {
  const un = await listen<{ done: number; total: number }>("reference-progress", (e) => onProgress(e.payload.total ? e.payload.done / e.payload.total : 1)).catch(() => undefined);
  try {
    await invoke("learn_from_folder", { dir, style, compute: "auto", quality: currentTier() });
    await refreshStyles();
    bumpPrefsRevision();
    return true;
  } catch { return false; } finally { un?.(); }
}

export async function resetStyle(id: string) {
  try { await invoke("prefs_reset", { style: id }); await refreshStyles(); bumpPrefsRevision(); } catch { /* ignore */ }
}

void refreshStyles();
