import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import zh from "./zh-CN";
import en from "./en";

export const LANGS = [
  { code: "zh-CN", label: "简体中文" },
  { code: "en", label: "English" },
] as const;
export type Pref = "system" | (typeof LANGS)[number]["code"];

const KEY = "fg.lang";

function fromSystem(): string {
  const l = (navigator.languages?.[0] ?? navigator.language ?? "en").toLowerCase();
  return l.startsWith("zh") ? "zh-CN" : "en";
}

export function storedPref(): Pref {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "zh-CN" || v === "en") return v;
  } catch { /* storage may be unavailable */ }
  return "system";
}

function apply(lng: string) {
  document.documentElement.lang = lng;
  invoke("set_locale", { locale: lng }).catch(() => { /* not running inside Tauri */ });
}

export function setPref(p: Pref) {
  try { p === "system" ? localStorage.removeItem(KEY) : localStorage.setItem(KEY, p); } catch { /* ignore */ }
  const lng = p === "system" ? fromSystem() : p;
  i18n.changeLanguage(lng);
  apply(lng);
}

const initial = storedPref() === "system" ? fromSystem() : storedPref();
i18n.use(initReactI18next).init({
  resources: { "zh-CN": { translation: zh }, en: { translation: en } },
  lng: initial,
  fallbackLng: "en",
  interpolation: { escapeValue: false },
  returnObjects: true,
});
apply(initial);

export default i18n;
