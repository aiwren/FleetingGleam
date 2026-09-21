import { useRef, type CSSProperties } from "react";
import { motion, useReducedMotion, useScroll, useTransform } from "motion/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";

const d = (s: number) => ({ "--d": `${s}s` }) as CSSProperties;

/** One headline line rises out of a mask; the second is set in the full ink colour so the sentence has a first and a second beat. */
function Line({ children, delay, quiet }: { children: string; delay: number; quiet?: boolean }) {
  return (
    <span className="block overflow-hidden pb-[0.12em]">
      <span className={`a-line block ${quiet ? "text-ash" : ""}`} style={d(delay)}>{children}</span>
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

  return (
    <section className="mx-auto max-w-6xl px-5 pt-16 md:pt-24">
      <h1 className="max-w-4xl text-[clamp(2.25rem,6vw,4.5rem)] font-semibold leading-[1.05] tracking-[-0.035em]">
        <Line delay={0.05} quiet>{c.title[0]}</Line>
        <Line delay={0.16}>{c.title[1]}</Line>
      </h1>
      <p className="a-fade mt-6 max-w-xl text-lg text-ash" style={d(0.4)}>{c.body}</p>
      <div className="a-fade mt-9 flex flex-wrap gap-3" style={d(0.5)}>
        <a href="#download" className="inline-flex min-h-12 items-center whitespace-nowrap py-3 rounded-full bg-ink px-6 font-medium text-on-ink transition-transform duration-150 active:translate-y-px">{c.cta}</a>
        <a href="#flow" className="inline-flex min-h-12 items-center whitespace-nowrap py-3 rounded-full border border-line px-6 font-medium transition-colors hover:bg-surface active:translate-y-px">{c.secondary}</a>
      </div>

      <div ref={shot} className="a-shot relative mt-14 md:mt-20" style={{ perspective: 1600 }}>
        {/* The light of the photographs spills softly behind the window, as it would from a lit screen in a dark room. */}
        <div aria-hidden className="pointer-events-none absolute inset-x-[8%] -bottom-8 top-10 -z-10 overflow-hidden rounded-[48px] opacity-40 blur-3xl saturate-150">
          <img src="/shots/glow-dark.webp" alt="" width={96} height={60} className="for-dark h-full w-full object-cover" />
          <img src="/shots/glow-light.webp" alt="" width={96} height={60} className="for-light h-full w-full object-cover" />
        </div>
        <motion.div style={reduce ? undefined : { rotateX, scale, transformOrigin: "50% 0%" }}>
          <Frame shot="wall" alt={c.alt} priority />
        </motion.div>
      </div>
    </section>
  );
}
