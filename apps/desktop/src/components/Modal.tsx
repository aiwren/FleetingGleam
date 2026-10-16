import { ReactNode, useEffect } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { EASE_OUT, SPRING } from "../motion";

/** A centred dialog over a dimmed, blurred backdrop. `locked` stops it closing while work is running. */
export default function Modal({ open, onClose, title, children, footer, locked = false, wide = false }: {
  open: boolean; onClose: () => void; title: string; children: ReactNode; footer: ReactNode; locked?: boolean; wide?: boolean;
}) {
  useEffect(() => {
    if (!open) return;
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape" && !locked) onClose(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [open, locked, onClose]);
  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div className="modal-back" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.22, ease: EASE_OUT }} onMouseDown={() => !locked && onClose()}>
          <motion.div className={`modal ${wide ? "wide" : ""}`} role="dialog" aria-modal aria-label={title} initial={{ opacity: 0, y: 16, scale: 0.97 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 8, scale: 0.98 }}
            transition={SPRING.soft} onMouseDown={(e) => e.stopPropagation()}>
            <h3>{title}</h3>
            <div className="modal-body">{children}</div>
            <div className="modal-foot">{footer}</div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
