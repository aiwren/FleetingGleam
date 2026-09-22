import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { motion, useReducedMotion, useScroll, useTransform } from "motion/react";
import { copy, type Lang } from "../content";
import BurstScene from "./Burst";

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
 * The opening holds still while the reader scrolls, and what they scroll is the work itself: a burst of six frames of one moment,
 * advancing towards them a frame at a time, each read and scored, until only the one worth keeping is left in focus. The sentence
 * sits on the left over the page's own light, the way it would over a photograph.
 */
export default function Hero({ lang }: { lang: Lang }) {
  const c = copy[lang].hero, b = copy[lang].burst;
  const reduce = useReducedMotion();
  const track = useRef<HTMLDivElement>(null);
  const [compact, setCompact] = useState<boolean | null>(null);

  useEffect(() => {
    const m = window.matchMedia("(max-width: 860px)");
    const read = () => setCompact(m.matches);
    read();
    m.addEventListener("change", read);
    return () => m.removeEventListener("change", read);
  }, []);

  // How far through the pinned run the reader has come, read on demand rather than held in React state.
  const progress = useCallback(() => {
    const el = track.current;
    if (!el) return 0;
    const r = el.getBoundingClientRect();
    const run = r.height - window.innerHeight;
    return run <= 0 ? 1 : Math.min(1, Math.max(0, -r.top / run));
  }, []);

  const { scrollYProgress } = useScroll({ target: track, offset: ["start start", "end end"] });
  // Once the reading begins, the sentence steps back to a whisper instead of competing with the frames.
  const textFade = useTransform(scrollYProgress, [0.08, 0.3], [1, 0.32]);

  return (
    <section ref={track} className="relative h-[300vh] md:h-[340vh]">
      <div className="sticky top-16 h-[calc(100dvh-4rem)] overflow-hidden">
        {compact !== null && <BurstScene compact={compact} progress={progress} />}

        {/* The words keep a side of the page to themselves: below the frames on a phone, beside them on a wide screen. */}
        <div aria-hidden className="pointer-events-none absolute inset-0 bg-gradient-to-t from-canvas from-20% via-canvas/60 via-38% to-transparent md:bg-gradient-to-r md:from-42% md:via-canvas/55 md:via-50% md:to-45%" />

        <div className="relative mx-auto flex h-full max-w-6xl flex-col justify-end px-5 pb-14 md:justify-center md:pb-0">
          <motion.div style={reduce || compact ? undefined : { opacity: textFade }}>
            <h1 className="max-w-[15ch] text-[clamp(2.1rem,4.2vw,3.4rem)] font-semibold leading-[1.03] tracking-[-0.042em]">
              <Line delay={0.05} quiet>{c.title[0]}</Line>
              <Line delay={0.16}>{c.title[1]}</Line>
            </h1>
            <p className="a-fade mt-6 max-w-[30ch] text-lg text-ash" style={d(0.4)}>{c.body}</p>
            <div className="a-fade mt-9" style={d(0.5)}>
              <a href="#flow" className="inline-flex min-h-12 items-center whitespace-nowrap rounded-full bg-ink px-6 py-3 font-medium text-on-ink transition-transform duration-150 active:translate-y-px">{c.cta}</a>
            </div>
          </motion.div>
        </div>

        <p className="pointer-events-none absolute inset-x-0 bottom-4 mx-auto max-w-6xl px-5 text-right text-xs text-ash/70">{b.note}</p>
      </div>
    </section>
  );
}
