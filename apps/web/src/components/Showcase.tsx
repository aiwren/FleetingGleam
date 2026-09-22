import { motion, useReducedMotion } from "motion/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";

const EASE = [0.16, 1, 0.3, 1] as const;

/** The app itself, once, as large as the page allows and flat on the page: the photographs in it are the argument. */
export default function Showcase({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  const reduce = useReducedMotion();
  return (
    <section className="relative mx-auto max-w-[1360px] px-5 pt-16 md:pt-24">
      <div aria-hidden className="pointer-events-none absolute inset-x-8 -top-10 h-[420px] opacity-70"
        style={{ background: "radial-gradient(55% 60% at 50% 40%, color-mix(in srgb, var(--gleam) 12%, transparent), transparent 70%)" }} />
      <motion.div className="relative"
        initial={reduce ? false : { opacity: 0, y: 44 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true, amount: 0.15 }}
        transition={{ duration: 1, ease: EASE }}>
        <Frame shot="wall" alt={c.alt} sizes="(min-width: 1400px) 1320px, 100vw" />
      </motion.div>
    </section>
  );
}
