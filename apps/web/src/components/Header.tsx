import { Link } from "@tanstack/react-router";
import { copy, type Lang } from "../content";
import LangMenu from "./LangMenu";
import ThemeToggle from "./ThemeToggle";

export default function Header({ lang, page }: { lang: Lang; page?: "privacy" | "terms" }) {
  const c = copy[lang];
  return (
    <header className="sticky top-0 z-40 border-b border-line/60 bg-canvas/80 backdrop-blur-xl">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-5">
        <Link to={c.home} className="flex items-center gap-2.5 py-2">
          <img src="/logo.png" alt="" width={28} height={28} className="rounded-[7px]" />
          <b className="font-semibold tracking-tight">{c.name}</b>
          {c.latin && <span className="hidden text-xs text-ash sm:inline">{c.latin}</span>}
        </Link>
        <nav className="flex items-center gap-1 text-sm">
          {c.nav.map(([label, href]) => (
            <a key={href} href={page ? `${c.home}${href}` : href} className="hidden rounded-full px-3.5 py-1.5 text-ash transition-colors hover:bg-surface hover:text-ink md:inline-block">{label}</a>
          ))}
          <LangMenu lang={lang} page={page} />
          <ThemeToggle label={c.theme} />
        </nav>
      </div>
    </header>
  );
}
