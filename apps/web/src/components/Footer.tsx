import { Link } from "@tanstack/react-router";
import { copy, type Lang } from "../content";
import { legal } from "../legal";

export default function Footer({ lang }: { lang: Lang }) {
  const c = copy[lang], l = legal[lang].labels;
  const base = lang === "zh" ? "/zh" : "";
  return (
    <footer className="mx-auto mt-24 max-w-6xl border-t border-line px-5 py-10 text-sm text-ash">
      <nav className="mb-5 flex flex-wrap gap-x-6 gap-y-2" aria-label="Legal">
        <Link to={`${base}/privacy` as never} className="inline-block py-1.5 text-ink underline-offset-4 hover:underline">{l.privacy}</Link>
        <Link to={`${base}/terms` as never} className="inline-block py-1.5 text-ink underline-offset-4 hover:underline">{l.terms}</Link>
      </nav>
      <p>{c.footer.rights}</p>
      <p className="mt-1.5 text-xs">{c.footer.note}</p>
    </footer>
  );
}
