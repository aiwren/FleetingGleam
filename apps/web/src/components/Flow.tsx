import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useInView, useReducedMotion } from "motion/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";

const SHOTS = ["wall", "compare", "wall"] as const;
const EASE = [0.16, 1, 0.3, 1] as const;

function Step({ n, title, body, active, onEnter, alt }: { n: number; title: string; body: string; active: boolean; onEnter: () => void; alt: string }) {
  const ref = useRef<HTMLLIElement>(null);
  // The step that crosses the middle of the viewport is the current one; the picture on the right follows it.
  const hit = useInView(ref, { margin: "-45% 0px -45% 0px", amount: 0 });
  useEffect(() => { if (hit) onEnter(); }, [hit]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <li ref={ref} className="flex items-center py-10 lg:min-h-[46vh]">
      <div className={`flex gap-5 transition-opacity duration-500 ${active ? "opacity-100" : "lg:opacity-35"}`}>
        <span className={`grid size-10 shrink-0 place-items-center rounded-full border text-sm tabular-nums transition-colors duration-500 ${active ? "border-transparent bg-gleam text-gleam-ink" : "border-line text-ash"}`}>{n}</span>
        <div>
          <h3 className="text-2xl font-semibold tracking-tight">{title}</h3>
          <p className="mt-2 max-w-md text-ash">{body}</p>
          <Frame shot={SHOTS[n - 1]} alt={alt} className="mt-6 lg:hidden" />
        </div>
      </div>
    </li>
  );
}

export default function Flow({ lang }: { lang: Lang }) {
  const c = copy[lang].flow;
  const reduce = useReducedMotion();
  const [now, setNow] = useState(0);
  return (
    <section id="flow" className="mx-auto max-w-6xl px-5 pt-40 md:pt-56">
      <motion.h2 initial={{ opacity: 0, y: 24 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true }} transition={{ duration: 0.8, ease: EASE }}
        className="max-w-2xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-[-0.035em] text-balance">{c.title}</motion.h2>
      <div className="mt-10 grid gap-x-16 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
        <ol>
          {c.steps.map(([t, d], i) => <Step key={t} n={i + 1} title={t} body={d} active={now === i} onEnter={() => setNow(i)} alt={t} />)}
        </ol>
        <div className="sticky top-0 hidden h-dvh items-center self-start lg:flex">
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.div key={now} className="w-full" initial={reduce ? false : { opacity: 0, scale: 0.97, y: 16 }} animate={{ opacity: 1, scale: 1, y: 0 }} exit={reduce ? undefined : { opacity: 0, scale: 1.02 }} transition={{ duration: 0.6, ease: EASE }}>
              <Frame shot={SHOTS[now]} alt={c.steps[now][0]} />
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </section>
  );
}
