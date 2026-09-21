import { AnimatePresence, motion } from "motion/react";
import { DUR, EASE_OUT } from "../motion";

/** A number whose digits roll upward when they change (odometer style). */
export default function Ticker({ value, className = "" }: { value: number; className?: string }) {
  const s = String(Math.max(0, Math.round(value)));
  return (
    <span className={`ticker mono ${className}`} aria-label={s}>
      {[...s].map((d, i) => (
        <span key={s.length - i} className="tk" aria-hidden>
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.span key={d} className="tk-d" initial={{ y: "80%", opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: "-80%", opacity: 0 }} transition={{ duration: DUR.base, ease: EASE_OUT }}>{d}</motion.span>
          </AnimatePresence>
        </span>
      ))}
    </span>
  );
}
