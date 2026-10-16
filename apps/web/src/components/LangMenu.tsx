import { useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import { CaretDown, Check, Globe } from "@phosphor-icons/react";
import type { Lang } from "../content";

const LANGS = [
  { id: "en", label: "English", to: "/" },
  { id: "zh", label: "中文", to: "/zh" },
] as const;

export default function LangMenu({ lang, page }: { lang: Lang; page?: string }) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const now = LANGS.find((l) => l.id === lang)!;

  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => { if (!box.current?.contains(e.target as Node)) setOpen(false); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("pointerdown", away); document.removeEventListener("keydown", esc); };
  }, [open]);

  return (
    <div ref={box} className="relative ml-2">
      <button type="button" onClick={() => setOpen((v) => !v)} aria-haspopup="listbox" aria-expanded={open}
        className="flex items-center gap-1.5 rounded-full border border-line py-1.5 pl-3 pr-2.5 transition-colors hover:bg-surface active:translate-y-px">
        <Globe size={16} className="text-ash" />
        <span>{now.label}</span>
        <CaretDown size={12} className={`text-ash transition-transform duration-300 ${open ? "rotate-180" : ""}`} />
      </button>
      <AnimatePresence>
        {open && (
          <motion.ul role="listbox" initial={{ opacity: 0, y: -6, scale: 0.97 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: -6, scale: 0.97 }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }} style={{ transformOrigin: "top right" }}
            className="absolute right-0 top-full mt-2 min-w-40 overflow-hidden glass rounded-2xl border border-line p-1 shadow-[0_24px_48px_-24px_color-mix(in_srgb,var(--ink)_40%,transparent)]">
            {LANGS.map((l) => (
              <li key={l.id} role="option" aria-selected={l.id === lang}>
                <Link to={(page ? `${l.id === "zh" ? "/zh" : ""}/${page}` : l.to) as never} onClick={() => { setOpen(false); try { localStorage.setItem("lang", l.id); } catch { /* the choice just is not remembered */ } }} className="flex items-center justify-between gap-6 rounded-xl px-3 py-2 transition-colors hover:bg-surface">
                  <span>{l.label}</span>
                  {l.id === lang && <Check size={14} weight="bold" className="text-gleam" />}
                </Link>
              </li>
            ))}
          </motion.ul>
        )}
      </AnimatePresence>
    </div>
  );
}
