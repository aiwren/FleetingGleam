import { motion } from "motion/react";
import { copy, type Lang } from "../content";

const EASE = [0.16, 1, 0.3, 1] as const;
const rise = { initial: { opacity: 0, y: 24 }, whileInView: { opacity: 1, y: 0 }, viewport: { once: true, amount: 0.4 }, transition: { duration: 0.8, ease: EASE } } as const;

export default function Closing({ lang }: { lang: Lang }) {
  const c = copy[lang];
  return (
    <>
      <section className="mx-auto max-w-6xl px-5 pt-32 md:pt-48">
        <motion.h2 {...rise} className="max-w-3xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-tighter text-balance">{c.honest.title}</motion.h2>
        <motion.p {...rise} className="mt-6 max-w-2xl text-lg text-ash">{c.honest.body}</motion.p>
      </section>

      <section id="download" className="mx-auto max-w-6xl px-5 pb-8 pt-32 md:pt-48">
        <motion.div {...rise} className="relative overflow-hidden rounded-3xl border border-line bg-panel px-7 py-16 md:px-16 md:py-24">
          <div aria-hidden className="pointer-events-none absolute -right-24 -top-24 size-[420px] rounded-full bg-gleam/[0.12] blur-3xl" />
          <div className="relative">
            <h2 className="max-w-xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-tighter text-balance">{c.final.title}</h2>
            <p className="mt-5 max-w-lg text-lg text-ash">{c.final.body}</p>
            <span className="mt-9 inline-flex min-h-12 items-center whitespace-nowrap rounded-full border border-line bg-canvas/60 px-6 font-medium">{c.final.label}</span>
          </div>
        </motion.div>
      </section>

      <footer className="mx-auto max-w-6xl px-5 py-12 font-mono text-xs text-ash">{c.footer}</footer>
    </>
  );
}
