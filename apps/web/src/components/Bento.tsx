import { useEffect, useState, type ReactNode, type MouseEvent } from "react";
import { AnimatePresence, motion, useMotionTemplate, useMotionValue, useReducedMotion } from "motion/react";
import { Check, MagnifyingGlass } from "@phosphor-icons/react";
import { copy, type Lang } from "../content";
import Frame from "./Frame";

const EASE = [0.16, 1, 0.3, 1] as const;

/** A tile whose border catches a soft light that follows the pointer. Motion values keep it off the React render path. */
function Tile({ className = "", i, children }: { className?: string; i: number; children: ReactNode }) {
  const x = useMotionValue(-200), y = useMotionValue(-200);
  const glow = useMotionTemplate`radial-gradient(360px circle at ${x}px ${y}px, color-mix(in srgb, var(--gleam) 16%, transparent), transparent 70%)`;
  const move = (e: MouseEvent<HTMLElement>) => { const r = e.currentTarget.getBoundingClientRect(); x.set(e.clientX - r.left); y.set(e.clientY - r.top); };
  return (
    <motion.article onMouseMove={move} initial={{ opacity: 0, y: 40 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true, amount: 0.25 }} transition={{ duration: 0.8, ease: EASE, delay: (i % 2) * 0.08 }}
      className={`group relative flex flex-col overflow-hidden rounded-3xl border border-line p-7 md:p-8 ${className}`}>
      <motion.div aria-hidden className="pointer-events-none absolute inset-0 opacity-0 transition-opacity duration-500 group-hover:opacity-100" style={{ background: glow }} />
      <div className="relative flex flex-1 flex-col">{children}</div>
    </motion.article>
  );
}

const H = ({ children }: { children: ReactNode }) => <h3 className="text-xl font-semibold tracking-tight">{children}</h3>;
const P = ({ children }: { children: ReactNode }) => <p className="mt-2 max-w-md text-ash">{children}</p>;

/** Kinds of scene take turns to light up: the app really does weigh each of them differently. */
function Kinds({ kinds }: { kinds: readonly string[] }) {
  const reduce = useReducedMotion();
  const [on, setOn] = useState(0);
  useEffect(() => { if (reduce) return; const id = setInterval(() => setOn((v) => (v + 1) % kinds.length), 1400); return () => clearInterval(id); }, [kinds.length, reduce]);
  return (
    <div className="mt-auto flex flex-wrap gap-2 pt-8">
      {kinds.map((k, i) => (
        <span key={k} className={`relative rounded-full border px-3.5 py-1.5 text-sm transition-colors duration-500 ${i === on ? "border-transparent text-gleam-ink" : "border-line text-ash"}`}>
          {i === on && <motion.i layoutId="kind-pill" className="absolute inset-0 rounded-full bg-gleam" transition={{ type: "spring", stiffness: 260, damping: 28 }} />}
          <span className="relative">{k}</span>
        </span>
      ))}
    </div>
  );
}

/** Types a few descriptions one after another, the way the search box is used; the photo that matches appears once it is typed. */
function Typer({ phrases }: { phrases: readonly (readonly [string, string])[] }) {
  const reduce = useReducedMotion();
  const [text, setText] = useState(reduce ? phrases[0][0] : "");
  const [shown, setShown] = useState<string | null>(reduce ? phrases[0][1] : null);
  useEffect(() => {
    if (reduce) return;
    let alive = true, p = 0, n = 0, dir = 1, timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      if (!alive) return;
      const chars = [...phrases[p][0]];
      n += dir; setText(chars.slice(0, n).join(""));
      if (dir === 1 && n >= chars.length) { setShown(phrases[p][1]); dir = -1; timer = setTimeout(tick, 2200); return; }
      if (dir === -1 && n === chars.length - 1) setShown(null);
      if (dir === -1 && n <= 0) { dir = 1; p = (p + 1) % phrases.length; timer = setTimeout(tick, 350); return; }
      timer = setTimeout(tick, dir === 1 ? 55 : 22);
    };
    timer = setTimeout(tick, 600);
    return () => { alive = false; clearTimeout(timer); };
  }, [phrases, reduce]);
  return (
    <div className="mt-8" aria-hidden>
      <div className="flex h-12 items-center gap-3 rounded-full border border-line bg-canvas/60 px-5 text-ink">
        <MagnifyingGlass size={18} className="shrink-0 text-ash" />
        <span className="truncate">{text}</span>
        <motion.i className="-ml-2 h-5 w-px bg-gleam" animate={reduce ? undefined : { opacity: [1, 0, 1] }} transition={{ duration: 1, repeat: Infinity }} />
      </div>
      <div className="relative mt-4 aspect-[3/2] overflow-hidden rounded-2xl">
        <AnimatePresence>
          {shown && (
            <motion.img key={shown} src={`/photos/${shown}.webp`} alt="" width={720} height={480} loading="lazy" draggable={false}
              initial={{ opacity: 0, scale: 1.06 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.7, ease: EASE }}
              className="absolute inset-0 h-full w-full object-cover" />
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

/** Styles are switched, not stacked: one is active, and the active one glides. */
function Styles({ styles }: { styles: readonly string[] }) {
  const [on, setOn] = useState(0);
  return (
    <div className="mt-8 inline-flex w-fit max-w-full flex-wrap gap-1 rounded-full border border-line bg-canvas/60 p-1">
      {styles.map((s, i) => (
        <button key={s} type="button" onClick={() => setOn(i)} aria-pressed={i === on}
          className={`relative rounded-full px-4 py-1.5 text-sm transition-colors duration-300 active:translate-y-px ${i === on ? "text-on-ink" : "text-ash hover:text-ink"}`}>
          {i === on && <motion.i layoutId="style-pill" className="absolute inset-0 rounded-full bg-ink" transition={{ type: "spring", stiffness: 300, damping: 30 }} />}
          <span className="relative">{s}</span>
        </button>
      ))}
    </div>
  );
}

/** A folder of past picks: each photo gets its amber mark in turn, the way the app reads them as the taste to learn from. */
function Picks() {
  const reduce = useReducedMotion();
  const names = ["field", "village", "boat", "forest"];
  return (
    <div className="mt-auto grid grid-cols-2 gap-3 pt-10 sm:grid-cols-4" aria-hidden>
      {names.map((n, i) => (
        <div key={n} className="relative aspect-[3/2] overflow-hidden rounded-2xl">
          <img src={`/photos/${n}.webp`} alt="" width={720} height={480} loading="lazy" draggable={false} className="h-full w-full object-cover" />
          <motion.span initial={reduce ? false : { scale: 0, opacity: 0 }} whileInView={{ scale: 1, opacity: 1 }} viewport={{ once: true, amount: 0.8 }}
            transition={{ type: "spring", stiffness: 380, damping: 22, delay: 0.5 + i * 0.28 }}
            className="absolute right-2 top-2 grid size-6 place-items-center rounded-full bg-gleam text-gleam-ink"><Check size={13} weight="bold" /></motion.span>
        </div>
      ))}
    </div>
  );
}

export default function Bento({ lang }: { lang: Lang }) {
  const c = copy[lang].features;
  return (
    <section id="features" className="mx-auto max-w-6xl px-5 pt-40 md:pt-56">
      <motion.h2 initial={{ opacity: 0, y: 24 }} whileInView={{ opacity: 1, y: 0 }} viewport={{ once: true }} transition={{ duration: 0.8, ease: EASE }}
        className="max-w-3xl text-[clamp(1.9rem,4.4vw,3.25rem)] font-semibold leading-[1.1] tracking-[-0.035em] text-balance">{c.title}</motion.h2>
      <div className="mt-14 grid gap-4 md:grid-cols-6">
        <Tile i={0} className="bg-panel md:col-span-4">
          <H>{c.a.t}</H><P>{c.a.d}</P>
          <div className="relative mt-8 h-[260px] overflow-hidden rounded-2xl md:h-[300px]">
            {(["dark", "light"] as const).map((t) => (
              <img key={t} src={`/shots/compare-${t}-1200.webp`} srcSet={`/shots/compare-${t}-1200.webp 1200w, /shots/compare-${t}-2400.webp 2400w`} sizes="(min-width: 768px) 700px, 100vw" alt={c.a.alt} loading="lazy" width={2400} height={1500} draggable={false}
                className={`h-full w-full object-cover object-top transition-transform duration-700 ease-out-expo group-hover:scale-[1.03] ${t === "dark" ? "for-dark" : "for-light"}`} />
            ))}
          </div>
        </Tile>
        <Tile i={1} className="bg-gleam/[0.09] md:col-span-2">
          <H>{c.b.t}</H><P>{c.b.d}</P><Kinds kinds={c.b.kinds} />
        </Tile>
        <Tile i={2} className="bg-surface md:col-span-2">
          <H>{c.c.t}</H><P>{c.c.d}</P><Typer phrases={c.c.phrases} />
        </Tile>
        <Tile i={3} className="bg-panel md:col-span-4">
          <H>{c.d.t}</H><P>{c.d.d}</P><Styles styles={c.d.styles} /><Picks />
        </Tile>
      </div>
    </section>
  );
}
