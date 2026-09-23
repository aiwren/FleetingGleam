import { useEffect, useState, type MouseEvent } from "react";
import { motion, useReducedMotion, useSpring } from "motion/react";
import { Gleam } from "./Gleam";

const SPRING_OUT = { type: "spring", stiffness: 220, damping: 24, mass: 1 } as const;
const SPRING_LIFT = { type: "spring", stiffness: 460, damping: 28 } as const; // the card itself: quick, energetic
const SPRING_YIELD = { type: "spring", stiffness: 240, damping: 30 } as const; // everyone else: a slower, following give

/**
 * Real photos, not stock cards — and only the ones that actually hold up at this size. Half of the site's stock set carries
 * a dated warm-filter, lens-flare look that reads as generic the moment it's the largest thing on the page; this pulls from
 * both photo folders for the ones that don't: a mix of scenes (street, coast, architecture, a boat, snow, countryside) so
 * the fan reads as "a burst across a trip," not one repeated subject.
 */
const CARDS = [
  { src: "/wall/01.webp" }, // Pulpit Rock, Norway
  { src: "/photos/village.webp" }, // Santorini
  { src: "/photos/st0.webp" }, // a street, mid-stride
  { src: "/photos/boat.webp" }, // the keeper
  { src: "/photos/st1.webp" }, // SoHo
  { src: "/wall/11.webp" }, // Kings Canyon, sunrise
  { src: "/wall/03.webp" }, // Everest base camp
] as const;
const KEPT = 3; // dead centre of seven, and the one the app would keep.

/** Card footprint and spread in pixels, by breakpoint. Wide screens get a fan large enough to run past the container's own
 * edges — the point of a fan is that it fills the hand, not that it sits politely inside a box. */
const SIZE = { w: 92, h: 68, spread: 34, arc: 5.5 };
const SIZE_WIDE = { w: 190, h: 150, spread: 78, arc: 8 };
const SIZE_LARGE = { w: 258, h: 202, spread: 118, arc: 10 };

/**
 * The hero's one showpiece: a burst of real photos fanned like a hand of prints. It settles into place once, opening from
 * the centre outward the way a held stack actually fans, then answers to the pointer two ways: the whole group tilts very
 * slightly toward it for a moment of depth, and lifting one card — quick, energetic — makes its neighbours give way with a
 * slower, trailing spring, so the two motions read as leader and followers instead of one uniform jump.
 */
export default function HeroFan() {
  const reduce = useReducedMotion();
  const [hover, setHover] = useState<number | null>(null);
  const [entered, setEntered] = useState(false);
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
  useEffect(() => {
    if (reduce) { setEntered(true); return; }
    const t = setTimeout(() => setEntered(true), 1100);
    return () => clearTimeout(t);
  }, [reduce]);

  // A very light tilt toward the pointer, on the group as a whole — the one place this fan reaches for real depth.
  const rx = useSpring(0, { stiffness: 120, damping: 16 });
  const ry = useSpring(0, { stiffness: 120, damping: 16 });
  const onMove = (e: MouseEvent<HTMLDivElement>) => {
    if (reduce) return;
    const r = e.currentTarget.getBoundingClientRect();
    ry.set(((e.clientX - r.left) / r.width - 0.5) * 10);
    rx.set(((e.clientY - r.top) / r.height - 0.5) * -6);
  };
  const onLeave = () => { rx.set(0); ry.set(0); };

  const s = tier === 2 ? SIZE_LARGE : tier === 1 ? SIZE_WIDE : SIZE;
  const n = CARDS.length;
  const centre = (n - 1) / 2;

  return (
    <div className="relative left-1/2 w-screen -translate-x-1/2 select-none" style={{ perspective: 1400, height: s.h + 130 }}
      onMouseMove={onMove} onMouseLeave={onLeave}>
      <motion.div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2" style={{ rotateX: rx, rotateY: ry, transformStyle: "preserve-3d" }}>
        {CARDS.map(({ src }, i) => {
          const off = i - centre;
          const angle = off * 7.5;
          const spread = off * s.spread;
          const arc = Math.abs(off) ** 1.4 * s.arc;
          const kept = i === KEPT;
          const lifted = hover === i;
          // A lifted card pulls its neighbours a little further along the fan, so the hand visibly parts to make room.
          const yield_ = hover !== null && hover !== i ? Math.sign(off - (hover - centre)) * (s.spread * 0.3) : 0;
          const transition = lifted ? SPRING_LIFT : hover !== null || entered ? SPRING_YIELD : { ...SPRING_OUT, delay: reduce ? 0 : 0.1 + Math.abs(off) * 0.085 };
          return (
            <motion.button key={src} type="button" aria-label={src} tabIndex={-1}
              onHoverStart={() => setHover(i)} onHoverEnd={() => setHover(null)}
              className="absolute left-0 top-0 origin-bottom cursor-default overflow-hidden rounded-2xl border border-line/70 bg-panel shadow-[var(--lift-sm)] transition-shadow duration-300"
              style={{
                width: s.w, height: s.h, marginLeft: -s.w / 2, marginTop: -s.h / 2,
                zIndex: lifted ? 50 : kept ? 30 : 20 - Math.round(Math.abs(off) * 2), boxShadow: lifted ? "var(--lift)" : undefined,
              }}
              initial={reduce ? false : { opacity: 0, x: 0, y: 30, rotate: 0, scale: 0.8 }}
              animate={{
                opacity: 1,
                x: spread + yield_,
                y: lifted ? arc - s.h * 0.22 : arc,
                rotate: lifted ? angle * 0.35 : angle,
                scale: lifted ? 1.07 : 1,
              }}
              transition={transition}
              whileTap={{ scale: 1.03 }}>
              <img src={src} alt="" width={720} height={480} loading="eager" fetchPriority={kept ? "high" : "auto"} draggable={false}
                className="h-full w-full object-cover" />
              <div aria-hidden className="pointer-events-none absolute inset-0 shadow-[inset_0_1px_0_rgba(255,255,255,.18),inset_0_-32%_38%_-24%_rgba(0,0,0,.4)]" />
              {kept && (
                <motion.i aria-hidden initial={reduce ? false : { opacity: 0, scale: 0, rotate: -90 }} animate={{ opacity: 1, scale: 1, rotate: 0 }}
                  transition={{ type: "spring", stiffness: 380, damping: 22, delay: reduce ? 0 : 0.1 + 0.4 }}
                  style={{ position: "absolute", right: s.w * 0.06, top: s.w * 0.06 }} className="drop-shadow-[0_1px_3px_rgba(0,0,0,.55)]">
                  <Gleam size={tier === 2 ? 24 : tier === 1 ? 18 : 12} />
                </motion.i>
              )}
            </motion.button>
          );
        })}
      </motion.div>
    </div>
  );
}
