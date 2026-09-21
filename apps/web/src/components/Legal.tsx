import { Link } from "@tanstack/react-router";
import type { Lang } from "../content";
import { CONTACT_EMAIL, legal } from "../legal";
import Header from "./Header";
import Footer from "./Footer";

export default function Legal({ lang, kind }: { lang: Lang; kind: "privacy" | "terms" }) {
  const set = legal[lang], doc = set[kind], l = set.labels;
  return (
    <div className="min-h-dvh overflow-x-clip">
      <Header lang={lang} page={kind} />
      <main className="mx-auto max-w-3xl px-5 pb-8 pt-16 md:pt-24">
        <Link to={(lang === "zh" ? "/zh" : "/") as never} className="inline-block py-2 text-sm text-ash underline-offset-4 hover:text-ink hover:underline">{l.back}</Link>
        <h1 className="mt-8 text-[clamp(2rem,5vw,3.25rem)] font-semibold leading-[1.08] tracking-[-0.035em] text-balance">{doc.title}</h1>
        <p className="mt-4 text-sm text-ash">{l.updated}: {doc.updated}</p>
        <p className="mt-8 max-w-[65ch] text-lg text-ash">{doc.intro}</p>
        <div className="mt-6">
          {doc.sections.map((s, i) => (
            <section key={s.h} className="pt-12" aria-labelledby={`s${i}`}>
              <h2 id={`s${i}`} className="text-xl font-semibold tracking-tight">{s.h}</h2>
              {s.p?.map((t) => <p key={t} className="mt-3 max-w-[65ch] text-ash">{t}</p>)}
              {s.ul && <ul className="mt-3 max-w-[65ch] list-disc space-y-2 pl-5 text-ash marker:text-gleam">{s.ul.map((t) => <li key={t}>{t}</li>)}</ul>}
            </section>
          ))}
          <section className="pt-12" aria-labelledby="contact">
            <h2 id="contact" className="text-xl font-semibold tracking-tight">{l.contact}</h2>
            <p className="mt-3 text-ash">{CONTACT_EMAIL ? <a href={`mailto:${CONTACT_EMAIL}`} className="text-ink underline underline-offset-4">{CONTACT_EMAIL}</a> : l.contactMissing}</p>
          </section>
        </div>
      </main>
      <Footer lang={lang} />
    </div>
  );
}
