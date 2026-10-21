import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";

/** A network share the app connects to itself (see src-tauri/src/nas.rs): no password here, that lives in the Keychain. */
export type Share = { id: string; name: string; url: string; user: string; mountpoint: string; connected: boolean };

let shares: Share[] = [];
const subs = new Set<() => void>();
const notify = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
const set = (v: unknown) => { if (Array.isArray(v)) { shares = v as Share[]; notify(); } };

export const useShares = () => useSyncExternalStore(subscribe, () => shares);
export async function refreshShares() { try { set(await invoke("nas_list")); } catch { /* not running inside the app */ } }
/** Connects at once; nothing is remembered unless that works. Throws with the reason otherwise. */
export async function addShare(p: { name: string; url: string; user: string; password: string; remember: boolean }) { set(await invoke("nas_add", p)); }
export async function removeShare(id: string) { set(await invoke("nas_remove", { id })); }
export async function connectShare(id: string, password?: string) { set(await invoke("nas_connect", { id, password: password ?? null })); }
export async function disconnectShare(id: string) { set(await invoke("nas_disconnect", { id })); }
