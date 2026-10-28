import { useEffect, useRef, useState, type ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import { useInView } from "motion/react";
import { release, type Lang } from "../content";
import { CONTACT_EMAIL, legal, type Doc } from "../legal";
import Header from "./Header";
import Footer from "./Footer";

/** A stable, readable anchor per heading: an English title slugs cleanly; a Chinese one falls back to its position,
 * since romanising it would not be a slug a reader recognises. Matches how bilingual legal pages usually do this. */
function anchors(doc: Doc, contactLabel: string): string[] {
  const slug = (s: string, i: number) => {
    const ascii = s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
    return ascii || `section-${i + 1}`;
  };
  return [...doc.sections.map((s, i) => slug(s.h, i)), slug(contactLabel, doc.sections.length)];
}

/** One entry in the table of contents: it reports when its own section is the one centred in the viewport, so the
 * list can highlight it without a scroll listener (an IntersectionObserver window, the same device Flow.tsx uses). */
function Tracked({ id, onEnter, children }: { id: string; onEnter: (id: string) => void; children: ReactNode }) {
  const ref = useRef<HTMLElement>(null);
  const hit = useInView(ref, { margin: "-45% 0px -50% 0px", amount: 0 });
  useEffect(() => { if (hit) onEnter(id); }, [hit, id, onEnter]);
  return <section ref={ref} id={id} className="scroll-mt-24 py-10 first:pt-4">{children}</section>;
}

export default function Legal({ lang, kind }: { lang: Lang; kind: "privacy" | "terms" }) {
  const set = legal[lang], doc = set[kind], l = set.labels;
  const ids = anchors(doc, l.contact);
  const titles = [...doc.sections.map((s) => s.h), l.contact];
  const [active, setActive] = useState(ids[0]);

  return (
    <div className="min-h-dvh overflow-x-clip">
      <Header lang={lang} page={kind} />
      <main className="mx-auto max-w-[1180px] px-5 pb-8 pt-16 md:pt-24">
        <Link to={(lang === "zh" ? "/zh" : "/") as never} className="inline-block py-2 text-sm text-ash underline-offset-4 hover:text-ink hover:underline">{l.back}</Link>

        <div className="mt-8 lg:grid lg:grid-cols-[220px_minmax(0,1fr)] lg:items-start lg:gap-12">
          {/* Desktop: a plain list, no card of its own (see nested-cards) — a left bar marks the section in view. */}
          <nav aria-label="Table of contents" className="sticky top-24 hidden max-h-[calc(100dvh-7rem)] overflow-y-auto pb-8 lg:block">
            <ol className="border-l border-line">
              {titles.map((t, i) => (
                <li key={ids[i]}>
                  <a href={`#${ids[i]}`}
                    className={`-ml-px block border-l py-1.5 pl-4 text-sm transition-colors ${active === ids[i] ? "border-gleam text-ink" : "border-transparent text-ash hover:text-ink"}`}>
                    {t}
                  </a>
                </li>
              ))}
            </ol>
          </nav>

          {/* Phone and tablet: the same jump list, as a horizontal scroll-snap row above the document. */}
          <nav aria-label="Table of contents" className="-mx-5 mb-8 overflow-x-auto px-5 lg:hidden" style={{ scrollSnapType: "x proximity" }}>
            <ol className="flex w-max gap-2">
              {titles.map((t, i) => (
                <li key={ids[i]} className="snap-start">
                  <a href={`#${ids[i]}`} className="block whitespace-nowrap rounded-full border border-line px-3.5 py-1.5 text-sm text-ash">{t}</a>
                </li>
              ))}
            </ol>
          </nav>

          <div className="glass min-w-0 rounded-3xl border border-line px-6 py-10 md:px-12 md:py-14">
            <h1 className="text-[clamp(2rem,5vw,3.25rem)] font-semibold leading-[1.08] tracking-[-0.035em] text-balance">{doc.title}</h1>
            <p className="mt-4 text-sm text-ash">{l.updated}: {doc.updated}</p>
            <p className="mt-8 max-w-[65ch] text-lg text-ash">{doc.intro}</p>
            <div className="mt-6 divide-y divide-line">
              {doc.sections.map((s, i) => (
                <Tracked key={ids[i]} id={ids[i]} onEnter={setActive}>
                  <h2 className="text-xl font-semibold tracking-tight">{s.h}</h2>
                  {s.p?.map((t) => <p key={t} className="mt-3 max-w-[65ch] text-ash">{t}</p>)}
                  {s.ul && <ul className="mt-3 max-w-[65ch] list-disc space-y-2 pl-5 text-ash marker:text-gleam">{s.ul.map((t) => <li key={t}>{t}</li>)}</ul>}
                </Tracked>
              ))}
              <Tracked id={ids[ids.length - 1]} onEnter={setActive}>
                <h2 className="text-xl font-semibold tracking-tight">{l.contact}</h2>
                {CONTACT_EMAIL
                  ? <p className="mt-3 text-ash"><a href={`mailto:${CONTACT_EMAIL}`} className="text-ink underline underline-offset-4">{CONTACT_EMAIL}</a></p>
                  : release.available
                    ? <p className="mt-3 max-w-[65ch] text-ash">{l.contactMissing} <a href={`${release.repo}/issues`} className="text-ink underline underline-offset-4">{l.issues}</a></p>
                    : <p className="mt-3 max-w-[65ch] text-ash">{l.contactPending}</p>}
              </Tracked>
            </div>
          </div>
        </div>
      </main>
      <Footer lang={lang} />
    </div>
  );
}
