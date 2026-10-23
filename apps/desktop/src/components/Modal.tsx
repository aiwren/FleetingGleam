import { ReactNode, useEffect } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { DUR, EASE_OUT } from "../motion";

/* How many dialogs are open: the keyboard shortcuts of the pages underneath stay quiet while one is. */
let openDialogs = 0;
export const isModalOpen = () => openDialogs > 0;

/** A centred dialog over a dimmed, blurred backdrop. `locked` stops it closing while work is running. */
export default function Modal({ open, onClose, title, children, footer, locked = false, wide = false }: {
  open: boolean; onClose: () => void; title: string; children: ReactNode; footer: ReactNode; locked?: boolean; wide?: boolean;
}) {
  useEffect(() => {
    if (!open) return;
    openDialogs += 1;
    return () => { openDialogs -= 1; };
  }, [open]);
  useEffect(() => {
    if (!open) return;
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape" && !locked) onClose(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [open, locked, onClose]);
  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div className="modal-back" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: DUR.base, ease: EASE_OUT }} onMouseDown={() => !locked && onClose()}>
          {/* 对话框 320 ms expo-out，不用弹簧（M10.2） */}
          <motion.div className={`modal ${wide ? "wide" : ""}`} role="dialog" aria-modal aria-label={title} initial={{ opacity: 0, y: 12, scale: 0.98 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 6, scale: 0.98 }}
            transition={{ duration: DUR.base, ease: EASE_OUT }} onMouseDown={(e) => e.stopPropagation()}>
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
