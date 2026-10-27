import { Link } from "@tanstack/react-router";
import { copy, release, type Lang } from "../content";
import { legal } from "../legal";

/** 页脚：一条细线下面三行小字，法律页面的入口放在最前。 */
export default function Footer({ lang }: { lang: Lang }) {
  const c = copy[lang], l = legal[lang].labels;
  const base = lang === "zh" ? "/zh" : "";
  return (
    <footer className="ld-footer">
      <div className="wrap">
        <nav aria-label="Legal">
          <Link to={`${base}/privacy` as never}>{l.privacy}</Link>
          <Link to={`${base}/terms` as never}>{l.terms}</Link>
          {release.available && <a href={release.repo}>GitHub</a>}
        </nav>
        <p>{c.footer.rights}</p>
        <p>{c.footer.note}</p>
      </div>
    </footer>
  );
}
