import { useEffect, useState } from "react";
import { motion, useReducedMotion } from "motion/react";
import { Gleam } from "./Gleam";

const SPRING_OUT = { type: "spring", stiffness: 240, damping: 22, mass: 0.9 } as const;
const SPRING_LIFT = { type: "spring", stiffness: 420, damping: 30 } as const;

/**
 * Real photos, not stock cards — but only the ones that actually hold up at this size. Half of `/public/photos` carries a
 * dated warm-filter, lens-flare stock-photo look (wd0-3, field, forest) that reads as generic the moment it's the largest
 * thing on the page; st2/st3 are strong but black-and-white, which breaks with the rest of the site's colour. These four
 * are the only ones that look like something a person actually shot: fewer cards, all of them good, which is the page's
 * own headline in miniature.
 */
const CARDS = ["st0", "village", "boat", "st1"] as const;
const KEPT = 2; // the boat sits centre-top of the fan and is the one the app would keep.

/** Card footprint and spread in pixels, by breakpoint. Wide screens get a fan large enough to run past the container's own
 * edges — the point of a fan is that it fills the hand, not that it sits politely inside a box. */
const SIZE = { w: 100, h: 72, spread: 40, arc: 6 };
const SIZE_WIDE = { w: 230, h: 182, spread: 96, arc: 9 };
const SIZE_LARGE = { w: 320, h: 250, spread: 150, arc: 11 };

/**
 * The hero's one showpiece: a burst of real photos fanned like a hand of prints. It settles into place once, with weight,
 * then answers to the pointer — lift a card and its neighbours make room, the way a full hand parts for the one you pull out.
 * Only transform and shadow ever move; nothing here reflows layout.
 */
export default function HeroFan() {
  const reduce = useReducedMotion();
  const [hover, setHover] = useState<number | null>(null);
  const [tier, setTier] = useState<0 | 1 | 2>(0);
  useEffect(() => {
    const mWide = window.matchMedia("(min-width: 640px)");
    const mLarge = window.matchMedia("(min-width: 1024px)");
    const read = () => setTier(mLarge.matches ? 2 : mWide.matches ? 1 : 0);
    read();
    mWide.addEventListener("change", read);
    mLarge.addEventListener("change", read);
    return () => { mWide.removeEventListener("change", read); mLarge.removeEventListener("change", read); };
  }, []);
  const s = tier === 2 ? SIZE_LARGE : tier === 1 ? SIZE_WIDE : SIZE;
  const wide = tier > 0;
  const n = CARDS.length;
  const centre = (n - 1) / 2;

  return (
    <div className="relative left-1/2 w-screen -translate-x-1/2 select-none" style={{ perspective: 1400, height: s.h + 130 }}>
      <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2">
        {CARDS.map((photo, i) => {
          const off = i - centre;
          const angle = off * 7.5;
          const spread = off * s.spread;
          const arc = Math.abs(off) ** 1.4 * s.arc;
          const kept = i === KEPT;
          const lifted = hover === i;
          // A lifted card pulls its neighbours a little further along the fan, so the hand visibly parts to make room.
          const yield_ = hover !== null && hover !== i ? Math.sign(off - (hover - centre)) * (s.spread * 0.28) : 0;
          return (
            <motion.button key={photo} type="button" aria-label={photo} tabIndex={-1}
              onHoverStart={() => setHover(i)} onHoverEnd={() => setHover(null)}
              className="absolute left-0 top-0 origin-bottom cursor-default overflow-hidden rounded-2xl border border-line/70 bg-panel shadow-[var(--lift-sm)] transition-shadow duration-300"
              style={{
                width: s.w, height: s.h, marginLeft: -s.w / 2, marginTop: -s.h / 2,
                zIndex: lifted ? 50 : kept ? 30 : 20 - Math.round(Math.abs(off) * 2), boxShadow: lifted ? "var(--lift)" : undefined,
              }}
              initial={reduce ? false : { opacity: 0, x: 0, y: 40, rotate: 0, scale: 0.82 }}
              animate={{
                opacity: 1,
                x: spread + yield_,
                y: lifted ? arc - s.h * 0.22 : arc,
                rotate: lifted ? angle * 0.35 : angle,
                scale: lifted ? 1.07 : 1,
              }}
              transition={lifted || hover !== null ? SPRING_LIFT : { ...SPRING_OUT, delay: reduce ? 0 : 0.15 + i * 0.07 }}
              whileTap={{ scale: 1.03 }}>
              <img src={`/photos/${photo}.webp`} alt="" width={720} height={480} loading="eager" fetchPriority={i === KEPT ? "high" : "auto"} draggable={false}
                className="h-full w-full object-cover" />
              <div aria-hidden className="pointer-events-none absolute inset-0 shadow-[inset_0_1px_0_rgba(255,255,255,.18),inset_0_-32%_38%_-24%_rgba(0,0,0,.4)]" />
              {kept && (
                <motion.i aria-hidden initial={reduce ? false : { opacity: 0, scale: 0, rotate: -90 }} animate={{ opacity: 1, scale: 1, rotate: 0 }}
                  transition={{ type: "spring", stiffness: 380, damping: 22, delay: reduce ? 0 : 0.15 + KEPT * 0.07 + 0.4 }}
                  style={{ position: "absolute", right: s.w * 0.06, top: s.w * 0.06 }} className="drop-shadow-[0_1px_3px_rgba(0,0,0,.55)]">
                  <Gleam size={tier === 2 ? 26 : tier === 1 ? 19 : 12} />
                </motion.i>
              )}
            </motion.button>
          );
        })}
      </div>
    </div>
  );
}
