import { motion, useReducedMotion } from "motion/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";
import ScrollTilt from "./ScrollTilt";

const EASE = [0.16, 1, 0.3, 1] as const;

/** Where each note sits on the real screenshot, as a share of it (the wall view: groups, marks, inspector, sidebar note). */
const PINS = [
  { x: 35.6, y: 9.4, side: "right" }, // the gleam on the best frame of the first group
  { x: 37, y: 58, side: "right" }, // three near-identical frames, side by side
  { x: 79, y: 48.6, side: "left" }, // the inspector's breakdown of one score
  { x: 3.2, y: 84.5, side: "right" }, // "processed on-device"
] as const;

/**
 * The first screen is the product, large and still: the real app window with the real groups, marks and inspector. It arrives
 * out of focus and racks into it, one soft band of warm light crosses it, and four notes — each true of this very screenshot —
 * come up one after another. Then nothing moves until the reader scrolls, and scrolling lays the window flat.
 */
export default function HeroApp({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  const reduce = useReducedMotion();
  const t0 = reduce ? 0 : 1.5;
  return (
    <ScrollTilt from={16}>
      <motion.div className="relative"
        initial={reduce ? false : { opacity: 0, filter: "blur(14px)", scale: 1.03 }} animate={{ opacity: 1, filter: "blur(0px)", scale: 1 }}
        transition={{ duration: 1.1, ease: EASE, delay: 0.25 }}>
        <Frame shot="wall" alt={c.alt} priority sizes="(min-width: 1200px) 1112px, 100vw" />

        {/* One band of light, once, in the direction the pass reads. */}
        <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden rounded-3xl">
          <motion.div className="absolute inset-y-0 -left-1/3 w-1/3"
            style={{ background: "linear-gradient(105deg, transparent, color-mix(in srgb, var(--gleam) 30%, transparent) 50%, transparent)" }}
            initial={reduce ? { opacity: 0 } : { x: "0%", opacity: 1 }} animate={{ x: "400%", opacity: reduce ? 0 : 1 }} transition={{ duration: 1.5, ease: EASE, delay: 0.95 }} />
        </div>

        <div className="pointer-events-none absolute inset-0 hidden sm:block">
          {PINS.map((p, i) => (
            <div key={i} className="absolute" style={{ left: `${p.x}%`, top: `${p.y}%`, transform: `translate(${p.side === "left" ? "calc(-100% - 14px)" : "14px"}, -50%)` }}>
              <motion.span className="block whitespace-nowrap rounded-full bg-black/60 px-3 py-1.5 text-xs font-medium text-white backdrop-blur-md"
                initial={reduce ? false : { opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.7, ease: EASE, delay: t0 + i * 0.28 }}>
                <i className="mr-2 inline-block size-1.5 rounded-full bg-gleam align-middle" />{c.pins[i]}
              </motion.span>
            </div>
          ))}
        </div>
      </motion.div>
    </ScrollTilt>
  );
}
