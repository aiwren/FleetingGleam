import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useInView, useReducedMotion, useScroll, useTransform } from "motion/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";

const SHOTS = ["wall", "compare", "wall"] as const;
const EASE = [0.16, 1, 0.3, 1] as const;

function Step({ n, title, body, active, onEnter, alt }: { n: number; title: string; body: string; active: boolean; onEnter: () => void; alt: string }) {
  const ref = useRef<HTMLLIElement>(null);
  // The step crossing the middle of the viewport is the current one, and the picture follows it.
  const hit = useInView(ref, { margin: "-48% 0px -48% 0px", amount: 0 });
  useEffect(() => { if (hit) onEnter(); }, [hit]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <li ref={ref} className="relative flex min-h-[26vh] items-center py-8 lg:min-h-[30vh]">
      {/* The number sits on the rail, so the distance between steps reads as the path taken rather than as empty space. */}
      <span className={`absolute -left-[41px] grid size-8 place-items-center rounded-full border text-[13px] tabular-nums transition-colors duration-500 ${active ? "border-transparent bg-gleam text-gleam-ink" : "border-line bg-canvas text-ash"}`}>{n}</span>
      <div className={`transition-opacity duration-500 ${active ? "opacity-100" : "lg:opacity-40"}`}>
        <h3 className="text-2xl font-semibold tracking-tight">{title}</h3>
        <p className="mt-2 max-w-md text-ash">{body}</p>
        <Frame shot={SHOTS[n - 1]} alt={alt} className="mt-6 lg:hidden" sizes="100vw" />
      </div>
    </li>
  );
}

/** The three steps on a rail that fills as the reader goes down it, with one picture on the right that keeps up. */
export default function Flow({ lang }: { lang: Lang }) {
  const c = copy[lang].flow;
  const reduce = useReducedMotion();
  const [now, setNow] = useState(0);
  const rail = useRef<HTMLOListElement>(null);
  const { scrollYProgress } = useScroll({ target: rail, offset: ["start 62%", "end 62%"] });
  const fill = useTransform(scrollYProgress, [0, 1], [0, 1]);

  return (
    <section id="flow" className="mx-auto max-w-6xl px-5 pt-40 md:pt-56">
      <motion.h2 initial={reduce ? false : { opacity: 0, y: 24 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true }} transition={{ duration: 0.8, ease: EASE }}
        className="max-w-2xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-[-0.035em] text-balance">{c.title}</motion.h2>

      <div className="mt-12 grid gap-x-16 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
        <ol ref={rail} className="relative ml-4 border-l border-line pl-10">
          <motion.i aria-hidden className="absolute -left-px top-0 h-full w-px origin-top bg-gleam"
            style={reduce ? { scaleY: 1 } : { scaleY: fill }} />
          {c.steps.map(([t, d], i) => (
            <Step key={t} n={i + 1} title={t} body={d} active={now === i} onEnter={() => setNow(i)} alt={t} />
          ))}
        </ol>

        <div className="sticky top-16 hidden h-[calc(100dvh-4rem)] items-center self-start lg:flex">
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.div key={now} className="w-full"
              initial={reduce ? false : { opacity: 0, scale: 0.98, y: 14 }} animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={reduce ? undefined : { opacity: 0, scale: 1.01 }} transition={{ duration: 0.55, ease: EASE }}>
              <Frame shot={SHOTS[now]} alt={c.steps[now][0]} sizes="(min-width: 1024px) 640px, 100vw" />
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </section>
  );
}
