import { Link } from "@tanstack/react-router";
import { copy, release, type Lang } from "../content";
import { legal } from "../legal";

/** A quiet glass panel, the same surface the feature cards and the language menu sit on, so the page does not go flat here. */
export default function Footer({ lang }: { lang: Lang }) {
  const c = copy[lang], l = legal[lang].labels;
  const base = lang === "zh" ? "/zh" : "";
  return (
    <footer className="mx-auto mb-10 mt-16 max-w-6xl px-5">
      <div className="glass rounded-3xl border border-line px-6 py-8 text-sm text-ash md:px-8">
        <nav className="mb-5 flex flex-wrap gap-x-6 gap-y-2" aria-label="Legal">
          <Link to={`${base}/privacy` as never} className="inline-block py-1.5 text-ink underline-offset-4 hover:underline">{l.privacy}</Link>
          <Link to={`${base}/terms` as never} className="inline-block py-1.5 text-ink underline-offset-4 hover:underline">{l.terms}</Link>
          {release.available && <a href={release.repo} className="inline-block py-1.5 text-ink underline-offset-4 hover:underline">GitHub</a>}
        </nav>
        <p>{c.footer.rights}</p>
        <p className="mt-1.5 text-xs">{c.footer.note}</p>
      </div>
    </footer>
  );
}
