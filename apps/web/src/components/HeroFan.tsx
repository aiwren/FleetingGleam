import { useEffect, useState, type MouseEvent } from "react";
import { AnimatePresence, motion, useReducedMotion, useScroll, useSpring, useTransform } from "motion/react";
import { Gleam } from "./Gleam";

const SPRING_OUT = { type: "spring", stiffness: 220, damping: 24, mass: 1 } as const;
const SPRING_LIFT = { type: "spring", stiffness: 460, damping: 28 } as const; // the card itself: quick, energetic
const SPRING_YIELD = { type: "spring", stiffness: 240, damping: 30 } as const; // everyone else: a slower, following give

/**
 * Real photos, not stock cards — and only the ones that actually hold up at this size (each checked one by one; half of the
 * site's stock set carries a dated filter look). The score on each is what the app would show on hover, and the keeper is
 * simply the highest of them: the whole product in one glance.
 */
const CARDS = [
  { src: "/wall/01.webp", score: 71 }, // Pulpit Rock, Norway
  { src: "/photos/village.webp", score: 64 }, // Santorini
  { src: "/photos/st0.webp", score: 58 }, // a street, mid-stride
  { src: "/photos/boat.webp", score: 94 }, // the keeper
  { src: "/photos/st1.webp", score: 77 }, // SoHo
  { src: "/wall/11.webp", score: 69 }, // Kings Canyon, sunrise
  { src: "/wall/03.webp", score: 73 }, // Everest base camp
] as const;
const KEPT = 3; // dead centre of seven, and the one the app would keep.

/**
 * The hero's one showpiece: a burst of real photos fanned like a hand of prints, sized from the viewport so it fills the
 * width at any screen instead of floating in it. It opens from the centre outward, then answers to the pointer: the group
 * tilts slightly toward it, a lifted card is quick and its neighbours give way on a slower spring, and the light behind the
 * fan takes the colour of whichever photo is in front. Scrolling away lets it settle back and recede. Only transform, opacity
 * and shadow ever move; nothing here reflows layout.
 */
export default function HeroFan() {
  const reduce = useReducedMotion();
  const [hover, setHover] = useState<number | null>(null);
  const [entered, setEntered] = useState(false);
  const [vw, setVw] = useState(1280);
  useEffect(() => {
    let raf = 0;
    const read = () => { raf = 0; setVw(window.innerWidth); };
    const on = () => { if (!raf) raf = requestAnimationFrame(read); };
    read();
    window.addEventListener("resize", on);
    return () => { window.removeEventListener("resize", on); if (raf) cancelAnimationFrame(raf); };
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

  const { scrollY } = useScroll();
  const drift = useTransform(scrollY, [0, 520], [0, -48]);
  const shrink = useTransform(scrollY, [0, 520], [1, 0.93]);

  const phone = vw < 640;
  const w = Math.round(Math.min(400, Math.max(96, vw * 0.2)));
  const h = Math.round(w * 0.78);
  const step = w * (phone ? 0.36 : 0.6);
  const arcK = w * 0.045;
  const n = CARDS.length;
  const centre = (n - 1) / 2;
  const active = hover ?? KEPT;

  return (
    <div className="relative isolate left-1/2 w-screen -translate-x-1/2 select-none" style={{ perspective: 1400, height: h + Math.round(w * 0.5) }}
      onMouseMove={onMove} onMouseLeave={onLeave}>
      {/* The light behind the fan is the front photo's own colour, so the page and the picture never disagree. */}
      <div aria-hidden className="pointer-events-none absolute inset-0 -z-10 overflow-hidden opacity-[.36]"
        style={{ maskImage: "radial-gradient(closest-side, #000 30%, transparent 100%)", WebkitMaskImage: "radial-gradient(closest-side, #000 30%, transparent 100%)" }}>
        <AnimatePresence>
          <motion.img key={CARDS[active].src} src={CARDS[active].src} alt="" draggable={false}
            initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.8, ease: [0.16, 1, 0.3, 1] }}
            className="absolute left-1/2 top-1/2 h-[120%] w-[68%] max-w-none -translate-x-1/2 -translate-y-1/2 object-cover blur-[70px] saturate-150" />
        </AnimatePresence>
      </div>

      <motion.div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2"
        style={{ rotateX: rx, rotateY: ry, y: reduce ? 0 : drift, scale: reduce ? 1 : shrink, transformStyle: "preserve-3d" }}>
        {CARDS.map(({ src, score }, i) => {
          const off = i - centre;
          const angle = off * 7;
          const spread = off * step;
          const arc = Math.abs(off) ** 1.4 * arcK;
          const kept = i === KEPT;
          const lifted = hover === i;
          // A lifted card pulls its neighbours a little further along the fan, so the hand visibly parts to make room.
          const yield_ = hover !== null && hover !== i ? Math.sign(off - (hover - centre)) * (step * 0.3) : 0;
          const transition = lifted ? SPRING_LIFT : hover !== null || entered ? SPRING_YIELD : { ...SPRING_OUT, delay: reduce ? 0 : 0.1 + Math.abs(off) * 0.085 };
          return (
            <motion.button key={src} type="button" aria-label={`${score}`} tabIndex={-1}
              onHoverStart={() => setHover(i)} onHoverEnd={() => setHover(null)}
              className="absolute left-0 top-0 origin-bottom cursor-default overflow-hidden rounded-[clamp(12px,1.2vw,20px)] border border-line/70 bg-panel shadow-[var(--lift-sm)] transition-shadow duration-300"
              style={{
                width: w, height: h, marginLeft: -w / 2, marginTop: -h / 2,
                zIndex: lifted ? 50 : kept ? 30 : 20 - Math.round(Math.abs(off) * 2), boxShadow: lifted ? "var(--lift)" : undefined,
              }}
              initial={reduce ? false : { opacity: 0, x: 0, y: 30, rotate: 0, scale: 0.8 }}
              animate={{
                opacity: 1,
                x: spread + yield_,
                y: lifted ? arc - h * 0.22 : arc,
                rotate: lifted ? angle * 0.35 : angle,
                scale: lifted ? 1.07 : 1,
              }}
              transition={transition}
              whileTap={{ scale: 1.03 }}>
              <img src={src} alt="" width={720} height={480} loading="eager" fetchPriority={kept ? "high" : "auto"} draggable={false}
                className="h-full w-full object-cover" />
              <div aria-hidden className="pointer-events-none absolute inset-0 shadow-[inset_0_1px_0_rgba(255,255,255,.18),inset_0_-32%_38%_-24%_rgba(0,0,0,.5)]" />
              <span aria-hidden className="pointer-events-none absolute tabular-nums text-white/90 [text-shadow:0_1px_2px_rgba(0,0,0,.5)]"
                style={{ left: w * 0.06, bottom: w * 0.05, fontSize: Math.max(10, Math.min(15, w * 0.055)), fontWeight: kept ? 600 : 500, opacity: kept ? 1 : 0.8 }}>
                {score}
              </span>
              {kept && (
                <motion.i aria-hidden initial={reduce ? false : { opacity: 0, scale: 0, rotate: -90 }} animate={{ opacity: 1, scale: 1, rotate: 0 }}
                  transition={{ type: "spring", stiffness: 380, damping: 22, delay: reduce ? 0 : 0.1 + 0.4 }}
                  style={{ position: "absolute", right: w * 0.06, top: w * 0.06 }} className="drop-shadow-[0_1px_3px_rgba(0,0,0,.55)]">
                  <Gleam size={Math.round(Math.max(12, Math.min(30, w * 0.085)))} />
                </motion.i>
              )}
            </motion.button>
          );
        })}
      </motion.div>
    </div>
  );
}
