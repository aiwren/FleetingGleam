import { useState, type MouseEvent } from "react";
import { motion, useReducedMotion, useSpring } from "motion/react";
import { copy, type Lang } from "../content";
import { Gleam } from "./Gleam";

const EASE = [0.16, 1, 0.3, 1] as const;

/**
 * Where each callout is pinned, as a share of the photograph itself, so a mark stays on the boat however the page is sized.
 * `side` is the way its hairline runs out of the frame; the order matches `copy.hero.marks`.
 */
const PINS = [
  { x: 27, y: 64, side: "left", len: 120 }, // the hull: where focus is sharpest
  { x: 21, y: 52, side: "left", len: 70 }, // the flag: the subject sits on the thirds
  { x: 74, y: 27, side: "right", len: 110 }, // open water: highlights and shadows both hold
  { x: 64, y: 78, side: "right", len: 70 }, // the water against the hull
] as const;

/**
 * The first screen shows what the app does to one photograph: hairline marks are drawn onto it one after another, pinned to
 * the actual things they measure, and the frame ends up lit with the verdict. The two frames behind it are the takes that
 * did not make it. Nothing is a diagram; the picture is the real keeper from the sample set.
 */
export default function HeroAnnotate({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  const reduce = useReducedMotion();
  const [tilt, setTilt] = useState(false);
  const rx = useSpring(0, { stiffness: 110, damping: 18 });
  const ry = useSpring(0, { stiffness: 110, damping: 18 });
  const onMove = (e: MouseEvent<HTMLDivElement>) => {
    if (reduce) return;
    const r = e.currentTarget.getBoundingClientRect();
    ry.set(((e.clientX - r.left) / r.width - 0.5) * 6);
    rx.set(((e.clientY - r.top) / r.height - 0.5) * -4);
  };
  const onLeave = () => { rx.set(0); ry.set(0); setTilt(false); };
  const t0 = reduce ? 0 : 0.9; // the marks start once the frame has landed

  return (
    <div className="relative mx-auto" style={{ width: "min(100%, 720px, calc((100dvh - 24rem) * 1.5))", perspective: 1600 }}
      onMouseMove={(e) => { setTilt(true); onMove(e); }} onMouseLeave={onLeave} data-tilt={tilt}>
      <motion.div className="relative" style={{ rotateX: rx, rotateY: ry, transformStyle: "preserve-3d" }}
        initial={reduce ? false : { opacity: 0, y: 36, scale: 0.97 }} animate={{ opacity: 1, y: 0, scale: 1 }} transition={{ duration: 1.1, ease: EASE, delay: 0.2 }}>
        {/* The takes that did not make it, showing only their edges. */}
        <div aria-hidden className="absolute inset-0 -rotate-[3.2deg] overflow-hidden rounded-[22px] border border-line/70 bg-panel opacity-60 shadow-[var(--lift-sm)]" style={{ transform: "translateY(-16px) rotate(-3.2deg) scale(.97)" }}>
          <img src="/wall/04.webp" alt="" draggable={false} className="h-full w-full object-cover" />
        </div>
        <div aria-hidden className="absolute inset-0 overflow-hidden rounded-[22px] border border-line/70 bg-panel opacity-80 shadow-[var(--lift-sm)]" style={{ transform: "translateY(-8px) rotate(2.4deg) scale(.985)" }}>
          <img src="/photos/boat.webp" alt="" draggable={false} className="h-full w-full object-cover opacity-60" />
        </div>

        <div className="relative aspect-[3/2] overflow-hidden rounded-[22px] border border-line/70 bg-panel shadow-[var(--lift)]">
          <img src="/photos/boat.webp" alt={c.alt} width={720} height={480} loading="eager" fetchPriority="high" draggable={false} className="h-full w-full object-cover" />
          <div aria-hidden className="pointer-events-none absolute inset-0 shadow-[inset_0_1px_0_rgba(255,255,255,.18),inset_0_0_0_1px_rgba(255,255,255,.04)]" />
          {/* The verdict: a lit rim and the app's own mark, arriving after the measurements. */}
          <motion.div aria-hidden className="pointer-events-none absolute inset-0 rounded-[22px] shadow-[inset_0_0_0_2px_var(--gleam)]"
            initial={reduce ? false : { opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: 0.8, ease: EASE, delay: t0 + PINS.length * 0.22 + 0.3 }} />
          <motion.div className="absolute right-4 top-4 flex items-center gap-2 rounded-full bg-black/45 py-1.5 pl-2.5 pr-3.5 text-[13px] font-medium text-white backdrop-blur-md"
            initial={reduce ? false : { opacity: 0, y: -8, scale: 0.9 }} animate={{ opacity: 1, y: 0, scale: 1 }}
            transition={{ type: "spring", stiffness: 320, damping: 22, delay: t0 + PINS.length * 0.22 + 0.3 }}>
            <Gleam size={18} /><span className="tabular-nums">94</span><span className="opacity-70">{c.kept}</span>
          </motion.div>
        </div>

        {/* Hairlines run out of the frame to their labels; below sm they fold into the strip underneath instead. */}
        <div className="pointer-events-none absolute inset-0 hidden sm:block">
          {PINS.map((p, i) => {
            const [name, value] = c.marks[i];
            const delay = t0 + i * 0.22;
            const left = p.side === "left";
            return (
              <div key={name} className="absolute inset-x-0" style={{ top: `${p.y}%` }}>
                <motion.i className="absolute block size-[10px] -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/90 bg-black/40 shadow-[0_0_0_3px_rgba(0,0,0,.18)]"
                  style={{ left: `${p.x}%` }}
                  initial={reduce ? false : { scale: 0 }} animate={{ scale: 1 }} transition={{ type: "spring", stiffness: 420, damping: 20, delay }} />
                <motion.i className="absolute top-0 block h-px bg-ink/45"
                  style={left ? { left: -p.len, right: `calc(${100 - p.x}% + 5px)`, transformOrigin: "right" } : { left: `calc(${p.x}% + 5px)`, right: -p.len, transformOrigin: "left" }}
                  initial={reduce ? false : { scaleX: 0 }} animate={{ scaleX: 1 }} transition={{ duration: 0.6, ease: EASE, delay: delay + 0.1 }} />
                <motion.span className={`absolute top-0 -translate-y-1/2 whitespace-nowrap text-xs leading-tight ${left ? "text-right" : "text-left"}`}
                  style={left ? { right: `calc(100% + ${p.len + 12}px)` } : { left: `calc(100% + ${p.len + 12}px)` }}
                  initial={reduce ? false : { opacity: 0, x: left ? 8 : -8 }} animate={{ opacity: 1, x: 0 }} transition={{ duration: 0.6, ease: EASE, delay: delay + 0.4 }}>
                  <b className="block font-medium text-ink">{name}</b>
                  <span className="tabular-nums text-ash">{value}</span>
                </motion.span>
              </div>
            );
          })}
        </div>
      </motion.div>

      <ul className="mt-5 flex flex-wrap justify-center gap-x-5 gap-y-1.5 text-xs text-ash sm:hidden">
        {c.marks.map(([name, value]) => <li key={name}><b className="font-medium text-ink">{name}</b> {value}</li>)}
      </ul>
    </div>
  );
}
