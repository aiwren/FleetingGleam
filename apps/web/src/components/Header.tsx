import { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import { GithubLogo, List, X } from "@phosphor-icons/react";
import { copy, release, type Lang } from "../content";
import LangMenu from "./LangMenu";
import ThemeToggle from "./ThemeToggle";

export default function Header({ lang, page }: { lang: Lang; page?: "privacy" | "terms" }) {
  const c = copy[lang];
  // Below md the links live in a panel instead of vanishing: the sections stay reachable on a phone.
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("keydown", esc);
    return () => document.removeEventListener("keydown", esc);
  }, [open]);

  const href = (h: string) => (page ? `${c.home}${h}` : h);
  // 页面顶上顶栏是透明的，往下一滚就铺上底色和一条细线（新设计稿的做法）
  const [solid, setSolid] = useState(false);
  useEffect(() => {
    const on = () => setSolid(scrollY > 8);
    on();
    addEventListener("scroll", on, { passive: true });
    return () => removeEventListener("scroll", on);
  }, []);

  return (
    <header className={`sticky top-0 z-40 border-b transition-colors duration-300 ${solid || open ? "border-line bg-canvas" : "border-transparent bg-transparent"}`}>
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-5">
        <Link to={c.home} className="flex items-center gap-2.5 py-2">
          <img src="/logo.png" alt="" width={28} height={28} className="rounded-[7px]" />
          <b className="font-semibold tracking-tight">{c.name}</b>
          {c.latin && <span className="hidden text-xs text-ash sm:inline">{c.latin}</span>}
        </Link>
        <nav className="flex items-center gap-1 text-sm">
          {c.nav.map(([label, h]) => (
            <a key={h} href={href(h)} className="hidden rounded-full px-3.5 py-1.5 text-ash transition-colors hover:bg-surface hover:text-ink md:inline-block">{label}</a>
          ))}
          {release.available && (
            <a href={release.repo} aria-label="GitHub" className="hidden size-9 place-items-center rounded-full text-ash transition-colors hover:bg-surface hover:text-ink md:grid"><GithubLogo size={18} /></a>
          )}
          <LangMenu lang={lang} page={page} />
          <ThemeToggle label={c.theme} />
          <a href={href("#download")} className="ml-2 hidden min-h-9 items-center whitespace-nowrap rounded-lg border border-line-strong bg-panel px-4 text-sm font-medium transition-colors hover:border-ash active:translate-y-px sm:inline-flex">{release.available ? c.cta.ready : c.cta.soon}</a>
          <button type="button" onClick={() => setOpen((v) => !v)} aria-expanded={open} aria-label={c.menu}
            className="ml-1 grid size-9 place-items-center rounded-full border border-line text-ash transition-colors hover:bg-surface hover:text-ink active:translate-y-px md:hidden">
            {open ? <X size={17} /> : <List size={17} />}
          </button>
        </nav>
      </div>
      <AnimatePresence>
        {open && (
          <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }} className="overflow-hidden border-t border-line/60 md:hidden">
            <ul className="mx-auto max-w-6xl px-5 py-2">
              {c.nav.map(([label, h]) => (
                <li key={h}><a href={href(h)} onClick={() => setOpen(false)} className="block border-b border-line/50 py-3.5 last:border-0">{label}</a></li>
              ))}
            </ul>
          </motion.div>
        )}
      </AnimatePresence>
    </header>
  );
}
