import { Link } from "@tanstack/react-router";
import { copy, type Lang } from "../content";
import AppMock from "./AppMock";

export default function Landing({ lang }: { lang: Lang }) {
  const c = copy[lang];
  return (
    <div className="page" lang={lang === "zh" ? "zh-CN" : "en"}>
      <header className="top">
        <a className="brand" href="#top" aria-label={c.name}>
          <img src="/logo.png" alt="" width="28" height="28" />
          <b>{c.name}</b>
          {c.latin && <span className="mono">{c.latin}</span>}
        </a>
        <nav>
          <a href="#features">{c.nav.features}</a>
          <a href="#privacy">{c.nav.privacy}</a>
          <a href="#flow">{c.nav.flow}</a>
          <Link to={c.nav.other.to} className="lang">{c.nav.other.label}</Link>
        </nav>
      </header>

      <main id="top">
        <section className="hero">
          <div className="hero-text">
            <p className="kicker mono">{c.hero.kicker}</p>
            <h1>{c.hero.title[0]}<br />{c.hero.title[1]}</h1>
            <p className="lead">{c.hero.body}</p>
            <div className="cta-row">
              <span className="btn primary">{c.hero.cta}</span>
              <a className="btn ghost" href="#flow">{c.hero.secondary}</a>
            </div>
            <p className="note mono">{c.hero.note}</p>
          </div>
          <AppMock labels={c.mock} />
        </section>

        <ul className="proof">{c.proof.map((p) => <li key={p}>{p}</li>)}</ul>

        <section id="privacy" className="split">
          <div className="split-head">
            <p className="kicker mono">{c.privacy.kicker}</p>
            <h2>{c.privacy.title}</h2>
            <p className="lead">{c.privacy.body}</p>
          </div>
          <dl className="points">
            {c.privacy.points.map(([t, d]) => (<div key={t}><dt>{t}</dt><dd>{d}</dd></div>))}
          </dl>
        </section>

        <section id="features" className="features">
          <p className="kicker mono">{c.features.kicker}</p>
          <h2>{c.features.title}</h2>
          <ol>
            {c.features.items.map((f, i) => (
              <li key={f.t}>
                <span className="num mono">{String(i + 1).padStart(2, "0")}</span>
                <h3>{f.t}</h3>
                <p>{f.d}</p>
              </li>
            ))}
          </ol>
        </section>

        <section id="flow" className="split flow">
          <div className="split-head">
            <p className="kicker mono">{c.flow.kicker}</p>
            <h2>{c.flow.title}</h2>
          </div>
          <ol className="steps">
            {c.flow.steps.map(([t, d], i) => (
              <li key={t}><span className="num mono">{i + 1}</span><div><h3>{t}</h3><p>{d}</p></div></li>
            ))}
          </ol>
        </section>

        <section className="honest">
          <h2>{c.honest.title}</h2>
          <p>{c.honest.body}</p>
        </section>

        <section id="download" className="final">
          <h2>{c.cta.title}</h2>
          <p>{c.cta.body}</p>
          <span className="btn primary">{c.cta.button}</span>
        </section>
      </main>

      <footer className="foot mono">{c.footer}</footer>
    </div>
  );
}
