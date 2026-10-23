import { ReactNode, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { DUR, EASE_OUT } from "../motion";

/**
 * 一个"⋯"按钮带一个小菜单：次要的、危险的操作收在这里，行上只留一个值得整按钮的动作。
 * 首页项目卡、设置里的 NAS / 备份行、风格卡共用；点外面或 Esc 关。`plain` 是没有边框和底色的版本（压在照片上时用 `onPhoto`）。
 */
export default function RowMenu({ label, children, plain = false, onPhoto = false, className = "" }: { label: string; children: ReactNode; plain?: boolean; onPhoto?: boolean; className?: string }) {
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
  return (
    <div className={`style-menu rowmenu ${className}`} ref={root} onClick={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()}>
      <button className={`btn sm icon ${plain ? "plain" : ""} ${onPhoto ? "onphoto" : ""} ${open ? "on" : ""}`} aria-label={label} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden><circle cx="3.2" cy="8" r="1.3" /><circle cx="8" cy="8" r="1.3" /><circle cx="12.8" cy="8" r="1.3" /></svg>
      </button>
      <AnimatePresence>
        {open && (
          <motion.div className="vmpanel style-menu-panel" role="menu" initial={{ opacity: 0, y: -4, scale: 0.98 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: -2, scale: 0.98 }} transition={{ duration: DUR.fast, ease: EASE_OUT }}
            onClick={() => setOpen(false)}>
            <div className="vmlist">{children}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
