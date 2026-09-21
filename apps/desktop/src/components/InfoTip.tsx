import { ReactNode, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { EASE_OUT } from "../motion";

/** Progressive disclosure: a small "?" that reveals technical detail only for people who ask for it. */
export function useTip() {
  const [open, setOpen] = useState(false);
  return { open, toggle: () => setOpen((v) => !v) };
}

export function TipButton({ open, onClick }: { open: boolean; onClick: () => void }) {
  const { t } = useTranslation();
  return (
    <button className={`tipbtn ${open ? "on" : ""}`} onClick={onClick} aria-expanded={open} aria-label={open ? t("more.close") : t("more.open")} title={open ? t("more.close") : t("more.open")}>?</button>
  );
}

export function TipBody({ open, children }: { open: boolean; children: ReactNode }) {
  return (
    <AnimatePresence initial={false}>
      {open && (
        <motion.div className="tipbody" initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.28, ease: EASE_OUT }}>
          <div className="tipin">{children}</div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
