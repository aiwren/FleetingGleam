import { useSyncExternalStore } from "react";
import { getSettings, setSettings, subscribeSettings } from "./settings";
import type { State } from "./types";

/**
 * 可改的快捷键（A7.4）。固定键不在这里：数字 1–5 打星、0 清星 / 复位缩放、空格开大图、方向键移动、Esc 关闭、? 帮助、+/- 缩放，
 * 这些各模块自己判断，也不允许被绑成动作键。
 */
export type Action = State | "motion";
export type Preset = "default" | "left";
/** 存在 settings.keymap 里：一个预设，加上在它基础上逐个改掉的键（小写）。 */
export type KeymapSetting = { preset: Preset; custom: Partial<Record<Action, string>> };
/** 解析完的表：每个动作对应一个小写的 `KeyboardEvent.key`。 */
export type Keymap = Record<Action, string>;

/** 设置页按这个顺序列出；三个判定键是重点，播放实况只在查看器里用。 */
export const ACTIONS: Action[] = ["keep", "review", "reject", "motion"];
export const DECISIONS: State[] = ["keep", "review", "reject"];

/** `default`：P / R / X。`left`：右手留在方向键上，左手 Z 保留、C 待复查、X 淘汰，V 播放实况。 */
export const PRESETS: Record<Preset, Keymap> = {
  default: { keep: "p", review: "r", reject: "x", motion: "m" },
  left: { keep: "z", review: "c", reject: "x", motion: "v" },
};
export const DEFAULT_KEYMAP: KeymapSetting = { preset: "default", custom: {} };

/** 固定用途的键，不能绑给动作（小写的 `KeyboardEvent.key`）。 */
const FIXED = new Set(["0", "1", "2", "3", "4", "5", " ", "arrowup", "arrowdown", "arrowleft", "arrowright", "pageup", "pagedown", "home", "end", "escape", "?", "+", "-", "=", "tab", "enter", "backspace", "delete"]);
export const isFixedKey = (key: string) => FIXED.has(key.toLowerCase());
/** 能当动作键的：单个可见字符，而且不是固定键。 */
export const isBindable = (key: string) => key.length === 1 && key.trim() !== "" && !isFixedKey(key);

export function resolve(km: KeymapSetting = getSettings().keymap ?? DEFAULT_KEYMAP): Keymap {
  const base = PRESETS[km.preset] ?? PRESETS.default;
  const out = { ...base };
  for (const a of ACTIONS) { const k = km.custom?.[a]; if (k && isBindable(k)) out[a] = k.toLowerCase(); }
  return out;
}

/** 显示用：当前键的大写。 */
export const keyFor = (action: Action, km: Keymap = resolve()) => km[action].toUpperCase();
/** 文案插值用：`t("cull.hint", keyVars(km))` 里的 {{keep}} / {{review}} / {{reject}} / {{motion}}。 */
export const keyVars = (km: Keymap = resolve()): Record<Action, string> => ({ keep: keyFor("keep", km), review: keyFor("review", km), reject: keyFor("reject", km), motion: keyFor("motion", km) });

/** 这次按键对应哪个动作；带 ⌘ / Ctrl / Alt 的不算，大小写不分。总是读最新的设置，不用随设置变化重新挂监听。 */
export function actionOf(e: KeyboardEvent, km: Keymap = resolve()): Action | null {
  if (e.metaKey || e.ctrlKey || e.altKey) return null;
  const k = e.key.toLowerCase();
  for (const a of ACTIONS) if (km[a] === k) return a;
  return null;
}
/** 只看三个判定键。 */
export function decisionOf(e: KeyboardEvent, km?: Keymap): State | null {
  const a = actionOf(e, km);
  return a && a !== "motion" ? a : null;
}

/** 把 `key` 绑给 `action` 会出什么问题：固定键、与别的动作重复，或没有问题。 */
export function checkBinding(action: Action, key: string, km: Keymap = resolve()): { kind: "fixed" } | { kind: "conflict"; with: Action } | null {
  const k = key.toLowerCase();
  if (!isBindable(k)) return { kind: "fixed" };
  for (const a of ACTIONS) if (a !== action && km[a] === k) return { kind: "conflict", with: a };
  return null;
}
/** 已经解析出的表里仍然重复的动作（正常不会有，留给设置页提示）。 */
export function conflicts(km: Keymap): Set<Action> {
  const out = new Set<Action>();
  for (const a of ACTIONS) for (const b of ACTIONS) if (a !== b && km[a] === km[b]) out.add(a);
  return out;
}

const current = () => getSettings().keymap ?? DEFAULT_KEYMAP;
/** 改一个键；传 null 恢复成预设里的值。不校验，调用方先用 checkBinding。 */
export function setBinding(action: Action, key: string | null) {
  const km = current();
  const custom = { ...km.custom };
  const k = key?.toLowerCase();
  if (!k || PRESETS[km.preset][action] === k) delete custom[action]; else custom[action] = k;
  setSettings({ keymap: { preset: km.preset, custom } });
}
/** 换预设时清掉逐个改过的键：改键总是“在某个预设基础上”。 */
export const setPreset = (preset: Preset) => setSettings({ keymap: { preset, custom: {} } });
export const resetKeymap = () => setSettings({ keymap: DEFAULT_KEYMAP });

/* 设置页和各处提示靠它随设置重渲染；监听按键的地方直接用 actionOf（它读的是最新设置）。 */
let cached: { src: KeymapSetting; map: Keymap } | null = null;
function snapshot(): Keymap {
  const src = current();
  if (!cached || cached.src !== src) cached = { src, map: resolve(src) };
  return cached.map;
}
export const useKeymap = () => useSyncExternalStore(subscribeSettings, snapshot);
