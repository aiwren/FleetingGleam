import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useInView, useReducedMotion } from "motion/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";

const SHOTS = ["wall", "compare", "wall"] as const;
const EASE = [0.16, 1, 0.3, 1] as const;

/** Whichever step is crossing the middle of the window is the current one, and it can also simply be clicked. */
function Step({ n, title, body, on, onEnter, onPick }: { n: number; title: string; body: string; on: boolean; onEnter: () => void; onPick: () => void }) {
  const ref = useRef<HTMLLIElement>(null);
  const hit = useInView(ref, { margin: "-50% 0px -50% 0px", amount: 0 });
  useEffect(() => { if (hit) onEnter(); }, [hit]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <li ref={ref}>
      <button type="button" onClick={onPick} aria-current={on} className="flex w-full gap-4 py-4 text-left">
        <span className={`mt-0.5 grid size-8 shrink-0 place-items-center rounded-full border text-[13px] tabular-nums transition-colors duration-500 ${on ? "border-transparent bg-gleam text-gleam-ink" : "border-line text-ash"}`}>{n}</span>
        <span className={`transition-opacity duration-500 ${on ? "opacity-100" : "opacity-45"}`}>
          <span className="block text-xl font-semibold tracking-tight">{title}</span>
          <span className="mt-1.5 block max-w-md text-ash">{body}</span>
        </span>
      </button>
    </li>
  );
}

/**
 * Three steps, close together, with the picture on the right keeping up. Scrolling past moves through them, and so does
 * clicking one: the same linkage either way, without a screen of empty space between the steps to carry it.
 */
export default function Flow({ lang }: { lang: Lang }) {
  const c = copy[lang].flow;
  const reduce = useReducedMotion();
  const [now, setNow] = useState(0);

  return (
    <section id="flow" className="mx-auto max-w-6xl px-5 pt-40 md:pt-56">
      <motion.h2 initial={reduce ? false : { opacity: 0, y: 24 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true }} transition={{ duration: 0.8, ease: EASE }}
        className="max-w-2xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-[-0.035em] text-balance">{c.title}</motion.h2>

      <div className="mt-12 grid items-center gap-x-14 gap-y-10 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)]">
        <ol>
          {c.steps.map(([t, d], i) => (
            <Step key={t} n={i + 1} title={t} body={d} on={now === i} onEnter={() => setNow(i)} onPick={() => setNow(i)} />
          ))}
        </ol>

        <div>
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.div key={now}
              initial={reduce ? false : { opacity: 0, scale: 0.985, y: 12 }} animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={reduce ? undefined : { opacity: 0, scale: 1.008 }} transition={{ duration: 0.5, ease: EASE }}>
              <Frame shot={SHOTS[now]} alt={c.steps[now][0]} sizes="(min-width: 1024px) 660px, 100vw" />
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </section>
  );
}
