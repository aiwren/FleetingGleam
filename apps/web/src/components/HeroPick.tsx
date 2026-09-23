import { useRef } from "react";
import { motion, useInView, useReducedMotion } from "motion/react";
import { Gleam } from "./Gleam";

const EASE = [0.16, 1, 0.3, 1] as const;
/** Frames 8 to 10 of the wall: three takes of one moment. Only the last survives, which is the whole job in miniature. */
const FRAMES = [8, 9, 10] as const;
const pad = (n: number) => String(n).padStart(2, "0");

/**
 * The hero's one payoff, resolved in one screen rather than a scroll: three near-identical frames settle, the keeper lights up,
 * the other two step back. Not a diagram of the product, real tiles wearing the app's own mark.
 */
export default function HeroPick() {
  const reduce = useReducedMotion();
  const ref = useRef<HTMLDivElement>(null);
  const on = useInView(ref, { once: true, amount: 0.5 });
  const settled = reduce || on;
  return (
    <div ref={ref} className="flex gap-2.5">
      {FRAMES.map((n, i) => {
        const kept = i === FRAMES.length - 1;
        return (
          <motion.figure key={n} initial={{ opacity: 0, y: 16 }} animate={settled ? { opacity: 1, y: 0 } : undefined}
            transition={{ duration: 0.7, ease: EASE, delay: 0.15 + i * 0.09 }}
            className="relative aspect-[3/2] w-24 shrink-0 overflow-hidden rounded-xl sm:w-28">
            <motion.img src={`/wall/${pad(n)}.webp`} alt="" width={280} height={187} draggable={false} loading="eager" fetchPriority="high"
              initial={false} animate={settled ? { opacity: kept ? 1 : 0.5, scale: kept ? 1 : 0.94 } : { opacity: 1, scale: 1 }}
              transition={{ duration: 0.6, ease: EASE, delay: 0.85 }}
              className="h-full w-full object-cover" />
            {kept && (
              <motion.i aria-hidden initial={{ opacity: 0, scale: 0, rotate: -90 }} animate={settled ? { opacity: 1, scale: 1, rotate: 0 } : undefined}
                transition={{ type: "spring", stiffness: 380, damping: 22, delay: 0.95 }}
                className="absolute right-1.5 top-1.5 drop-shadow-[0_1px_3px_rgba(0,0,0,0.5)]"><Gleam size={15} /></motion.i>
            )}
          </motion.figure>
        );
      })}
    </div>
  );
}
