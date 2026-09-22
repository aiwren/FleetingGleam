import { motion, useReducedMotion } from "motion/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";

const EASE = [0.16, 1, 0.3, 1] as const;

/**
 * Three steps under one picture of the step that matters most: comparing the frames of a single moment. The steps sit in one tight
 * row rather than a tall tracked list, because three short lines do not earn a screen each.
 */
export default function Flow({ lang }: { lang: Lang }) {
  const c = copy[lang].flow;
  const reduce = useReducedMotion();
  return (
    <section id="flow" className="mx-auto max-w-6xl px-5 pt-40 md:pt-56">
      <motion.h2 initial={reduce ? false : { opacity: 0, y: 24 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true }} transition={{ duration: 0.8, ease: EASE }}
        className="max-w-2xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-[-0.035em] text-balance">{c.title}</motion.h2>

      <motion.div className="mt-12" initial={reduce ? false : { opacity: 0, y: 40 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true, amount: 0.15 }}
        transition={{ duration: 1, ease: EASE }}>
        <Frame shot="compare" alt={c.steps[1][0]} sizes="(min-width: 1200px) 1112px, 100vw" />
      </motion.div>

      <ol className="mt-14 grid gap-x-10 gap-y-9 border-t border-line pt-10 md:grid-cols-3">
        {c.steps.map(([t, d], i) => (
          <motion.li key={t} initial={reduce ? false : { opacity: 0, y: 20 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true, amount: 0.5 }}
            transition={{ duration: 0.7, ease: EASE, delay: i * 0.08 }}>
            <span className="grid size-9 place-items-center rounded-full bg-gleam text-sm tabular-nums text-gleam-ink">{i + 1}</span>
            <h3 className="mt-5 text-xl font-semibold tracking-tight">{t}</h3>
            <p className="mt-2 max-w-[34ch] text-ash">{d}</p>
          </motion.li>
        ))}
      </ol>
    </section>
  );
}
