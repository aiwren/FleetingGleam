import { RefObject, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { SPRING } from "../motion";

/**
 * A search field that stays a small button until it is needed, then opens into an input (⌘F). While the search index is still being
 * built a thin ring fills around the icon, so nobody wonders why a search finds only part of a project.
 */
export default function SearchBox({ value, onChange, inputRef, placeholder, progress, count, wide = false }: {
  value: string; onChange: (v: string) => void; inputRef: RefObject<HTMLInputElement | null>;
  placeholder: string; progress: number | null; count?: number | null; wide?: boolean;
}) {
  const { t } = useTranslation();
  const open = value.length > 0;
  const [focused, setFocused] = useState(false);
  const expanded = open || focused || wide;
  const r = 7, c = 2 * Math.PI * r;
  return (
    <motion.label className={`sbox ${open ? "open" : ""}`} animate={{ width: expanded ? (wide ? 320 : 250) : 34 }} transition={SPRING.glide}
      data-tip={progress !== null ? t("search.indexing", { pct: Math.round(progress * 100) }) : undefined}>
      <span className="sbox-ico" aria-hidden>
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"><circle cx="7" cy="7" r="4.6" /><path d="M10.6 10.6L14 14" /></svg>
        {progress !== null && (
          <svg className="sbox-ring" width="26" height="26" viewBox="0 0 16 16" aria-hidden>
            <circle cx="8" cy="8" r={r} fill="none" strokeWidth="1.2" strokeDasharray={c} strokeDashoffset={c * (1 - Math.max(0.04, progress))} transform="rotate(-90 8 8)" />
          </svg>
        )}
      </span>
      <input ref={inputRef} value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} aria-label={t("search.label")} spellCheck={false} onFocus={() => setFocused(true)} onBlur={() => setFocused(false)}
        onKeyDown={(e) => { if (e.key === "Escape") { onChange(""); (e.target as HTMLInputElement).blur(); } e.stopPropagation(); }} />
      <AnimatePresence>
        {open && (
          <motion.span className="sbox-end" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
            {count !== null && count !== undefined && <em className="mono">{count}</em>}
            <button onClick={() => { onChange(""); inputRef.current?.focus(); }} aria-label={t("search.clear")} tabIndex={-1}>
              <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round"><path d="M3 3l6 6M9 3L3 9" /></svg>
            </button>
          </motion.span>
        )}
      </AnimatePresence>
    </motion.label>
  );
}
