import { motion } from "motion/react";
import { copy, type Lang } from "../content";

const EASE = [0.16, 1, 0.3, 1] as const;
const rise = { initial: { opacity: 0, y: 24 }, whileInView: { opacity: 1, y: 0 }, viewport: { once: true, amount: 0.4 }, transition: { duration: 0.8, ease: EASE } } as const;

export default function Closing({ lang }: { lang: Lang }) {
  const c = copy[lang];
  return (
    <>
      {/* Set to the right on purpose: the first statement hugs the left edge, this one answers it from the other side. */}
      <section className="mx-auto max-w-6xl px-5 pt-40 md:pt-56">
        <div className="md:ml-[34%]">
          <motion.h2 {...rise} className="max-w-2xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-tighter text-balance">{c.honest.title}</motion.h2>
          <motion.p {...rise} className="mt-6 max-w-xl text-lg text-ash">{c.honest.body}</motion.p>
        </div>
      </section>

      <section id="download" className="mx-auto max-w-6xl px-5 pb-8 pt-40 md:pt-56">
        <motion.div {...rise} className="relative isolate overflow-hidden rounded-3xl border border-line">
          <img src="/photos/sunset.jpg" alt="" width={1600} height={900} loading="lazy" draggable={false} className="absolute inset-0 -z-20 h-full w-full object-cover" />
          {/* A scrim keeps the words readable on any part of the photograph. */}
          <div aria-hidden className="absolute inset-0 -z-10 bg-gradient-to-r from-black/75 via-black/45 to-black/10" />
          <div className="px-7 py-20 text-white md:px-16 md:py-32">
            <h2 className="max-w-xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-tighter text-balance">{c.final.title}</h2>
            <p className="mt-5 max-w-lg text-lg text-white/80">{c.final.body}</p>
            <span className="mt-9 inline-flex min-h-12 items-center whitespace-nowrap rounded-full border border-white/30 bg-black/35 px-6 font-medium backdrop-blur-md">{c.final.label}</span>
          </div>
        </motion.div>
      </section>

      <footer className="mx-auto max-w-6xl px-5 py-12 font-mono text-xs text-ash">{c.footer}</footer>
    </>
  );
}
