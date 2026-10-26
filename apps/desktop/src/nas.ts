import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";

/** 应用自己连接的一台 NAS（见 src-tauri/src/nas.rs）：服务器 + 账号，没有密码（在钥匙串里）。`local` 是它在本机的镜像文件夹。 */
/** `error`：没连上时最后一次的原因，带约定的前缀（`auth:` 密码不对、`net:` 连不上、`denied:` 没权限），连上时为 null。 */
export type Share = { id: string; name: string; host: string; user: string; enabled: boolean; local: string; connected: boolean; error?: string | null };
/** NAS 上一个位置的内容：根是共享列表，往下只有子文件夹和照片。 */
export type NasEntry = { name: string; dir: boolean; size: number };
export type Listing = { path: string; local: string; entries: NasEntry[] };
/** 从 NAS 拉取原图的进度（后端的 "nas-progress" 事件）。 */
export type NasProgress = { share: string; done: number; total: number; bytes_done: number; bytes_total: number };

let shares: Share[] = [];
const subs = new Set<() => void>();
const notify = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
const set = (v: unknown) => { if (Array.isArray(v)) { shares = v as Share[]; notify(); } };

export const useShares = () => useSyncExternalStore(subscribe, () => shares);
export const getShares = () => shares;
/** 启用的那些：选文件夹时只给这些。 */
export const getEnabledShares = () => shares.filter((s) => s.enabled);
export async function refreshShares() { try { set(await invoke("nas_list")); } catch { /* not running inside the app */ } }
/** 马上连接；连不上就什么都不记，抛出原因。 */
export async function addShare(p: { name: string; host: string; user: string; password: string; remember: boolean }) { set(await invoke("nas_add", p)); }
export async function removeShare(id: string) { set(await invoke("nas_remove", { id })); }
export async function connectShare(id: string, password?: string) { set(await invoke("nas_connect", { id, password: password ?? null })); }
export async function disconnectShare(id: string) { set(await invoke("nas_disconnect", { id })); }
/** 启用 / 停用：停用的不自动连接、不出现在选文件夹里。 */
export async function setShareEnabled(id: string, enabled: boolean) { set(await invoke("nas_set_enabled", { id, enabled })); }
/** 添加 NAS 时的"测试连接"：连上这台服务器，返回它的共享名；什么都不记。抛出原因。 */
export const probeShare = (p: { host: string; user: string; password: string; id?: string }) => invoke<string[]>("nas_probe", { ...p, id: p.id ?? null });
/** 编辑：改名、地址、账号或密码；密码留空就沿用记住的。先连一次，连不上什么都不改。 */
export async function updateShare(p: { id: string; name: string; host: string; user: string; password: string; remember: boolean }) { set(await invoke("nas_update", p)); }
/** 共享里一个文件夹（`/` 分隔，根是空串）的内容，以及它在镜像里的本地路径。 */
export const browseShare = (id: string, path: string) => invoke<Listing>("nas_browse", { id, path });

/*
 * 选文件夹：没有 NAS 时就是系统的文件夹对话框；有 NAS 时先问"本机还是哪个共享"，这个问题由 App 里挂着的 FolderPicker 来问。
 * 调用方只管 `pickFolder()`（见 projectActions.ts），拿到的是一个本地路径：本机文件夹，或共享镜像里的文件夹。
 */
/** `nasOnly`：只列 NAS（导出到 NAS 时）；`title` / `choose`：换掉"项目文件夹在哪里？"与"就选这个文件夹"。 */
export type PickOptions = { nasOnly?: boolean; title?: string; choose?: string; hint?: string };
export type PickRequest = { shareId?: string; opts?: PickOptions; resolve: (dir: string | null) => void };
let request: PickRequest | null = null;
const psubs = new Set<() => void>();
const psubscribe = (f: () => void) => { psubs.add(f); return () => { psubs.delete(f); }; };
export const usePickRequest = () => useSyncExternalStore(psubscribe, () => request);
export function openPicker(shareId?: string, opts?: PickOptions): Promise<string | null> {
  request?.resolve(null);
  return new Promise((resolve) => { request = { shareId, opts, resolve }; psubs.forEach((f) => f()); });
}

/** 镜像路径（…/nas/<id>/<共享>/<路径>）在界面上的说法："NAS 名 · 共享/路径"；不是镜像路径就原样给回。 */
export function nasLabel(path: string): string {
  const m = path.match(/\/nas\/([^/]+)\/(.+)$/);
  if (!m) return path;
  const s = shares.find((x) => x.id === m[1]);
  return s ? `${s.name} · ${m[2]}` : path;
}
/** FolderPicker 回答之后调用：`dir` 为 null 表示取消。 */
export function answerPicker(dir: string | null) {
  const r = request;
  request = null;
  psubs.forEach((f) => f());
  r?.resolve(dir);
}
