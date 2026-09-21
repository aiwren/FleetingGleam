import { ReactNode, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { EASE_OUT } from "../motion";

/**
 * A small "?" that opens a floating card anchored to it. Technical detail lives here, so the page itself stays quiet
 * and nothing shifts when it is opened. Closes on outside click, Escape, scroll or resize.
 */
export default function InfoPop({ title, children, width = 340 }: { title?: string; children: ReactNode; width?: number }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  const card = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0, arrow: 20, w: width, up: false });

  const place = () => {
    const el = btn.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const w = Math.min(width, window.innerWidth - 24);
    const left = Math.min(Math.max(12, r.left + r.width / 2 - 28), window.innerWidth - w - 12);
    setPos({ left, top: r.bottom + 10, arrow: Math.min(Math.max(16, r.left + r.width / 2 - left), w - 16), w, up: false });
  };
  useLayoutEffect(() => { if (open) place(); }, [open]);
  // If the card would run off the bottom, open it upwards instead.
  useEffect(() => {
    if (!open) return;
    const id = requestAnimationFrame(() => {
      const c = card.current, el = btn.current;
      if (!c || !el) return;
      const r = el.getBoundingClientRect();
      if (r.bottom + 10 + c.offsetHeight > window.innerHeight - 12 && r.top - 10 - c.offsetHeight > 12) {
        setPos((p) => ({ ...p, top: r.top - 10 - c.offsetHeight, up: true }));
      }
    });
    return () => cancelAnimationFrame(id);
  }, [open]);
  useEffect(() => {
    if (!open) return;
    const outside = (e: MouseEvent) => { if (!(e.target as HTMLElement).closest?.(".pop, .tipbtn")) setOpen(false); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    const close = () => setOpen(false);
    window.addEventListener("mousedown", outside);
    window.addEventListener("keydown", esc);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("mousedown", outside);
      window.removeEventListener("keydown", esc);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [open]);

  return (
    <>
      <button ref={btn} className={`tipbtn ${open ? "on" : ""}`} onClick={() => setOpen((v) => !v)} aria-haspopup="dialog" aria-expanded={open} aria-label={t("more.open")} title={t("more.open")}>?</button>
      {createPortal(
        <AnimatePresence>
          {open && (
            <motion.div ref={card} className={`pop ${pos.up ? "up" : ""}`} role="dialog" aria-label={title}
              style={{ left: pos.left, top: pos.top, width: pos.w, transformOrigin: `${pos.arrow}px ${pos.up ? "100%" : "0"}` }}
              initial={{ opacity: 0, y: pos.up ? 6 : -6, scale: 0.96 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, scale: 0.97 }}
              transition={{ duration: 0.2, ease: EASE_OUT }}>
              <i className="pop-arrow" style={{ left: pos.arrow }} />
              {title && <h4>{title}</h4>}
              <div className="pop-body">{children}</div>
            </motion.div>
          )}
        </AnimatePresence>,
        document.body,
      )}
    </>
  );
}
