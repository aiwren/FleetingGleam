import { ReactNode, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { EASE_OUT } from "../motion";

type Opt<T> = { id: T; label: string; icon?: ReactNode };

/**
 * One toolbar button for "how do I look at these photos": the browsing style and the sort order live in a small popover,
 * so the toolbar carries one control instead of two and stays usable at any window width.
 */
export default function ViewMenu<L extends string, S extends string>({ layout, layouts, onLayout, sort, sorts, onSort }: {
  layout: L; layouts: Opt<L>[]; onLayout: (v: L) => void;
  sort: S; sorts: Opt<S>[]; onSort: (v: S) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => { if (!root.current?.contains(e.target as Node)) setOpen(false); };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    window.addEventListener("pointerdown", away);
    window.addEventListener("keydown", key);
    return () => { window.removeEventListener("pointerdown", away); window.removeEventListener("keydown", key); };
  }, [open]);
  const cur = layouts.find((l) => l.id === layout);
  const svg = (icon: ReactNode) => <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" aria-hidden>{icon}</svg>;

  return (
    <div className="viewmenu" ref={root}>
      <button className={`btn vmbtn ${open ? "on" : ""}`} onClick={() => setOpen((v) => !v)} aria-expanded={open} aria-haspopup="menu" title={t("cull.viewMenu")}>
        {svg(cur?.icon)}
        <span className="vmlabel">{cur?.label}</span>
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d="M2.5 3.8L5 6.3l2.5-2.5" /></svg>
      </button>
      <AnimatePresence>
        {open && (
          <motion.div className="vmpanel" role="menu" initial={{ opacity: 0, y: -6, scale: 0.97 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: -4, scale: 0.98 }} transition={{ duration: 0.16, ease: EASE_OUT }}>
            <h4>{t("cull.layoutAria")}</h4>
            <div className="vmgrid">
              {layouts.map((l) => (
                <button key={l.id} role="menuitemradio" aria-checked={layout === l.id} className={layout === l.id ? "on" : ""} onClick={() => { onLayout(l.id); setOpen(false); }}>
                  {svg(l.icon)}<span>{l.label}</span>
                </button>
              ))}
            </div>
            {sorts.length > 0 && <h4>{t("cull.sortAria")}</h4>}
            {sorts.length > 0 && <div className="vmlist">
              {sorts.map((s) => (
                <button key={s.id} role="menuitemradio" aria-checked={sort === s.id} className={sort === s.id ? "on" : ""} onClick={() => { onSort(s.id); setOpen(false); }}>
                  <i aria-hidden />{s.label}
                </button>
              ))}
            </div>}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
