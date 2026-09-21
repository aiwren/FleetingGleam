import { Link } from "@tanstack/react-router";
import { copy, type Lang } from "../content";

export default function Header({ lang }: { lang: Lang }) {
  const c = copy[lang];
  return (
    <header className="sticky top-0 z-40 border-b border-line/60 bg-canvas/80 backdrop-blur-xl">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-5">
        <Link to={c.home} className="flex items-center gap-2.5">
          <img src="/logo.png" alt="" width={28} height={28} className="rounded-[7px]" />
          <b className="font-semibold tracking-tight">{c.name}</b>
          {c.latin && <span className="hidden font-mono text-xs text-ash sm:inline">{c.latin}</span>}
        </Link>
        <nav className="flex items-center gap-1 text-sm">
          {c.nav.map(([label, href]) => (
            <a key={href} href={href} className="hidden rounded-full px-3.5 py-1.5 text-ash transition-colors hover:bg-surface hover:text-ink md:inline-block">{label}</a>
          ))}
          <Link to={c.other.to} className="ml-2 rounded-full border border-line px-3.5 py-1.5 transition-colors hover:bg-surface">{c.other.label}</Link>
        </nav>
      </div>
    </header>
  );
}
