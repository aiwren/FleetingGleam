import { useCallback, useRef } from "react";
import { motion, useReducedMotion, useScroll, useTransform } from "motion/react";
import { copy, type Lang } from "../content";
import Wall from "./Wall";

const EASE = [0.16, 1, 0.3, 1] as const;

/**
 * The second act: the app's own photo wall, alive in the page, with the full pass scrubbed by scroll. The hero already made
 * its case in one screen; this is where the reader who wants to see it work can stay a while and watch the whole pass run.
 */
export default function WallDemo({ lang }: { lang: Lang }) {
  const c = copy[lang].wall;
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
  // Once the pass begins the heading steps back a little, rather than competing with the photographs.
  const textFade = useTransform(scrollYProgress, [0.12, 0.34], [1, 0.55]);

  return (
    <section ref={track} className="relative h-[300vh] md:h-[360vh]">
      <div className="sticky top-16 flex h-[calc(100dvh-4rem)] flex-col justify-center overflow-hidden">
        <div className="mx-auto w-full max-w-[1400px] px-5">
          <motion.div style={reduce ? undefined : { opacity: textFade }} className="max-w-2xl pb-8 md:pb-10">
            <motion.div initial={{ opacity: 0, y: 24 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true }} transition={{ duration: 0.8, ease: EASE }}>
              <h2 className="text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-[-0.035em] text-balance">{c.title}</h2>
              <p className="mt-3 max-w-md text-ash">{c.body}</p>
            </motion.div>
          </motion.div>

          <Wall lang={lang} progress={progress} />
        </div>
      </div>
    </section>
  );
}
