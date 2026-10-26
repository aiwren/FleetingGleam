import type { TFunction } from "i18next";
import { fmtBytes } from "./format";

/**
 * 核心对用户可能碰到的错误回一个带前缀的字符串（三端共用的约定）：`space:<需要字节>:<可用字节>`、`verify:…`、`net:…`、`auth:…`、
 * `denied:…`、`cancelled`。这里按前缀换成一句中文说明，冒号后的细节留给"详情"。认不出前缀的原样给回。
 */
export type ErrKind = "space" | "verify" | "net" | "auth" | "denied" | "cancelled" | "other";

export function errKind(e: unknown): ErrKind {
  const s = String(e);
  const m = s.match(/^(space|verify|net|auth|denied):/);
  if (m) return m[1] as ErrKind;
  return s === "cancelled" || s.endsWith("cancelled") ? "cancelled" : "other";
}

/**
 * `space:` 的两个数字（需要、可用）。核心给错误加上下文时前缀仍在最前：`space:<上下文>: <需要>:<可用>`，所以取最后两个数。
 * 太大的数（比如"需要"是 u64::MAX）当作不知道。
 */
function spaceNumbers(s: string): { need: number; free: number; ctx: string } | null {
  const m = s.match(/^space:(.*?)(\d+):(\d+)\s*$/);
  if (!m) return null;
  const need = Number(m[2]), free = Number(m[3]);
  if (!Number.isSafeInteger(need) || !Number.isSafeInteger(free)) return null;
  return { need, free, ctx: m[1].replace(/:\s*$/, "").trim() };
}

/** 冒号后的细节（`space:` 的两个数字不算细节，只留上下文）。 */
export function errDetail(e: unknown): string {
  const s = String(e);
  const k = errKind(e);
  if (k === "other") return s;
  if (k === "cancelled") return "";
  if (k === "space") return spaceNumbers(s)?.ctx ?? "";
  return s.slice(k.length + 1);
}

/** 一句给人看的说明；`cancelled` 不提示，返回空串。 */
export function errText(t: TFunction, lang: string, e: unknown): string {
  const k = errKind(e);
  if (k === "cancelled") return "";
  if (k === "other") return String(e);
  if (k === "space") {
    const n = spaceNumbers(String(e));
    return n ? t("errors.space", { need: fmtBytes(lang, n.need), free: fmtBytes(lang, n.free) }) : t("errors.spaceUnknown");
  }
  return t(`errors.${k}`);
}
