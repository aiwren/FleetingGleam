import { motion } from "motion/react";
import { copy, type Lang } from "../content";
import Footer from "./Footer";

const EASE = [0.16, 1, 0.3, 1] as const;
const rise = { initial: { opacity: 0, y: 24 }, whileInView: { opacity: 1, y: 0 }, viewport: { once: true, amount: 0.4 }, transition: { duration: 0.8, ease: EASE } } as const;

export default function Closing({ lang }: { lang: Lang }) {
  const c = copy[lang];
  return (
    <>
      {/* A pause before the close: one statement, set large, with a hairline to mark it off rather than an empty column. */}
      <section className="mx-auto max-w-6xl px-5 pt-40 md:pt-56">
        <div className="border-t border-line pt-12 md:pt-16">
          <motion.h2 {...rise} className="max-w-[26ch] text-[clamp(2rem,4.8vw,3.5rem)] font-semibold leading-[1.06] tracking-[-0.038em] text-balance">{c.honest.title}</motion.h2>
          <motion.p {...rise} className="mt-7 max-w-[52ch] text-lg text-ash">{c.honest.body}</motion.p>
        </div>
      </section>

      <section id="download" className="mx-auto max-w-6xl px-5 pb-8 pt-20 md:pt-28">
        <motion.div {...rise} className="relative isolate overflow-hidden rounded-3xl border border-line">
          <img src="/photos/sunset-1600.webp" srcSet="/photos/sunset-800.webp 800w, /photos/sunset-1600.webp 1600w" sizes="(min-width: 1200px) 1152px, 100vw" alt="" width={1600} height={900} loading="lazy" draggable={false} className="absolute inset-0 -z-20 h-full w-full object-cover" />
          {/* A scrim keeps the words readable on any part of the photograph. */}
          <div aria-hidden className="absolute inset-0 -z-10 bg-gradient-to-r from-black/75 via-black/45 to-black/10" />
          <div className="px-7 py-20 text-white md:px-16 md:py-32">
            <h2 className="max-w-xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-[-0.035em] text-balance">{c.final.title}</h2>
            <p className="mt-5 max-w-lg text-lg text-white/80">{c.final.body}</p>
            <span className="mt-9 inline-flex min-h-12 items-center whitespace-nowrap py-3 rounded-full border border-white/30 bg-black/35 px-6 font-medium backdrop-blur-md">{c.final.label}</span>
          </div>
        </motion.div>
      </section>

      <Footer lang={lang} />
    </>
  );
}
