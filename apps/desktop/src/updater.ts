/**
 * 应用内更新（和 Android 的 core/Updates.kt 同一套状态与文案）。版本只来自 GitHub Releases：原生层（src-tauri/src/update.rs）
 * 读发布时签好名的 latest.json，这里只管一个状态机给设置页"关于"显示：
 *   idle → checking → latest / failed / found → downloading → installing → ready（重启后生效）/ error
 * 启动后几秒静默查一次（设置里可关）：离线、GitHub 不通都安静地回到 idle，不弹任何东西；只有手动点"检查更新"才会说"没查到"。
 * 限流：GitHub 对匿名请求每小时只给 60 次，所以手动检查之后 60 秒内按钮置灰、不再联网；启动时的自动检查 10 分钟内查过就跳过。
 * 上次检查的时间存在 localStorage，重开应用也绕不过去（和 Android 的 Updates.kt 同样的两个数）。
 */
import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { flushProjects } from "./projects";
import { getSettings } from "./settings";

export type UpdateInfo = { version: string; current: string; notes: string | null; date: string | null };
export type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "latest" }
  | { status: "failed" }
  | { status: "found"; info: UpdateInfo }
  | { status: "downloading"; info: UpdateInfo; done: number; total: number | null }
  | { status: "installing"; info: UpdateInfo }
  | { status: "ready"; info: UpdateInfo }
  | { status: "error"; info: UpdateInfo; error: string };

/** 手动检查之间至少隔这么久。 */
export const MANUAL_COOLDOWN_MS = 60_000;
/** 启动时的自动检查：这么久之内查过就不查。 */
const AUTO_MIN_INTERVAL_MS = 10 * 60_000;
const CHECKED_KEY = "fg.updateCheckedAt";
let checkedAt: number = (() => { try { return Number(localStorage.getItem(CHECKED_KEY)) || 0; } catch { return 0; } })();
function markChecked() { checkedAt = Date.now(); try { localStorage.setItem(CHECKED_KEY, String(checkedAt)); } catch { /* storage may be unavailable */ } }
/** 手动检查还要等多少毫秒（0 = 现在就能查）。时钟被往回拨过就当可以查。 */
export function cooldownLeft(now = Date.now()): number {
  const left = checkedAt + MANUAL_COOLDOWN_MS - now;
  return left > 0 && left <= MANUAL_COOLDOWN_MS ? left : 0;
}

let state: UpdateState = { status: "idle" };
const subs = new Set<() => void>();
function set(next: UpdateState) { state = next; subs.forEach((f) => f()); }
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
export const useUpdate = () => useSyncExternalStore(subscribe, () => state);
export const getUpdate = () => state;

/** 问一次有没有新版本。`silent`（启动时的自动检查）查不到就当没事；手动点的才显示"没查到"。 */
export async function checkForUpdate(silent = false): Promise<void> {
  if (state.status === "checking" || state.status === "downloading" || state.status === "installing" || state.status === "ready") return;
  // 冷却中：不联网，界面上照旧显示上一次的结果（按钮本来也是灰的，这里再挡一次）
  if (silent ? Date.now() - checkedAt < AUTO_MIN_INTERVAL_MS && Date.now() >= checkedAt : cooldownLeft() > 0) return;
  // 已经发现过新版本：再点"检查"只是重新问一遍，仍旧显示那条
  set({ status: "checking" });
  markChecked();
  try {
    const info = await invoke<UpdateInfo | null>("update_check");
    set(info ? { status: "found", info } : { status: "latest" });
  } catch {
    set(silent ? { status: "idle" } : { status: "failed" });
  }
  markChecked(); // 冷却从这次检查结束时算起（慢网络下一次检查本身就可能好几秒）
  scheduleCooldownEnd();
}

/** 冷却结束时通知一次界面，让按钮重新可点。 */
let cooldownTimer: ReturnType<typeof setTimeout> | null = null;
function scheduleCooldownEnd() {
  if (cooldownTimer) clearTimeout(cooldownTimer);
  const left = cooldownLeft();
  if (left > 0) cooldownTimer = setTimeout(() => { cooldownTimer = null; subs.forEach((f) => f()); }, left + 50);
}
/** 设置页用：现在是否在冷却中（冷却结束时会自动重渲染）。 */
export const useCoolingDown = () => useSyncExternalStore(subscribe, () => cooldownLeft() > 0);
scheduleCooldownEnd();

let progressBound: Promise<() => void> | null = null;

/** 下载并安装刚才发现的版本；进度由原生层的 "update-progress" 事件送来。装完等用户点"重启"。 */
export async function installUpdate(): Promise<void> {
  if (state.status !== "found" && state.status !== "error") return;
  const info = state.info;
  set({ status: "downloading", info, done: 0, total: null });
  progressBound ??= listen<{ done: number; total: number | null }>("update-progress", (e) => {
    if (state.status === "downloading") set({ ...state, done: e.payload.done, total: e.payload.total });
  }).catch(() => () => { /* not in Tauri */ });
  const unDownloaded = await listen("update-downloaded", () => { if (state.status === "downloading") set({ status: "installing", info }); }).catch(() => () => { /* not in Tauri */ });
  try {
    await invoke("update_install");
    set({ status: "ready", info });
  } catch (e) {
    set({ status: "error", info, error: String(e) });
  } finally {
    unDownloaded();
  }
}

/** 重启到新版本：先把防抖里的项目写完（和退出前一样），再让原生层重启。 */
export async function restartToUpdate(): Promise<void> {
  await flushProjects().catch(() => { /* 写不完也得重启 */ });
  await invoke("update_restart").catch(() => { /* not in Tauri */ });
}

let scheduled = false;
/** 启动后 6 秒静默检查一次（设置"自动检查更新"关着就不查）。进程里只安排一次。 */
export function scheduleAutoCheck(): void {
  if (scheduled) return;
  scheduled = true;
  setTimeout(() => { if (getSettings().autoUpdate) void checkForUpdate(true); }, 6000);
}
