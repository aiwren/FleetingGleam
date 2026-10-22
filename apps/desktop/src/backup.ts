import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getSettings, subscribeSettings } from "./settings";
import { subscribeStyles } from "./styles";

/** 一个备份目标（现在只有 Cloudflare R2 一种）；从不带 Secret。 */
export type BackupTarget = { id: string; name: string; kind: "r2"; account: string; bucket: string; access_key: string; has_secret: boolean; last_ok: number; last_error: string | null };
/** 云备份的状态，后端 backup_status 给的。 */
export type BackupStatus = { configured: boolean; enabled: boolean; keep: number; targets: BackupTarget[] };

let status: BackupStatus | null = null;
const subs = new Set<() => void>();
const notify = () => subs.forEach((f) => f());
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
const set = (v: BackupStatus) => { status = v; notify(); };
export const useBackup = () => useSyncExternalStore(subscribe, () => status);

export async function refreshBackup() { try { set(await invoke<BackupStatus>("backup_status")); } catch { /* not running inside the app */ } }
/** 添加（没有 id）或修改一个目标；先连一次桶，连不上抛出原因、什么都不改。修改时 secret 留空就沿用。 */
export async function saveTarget(p: { id?: string; name: string; kind: "r2"; account: string; bucket: string; access_key: string; secret: string }) {
  // Tauri 把 Rust 侧的 access_key 映射成 accessKey
  set(await invoke<BackupStatus>("backup_save_target", { id: p.id ?? null, name: p.name, kind: p.kind, account: p.account, bucket: p.bucket, accessKey: p.access_key, secret: p.secret || null }));
}
export async function removeTarget(id: string) { set(await invoke<BackupStatus>("backup_remove_target", { id })); }
/** 自动备份的开关和保留份数。 */
export async function setBackupOptions(p: { enabled?: boolean; keep?: number }) { set(await invoke<BackupStatus>("backup_set_options", { enabled: p.enabled ?? null, keep: p.keep ?? null })); }
/** 备份一次到每一个目标；"立即备份"和自动触发走的是同一套。 */
export async function backupNow() { set(await invoke<BackupStatus>("backup_now", { settings: getSettings() })); }
export type TasteSummary = { styles: number; examples: number; refs: number; skipped: number };
/** 桶里最新那份备份的说明：哪台设备、什么时候做的（老版本做的备份没有，给 null）。 */
export type BackupMeta = { device: string; platform: string; exported_at: number; app_version: string; styles: number; examples: number };
export const peekBackup = (id: string) => invoke<BackupMeta | null>("backup_peek", { id });
/** 从某个目标的最新备份恢复。 */
export const restoreBackup = (id: string) => invoke<[TasteSummary, Record<string, unknown> | null]>("backup_restore", { id });

/*
 * 自动备份：设置或任何风格的喜好变了之后，静默一会儿再上传一次（连续改动只传最后一版）；失败了不打扰，状态里能看到。
 * 开关在后端的配置里（enabled），这里只在它开着时动手。
 */
const QUIET_MS = 20_000;
let timer: ReturnType<typeof setTimeout> | undefined;
let busy = false;
let again = false;
function schedule() {
  if (!status?.configured || !status.enabled) return;
  clearTimeout(timer);
  timer = setTimeout(run, QUIET_MS);
}
async function run() {
  if (busy) { again = true; return; }
  busy = true;
  try { await backupNow(); } catch { await refreshBackup(); }
  busy = false;
  if (again) { again = false; schedule(); }
}
let watching = false;
/** 应用启动时调一次：读状态，并开始盯着设置和喜好的变化。 */
export function watchForBackup() {
  if (watching) return;
  watching = true;
  void refreshBackup();
  subscribeSettings(schedule);
  subscribeStyles(schedule);
}
