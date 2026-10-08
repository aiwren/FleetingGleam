import { motion } from "motion/react";
import { copy, type Lang } from "../content";
import HeroAnnotate from "./HeroAnnotate";

const EASE = [0.16, 1, 0.3, 1] as const;

/** One photograph, read the way the app reads it: the measurements are drawn onto the things they measure. */
export default function AnnotateSection({ lang }: { lang: Lang }) {
  const c = copy[lang].read;
  return (
    <section className="mx-auto max-w-6xl px-5 pt-40 md:pt-56">
      <motion.div initial={{ opacity: 0, y: 24 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true }} transition={{ duration: 0.8, ease: EASE }} className="mb-16 max-w-2xl">
        <h2 className="text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-[-0.035em] text-balance">{c.title}</h2>
        <p className="mt-3 max-w-md text-ash">{c.body}</p>
      </motion.div>
      <HeroAnnotate lang={lang} />
    </section>
  );
}
