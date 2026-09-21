import { useRef } from "react";
import { motion, useReducedMotion, useScroll, useTransform } from "motion/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";

const EASE = [0.16, 1, 0.3, 1] as const;

/** One headline line rises out of a mask; the lines follow each other so the sentence is read in order. */
function Line({ children, delay }: { children: string; delay: number }) {
  const reduce = useReducedMotion();
  return (
    <span className="block overflow-hidden pb-[0.12em]">
      <motion.span className="block" initial={reduce ? false : { y: "105%" }} animate={{ y: 0 }} transition={{ duration: 0.9, ease: EASE, delay }}>{children}</motion.span>
    </span>
  );
}

export default function Hero({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  const reduce = useReducedMotion();
  const shot = useRef<HTMLDivElement>(null);
  // The picture starts tilted away and lies flat as it reaches the reader: the app "comes forward" while the page is scrolled to it.
  const { scrollYProgress } = useScroll({ target: shot, offset: ["start 95%", "start 25%"] });
  const rotateX = useTransform(scrollYProgress, [0, 1], [16, 0]);
  const scale = useTransform(scrollYProgress, [0, 1], [0.92, 1]);
  const fade = (delay: number) => (reduce ? {} : { initial: { opacity: 0, y: 16 }, animate: { opacity: 1, y: 0 }, transition: { duration: 0.8, ease: EASE, delay } });

  return (
    <section className="mx-auto max-w-6xl px-5 pt-16 md:pt-24">
      <h1 className="max-w-4xl text-[clamp(2.25rem,6vw,4.5rem)] font-semibold leading-[1.05] tracking-tighter">
        <Line delay={0.05}>{c.title[0]}</Line>
        <Line delay={0.16}>{c.title[1]}</Line>
      </h1>
      <motion.p {...fade(0.4)} className="mt-6 max-w-xl text-lg text-ash">{c.body}</motion.p>
      <motion.div {...fade(0.5)} className="mt-9 flex flex-wrap gap-3">
        <a href="#download" className="inline-flex min-h-12 items-center whitespace-nowrap rounded-full bg-ink px-6 font-medium text-on-ink transition-transform duration-150 active:translate-y-px">{c.cta}</a>
        <a href="#flow" className="inline-flex min-h-12 items-center whitespace-nowrap rounded-full border border-line px-6 font-medium transition-colors hover:bg-surface active:translate-y-px">{c.secondary}</a>
      </motion.div>

      <div ref={shot} className="mt-14 md:mt-20" style={{ perspective: 1600 }}>
        <motion.div style={reduce ? undefined : { rotateX, scale, transformOrigin: "50% 0%" }} initial={reduce ? false : { opacity: 0, y: 60 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 1.1, ease: EASE, delay: 0.55 }}>
          <Frame src="/shots/wall.jpg" alt={c.alt} priority />
        </motion.div>
      </div>
    </section>
  );
}
