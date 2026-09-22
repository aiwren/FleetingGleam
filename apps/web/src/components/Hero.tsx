import { useCallback, useRef, type CSSProperties } from "react";
import { motion, useReducedMotion, useScroll, useTransform } from "motion/react";
import { copy, type Lang } from "../content";
import Wall from "./Wall";

const d = (s: number) => ({ "--d": `${s}s` }) as CSSProperties;

/** One headline line rises out of a mask; the first is set quiet and the second in full ink, so the sentence has two beats. */
function Line({ children, delay, quiet }: { children: string; delay: number; quiet?: boolean }) {
  return (
    <span className="block overflow-hidden pb-[0.12em]">
      <span className={`a-line block ${quiet ? "text-ash" : ""}`} style={d(delay)}>{children}</span>
    </span>
  );
}

/**
 * The opening holds still while the reader scrolls, and what they scroll is the work: the app's own wall of a trip's photographs,
 * read left to right, keepers taking the amber mark and the rest stepping back. By the end of the pinned run the wall has been
 * culled, which is the product in one screen.
 */
export default function Hero({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  const reduce = useReducedMotion();
  const track = useRef<HTMLDivElement>(null);

  // How far through the pinned run the reader has come, read on demand rather than held in React state.
  const progress = useCallback(() => {
    const el = track.current;
    if (!el) return 0;
    const r = el.getBoundingClientRect();
    const run = r.height - window.innerHeight;
    return run <= 0 ? 1 : Math.min(1, Math.max(0, -r.top / run));
  }, []);

  const { scrollYProgress } = useScroll({ target: track, offset: ["start start", "end end"] });
  // Once the pass begins the sentence steps back a little, rather than competing with the photographs.
  const textFade = useTransform(scrollYProgress, [0.06, 0.28], [1, 0.55]);

  return (
    <section ref={track} className="relative h-[260vh] md:h-[300vh]">
      <div className="sticky top-16 flex h-[calc(100dvh-4rem)] flex-col justify-center overflow-hidden">
        <div className="mx-auto w-full max-w-[1400px] px-5">
          <motion.div style={reduce ? undefined : { opacity: textFade }}
            className="flex flex-col gap-6 pb-8 md:flex-row md:items-end md:justify-between md:gap-12 md:pb-10">
            <h1 className="max-w-[16ch] text-[clamp(2rem,4vw,3.1rem)] font-semibold leading-[1.04] tracking-[-0.04em]">
              <Line delay={0.05} quiet>{c.title[0]}</Line>
              <Line delay={0.16}>{c.title[1]}</Line>
            </h1>
            <div className="a-fade shrink-0 md:max-w-[32ch] md:text-right" style={d(0.4)}>
              <p className="text-lg text-ash">{c.body}</p>
              <a href="#flow" className="mt-6 inline-flex min-h-12 items-center whitespace-nowrap rounded-full bg-ink px-6 py-3 font-medium text-on-ink transition-transform duration-150 active:translate-y-px">{c.cta}</a>
            </div>
          </motion.div>

          <div className="a-fade" style={d(0.6)}>
            <Wall lang={lang} progress={progress} />
          </div>
        </div>
      </div>
    </section>
  );
}
