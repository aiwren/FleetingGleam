import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { motion, useMotionValueEvent, useReducedMotion, useScroll } from "motion/react";
import { copy, type Lang } from "../content";
import { Gleam } from "./Gleam";
import { Motes, ST, paletteFromTheme, sampleImage, type FrameSpec } from "./motes";

const EASE = [0.16, 1, 0.3, 1] as const;
const POP = [0.34, 1.56, 0.64, 1] as const;

/** The eight frames the sea condenses into, in the order they form. Three are kept; of those, one is best. */
const SHOTS = ["/wall/01.webp", "/wall/16.webp", "/wall/07.webp", "/wall/04.webp", "/wall/12.webp", "/wall/09.webp", "/wall/13.webp", "/wall/15.webp"] as const;
const KEPT = [1, 5, 6] as const;
const BEST = 1;
const isKept = (i: number) => (KEPT as readonly number[]).includes(i);

/**
 * The stage in its own units, scaled as one piece to fit the column. A phone gets its own stage rather than a shrunk copy of the
 * desktop one, so the frames and the card stay a size that can be read. `grid` places the eight frames; `final` is where the
 * three kept ones end up once the best is chosen (left, top, scale).
 */
type Geo = {
  W: number; H: number; FW: number; FH: number; grid: readonly (readonly [number, number])[];
  final: Record<number, readonly [number, number, number]>; cardTop: number; cardW: number; cols: number; rows: number; field: number; haze: number; max: number; travel: string;
};
const DESK: Geo = {
  W: 780, H: 560, FW: 168, FH: 112,
  grid: [[24, 92], [212, 92], [400, 92], [588, 92], [24, 224], [212, 224], [400, 224], [588, 224]],
  final: { 1: [306, 96, 1.55], 5: [48, 236, 1], 6: [564, 236, 1] },
  cardTop: 372, cardW: 324, cols: 34, rows: 23, field: 2000, haze: 260, max: 1.2, travel: "320vh",
};
const PHONE: Geo = {
  W: 360, H: 330, FW: 78, FH: 52,
  grid: [[12, 14], [98, 14], [184, 14], [270, 14], [12, 74], [98, 74], [184, 74], [270, 74]],
  final: { 1: [141, 70, 1.6], 5: [12, 8, 0.95], 6: [270, 8, 0.95] },
  cardTop: 156, cardW: 300, cols: 18, rows: 12, field: 520, haze: 120, max: 1.15, travel: "280vh",
};

/** How far the story has come: 0 the sea, 1 the photographs, 2 a few kept, 3 one best, 4 the reasons. */
export type Phase = 0 | 1 | 2 | 3 | 4;
const phaseAt = (p: number): Phase => (p >= ST.explain ? 4 : p >= ST.best ? 3 : p >= ST.decide ? 2 : p >= ST.developEnd - 0.03 ? 1 : 0);

/**
 * The first screen, pinned while the reader scrolls through it. A sea of light motes fills it; scrolling condenses most of them
 * into eight frames, develops the photographs under them, keeps three (the motes of the other five come loose and drift back
 * into the sea), raises the best of the three, and lays out the reasons on glass. The reader sets the pace and can scroll it
 * backwards. Whatever the scroll, the sea answers the pointer and the frames lean toward it.
 * The words of the first screen are passed in and told how far the story has come, so they keep step with it.
 */
export default function HeroScene({ lang, children }: { lang: Lang; children: (phase: Phase) => ReactNode }) {
  const c = copy[lang].hero;
  const reduce = useReducedMotion();
  const track = useRef<HTMLElement>(null);
  const pin = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const imgs = useRef<(HTMLImageElement | null)[]>([]);
  const motes = useRef<Motes | null>(null);
  const [u, setU] = useState(0);
  const [phone, setPhone] = useState(false);
  const [phase, setPhaseState] = useState<Phase>(reduce ? 4 : 0);
  const [dev, setDev] = useState(reduce ? 1 : 0); // how far the photographs have developed, 0 to 1
  const [glint, setGlint] = useState(0);
  const phaseRef = useRef<Phase>(phase);
  const setPhase = (p: Phase) => { if (p !== phaseRef.current) { phaseRef.current = p; setPhaseState(p); } };
  const g = phone ? PHONE : DESK;

  // The stage is fitted to its column and to what is left of the pinned screen under the words.
  useEffect(() => {
    const el = box.current!, pinned = pin.current!;
    const read = () => {
      const isPhone = window.innerWidth < 768;
      const geo = isPhone ? PHONE : DESK;
      const pr = pinned.getBoundingClientRect(), br = el.getBoundingClientRect();
      const room = Math.max(0.5 * geo.H, pr.bottom - br.top - 20);
      setPhone(isPhone);
      setU(Math.min(geo.max, br.width / geo.W, room / geo.H));
    };
    read();
    const ro = new ResizeObserver(read);
    ro.observe(el); ro.observe(pinned);
    window.addEventListener("resize", read);
    document.fonts?.ready.then(read);
    return () => { ro.disconnect(); window.removeEventListener("resize", read); };
  }, []);

  // Scroll progress through the pinned stretch drives the story.
  const { scrollYProgress } = useScroll({ target: track, offset: ["start start", "end end"] });
  const follow = (p: number) => {
    setPhase(phaseAt(p));
    const n = Math.min(1, Math.max(0, (p - ST.develop) / (ST.developEnd - ST.develop)));
    setDev((v) => (Math.abs(n - v) > 0.01 || n === 0 || n === 1 ? n : v));
  };
  useMotionValueEvent(scrollYProgress, "change", (p) => { if (motes.current?.ok) motes.current.setProgress(p); else if (!reduce) follow(p); });

  // The motes. Reduced motion skips them altogether: the photographs are simply there, chosen.
  useEffect(() => {
    if (reduce || !u || !canvas.current || !pin.current) return;
    const pinned = pin.current;
    const { cols, rows } = g;
    let m = motes.current;
    if (!m) {
      m = new Motes(canvas.current, paletteFromTheme(), { field: g.field, haze: g.haze });
      motes.current = m;
      if (import.meta.env.DEV) (window as unknown as { __fgMotes?: Motes }).__fgMotes = m; // 开发时便于在控制台查看
    }
    if (!m.ok) return;
    const mm = m;
    const frames = (): FrameSpec[] => {
      const pr = pinned.getBoundingClientRect();
      const st = stage.current!.getBoundingClientRect();
      return SHOTS.map((_, i) => {
        const img = imgs.current[i];
        const colors = img && img.complete && img.naturalWidth ? sampleImage(img, cols, rows) : null;
        const [gx, gy] = g.grid[i];
        return { rect: { x: st.left - pr.left + gx * u, y: st.top - pr.top + gy * u, w: g.FW * u, h: g.FH * u }, cols, rows, colors, order: i, out: !isKept(i) };
      });
    };
    const size = () => { const r = pinned.getBoundingClientRect(); mm.resize(r.width, r.height, window.devicePixelRatio || 1); };
    // the nebula's heart sits where the frames will form
    const focus = () => {
      const pr = pinned.getBoundingClientRect(), st = stage.current!.getBoundingClientRect();
      const [x0, y0] = g.grid[0], [x1, y1] = g.grid[g.grid.length - 1];
      const cx = st.left - pr.left + ((x0 + x1 + g.FW) / 2) * u, cy = st.top - pr.top + ((y0 + y1 + g.FH) / 2) * u;
      mm.setFocus(cx / pr.width, cy / pr.height, phone ? 0.62 : 0.5);
    };
    size(); focus();
    mm.setFrames(frames());
    mm.setProgress(scrollYProgress.get());
    mm.onTick = follow;
    mm.start();
    // the pixels arrive when the photographs have loaded; until then the motes carry their own light
    Promise.all(SHOTS.map((_, i) => imgs.current[i]?.decode?.().catch(() => undefined))).then(() => { if (motes.current === mm) mm.setFrames(frames()); });
    const ro = new ResizeObserver(() => { size(); focus(); mm.setFrames(frames()); });
    ro.observe(pinned);
    // the sea only runs while it can be seen
    const io = new IntersectionObserver(([e]) => { if (e.isIntersecting && document.visibilityState === "visible") mm.resume(); else mm.pause(); }, { threshold: 0 });
    io.observe(track.current!);
    const vis = () => { if (document.visibilityState === "visible") mm.resume(); else mm.pause(); };
    document.addEventListener("visibilitychange", vis);
    // mote colours follow the theme
    const theme = () => mm.setPalette(paletteFromTheme());
    const mo = new MutationObserver(theme);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    mq.addEventListener("change", theme);
    return () => { ro.disconnect(); io.disconnect(); mo.disconnect(); mq.removeEventListener("change", theme); document.removeEventListener("visibilitychange", vis); };
  }, [u, phone, reduce]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => () => { motes.current?.dispose(); motes.current = null; }, []);

  // Keeping a photo lights its gleam and lets a few motes go from it, as the app does.
  const burstAt = (i: number, scale: number) => {
    const m = motes.current, pinned = pin.current, st = stage.current;
    if (!m || !pinned || !st || reduce) return;
    const pr = pinned.getBoundingClientRect(), r = st.getBoundingClientRect();
    const fin = g.final[i];
    const [gx, gy] = phaseRef.current >= 3 && fin ? [fin[0], fin[1]] : g.grid[i];
    const cx = gx + g.FW / 2, cy = gy + g.FH / 2;
    const sx = cx + (g.FW / 2 - 14) * scale, sy = cy - (g.FH / 2 - 14) * scale;
    m.burst(r.left - pr.left + sx * u, r.top - pr.top + sy * u, 18);
  };
  const chosen = phase >= 2, bested = phase >= 3;
  useEffect(() => {
    if (!chosen) return;
    const ids = KEPT.map((i, k) => setTimeout(() => burstAt(i, 1), 180 + k * 120));
    return () => ids.forEach(clearTimeout);
  }, [chosen]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!bested) return;
    setGlint((v) => v + 1);
    const id = setTimeout(() => burstAt(BEST, g.final[BEST][2]), 700);
    return () => clearTimeout(id);
  }, [bested]); // eslint-disable-line react-hooks/exhaustive-deps

  const onMove = (e: React.PointerEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    motes.current?.pointer(e.clientX - r.left, e.clientY - r.top);
    if (reduce || !stage.current) return;
    const nx = ((e.clientX - r.left) / r.width - 0.5) * 2, ny = ((e.clientY - r.top) / r.height - 0.5) * 2;
    stage.current.style.setProperty("--ry", `${nx * 7}deg`);
    stage.current.style.setProperty("--rx", `${ny * -5}deg`);
  };
  const onLeave = () => { motes.current?.pointerOut(); stage.current?.style.setProperty("--ry", "0deg"); stage.current?.style.setProperty("--rx", "0deg"); };
  // a touch or click anywhere on the first screen lets a few motes go from that point
  const onDown = (e: React.PointerEvent<HTMLElement>) => {
    if ((e.target as HTMLElement).closest("a, button")) return;
    const r = e.currentTarget.getBoundingClientRect();
    motes.current?.burst(e.clientX - r.left, e.clientY - r.top);
  };

  const stageStyle: CSSProperties = { width: g.W, height: g.H, transform: `scale(${u || 1})`, transformOrigin: "0 0", opacity: u ? 1 : 0, perspective: 1400 };
  const developed = reduce ? 1 : dev;

  return (
    <section ref={track} className="relative" style={{ height: reduce ? "auto" : g.travel }}>
      <div ref={pin} className="sticky top-16 flex h-[calc(100dvh-4rem)] items-center overflow-hidden" onPointerMove={onMove} onPointerLeave={onLeave} onPointerDown={onDown}>
        {/* The sea of light fills the pinned screen; it sits under everything and takes no pointer. */}
        {!reduce && <canvas ref={canvas} aria-hidden className="hero-sea pointer-events-none absolute inset-0 z-0 h-full w-full" />}

        <div className="relative z-10 mx-auto grid w-full max-w-[1400px] items-center gap-x-10 gap-y-6 px-5 py-6 md:grid-cols-[minmax(0,5.2fr)_minmax(0,6.8fr)] md:gap-x-14 md:py-10">
          <div className="relative z-10">{children(phase)}</div>

          <div ref={box} className="relative mx-auto w-full max-w-[420px] md:max-w-none" style={{ height: g.H * (u || 1) }}>
            <div aria-hidden className="pointer-events-none absolute left-1/2 top-[46%] -z-10 h-[70%] w-[88%] -translate-x-1/2 -translate-y-1/2 rounded-full opacity-60 blur-3xl"
              style={{ background: "radial-gradient(closest-side, color-mix(in srgb, var(--gleam) 20%, transparent), transparent)" }} />
            <div ref={stage} style={stageStyle}>
              <div className="hero-tilt relative h-full w-full">
                {SHOTS.map((src, i) => {
                  const kept = isKept(i), best = i === BEST;
                  const [gx, gy] = g.grid[i];
                  const fin = g.final[i];
                  // where it sits now: the grid until the best is chosen, then its final place (turned-down frames are gone by then)
                  const to = bested && fin ? { x: fin[0] - gx, y: fin[1] - gy, scale: fin[2] } : { x: 0, y: 0, scale: 1 };
                  const out = chosen && !kept;
                  return (
                    <motion.div key={src} className="absolute" style={{ left: gx, top: gy, width: g.FW, height: g.FH, zIndex: best ? 3 : kept ? 2 : 1, pointerEvents: out ? "none" : "auto" }}
                      initial={false}
                      animate={{ ...to, opacity: out ? 0 : 1, z: bested ? (best ? 80 : kept ? 20 : 0) : 0 }}
                      transition={{ duration: out ? 0.45 : 1.1, ease: EASE, delay: bested && kept && !best ? 0.08 : 0 }}>
                      <motion.div className="relative h-full w-full overflow-hidden rounded-[10px]" whileHover={!reduce && !out ? { scale: 1.035 } : undefined}
                        style={{
                          opacity: developed, filter: `blur(${(1 - developed) * 10}px) brightness(${1 + (1 - developed) * 0.4})`,
                          boxShadow: chosen && kept ? `0 0 0 1.5px color-mix(in srgb, var(--gleam) ${best && bested ? 80 : 55}%, transparent), var(--lift)` : "var(--lift-sm)",
                          transition: "box-shadow .6s",
                        }}
                        onPointerEnter={() => { if (best && bested) setGlint((v) => v + 1); }}>
                        <img ref={(el) => { imgs.current[i] = el; }} src={src} alt="" width={600} height={400} fetchPriority={i < 2 ? "high" : "auto"} decoding="async" draggable={false}
                          className="block h-full w-full object-cover" />
                        {best && bested && glint > 0 && <span key={glint} aria-hidden className="hero-glint" />}
                        {kept && (
                          <motion.span className="absolute right-1.5 top-1.5 drop-shadow-[0_0_8px_color-mix(in_srgb,var(--gleam)_85%,transparent)]" initial={false}
                            animate={chosen ? { opacity: 1, scale: 1, rotate: 0 } : { opacity: 0, scale: 0, rotate: -90 }} transition={{ duration: 0.6, ease: POP, delay: 0.15 + KEPT.indexOf(i as 1 | 5 | 6) * 0.12 }}>
                            <Gleam size={best ? 20 : 16} />
                          </motion.span>
                        )}
                        {best && (["lt", "rt", "lb", "rb"] as const).map((k, j) => (
                          // the camera's focus frame locks on to the best one
                          <motion.i key={k} aria-hidden className={`hero-bracket ${k}`} initial={false}
                            animate={bested ? { opacity: 1, scale: 1 } : { opacity: 0, scale: 1.25 }} transition={{ duration: 0.55, ease: EASE, delay: 0.5 + j * 0.03 }} />
                        ))}
                      </motion.div>
                      {kept && !best && (
                        <motion.span className="glass absolute -bottom-2 left-1/2 flex -translate-x-1/2 items-center gap-1 whitespace-nowrap rounded-full px-2 py-0.5 text-[10px] font-medium text-ink" initial={false}
                          animate={chosen ? { opacity: 1, y: 0 } : { opacity: 0, y: 6 }} transition={{ duration: 0.5, ease: EASE, delay: 0.6 }}>
                          <Gleam size={9} />{c.scene.kept}
                        </motion.span>
                      )}
                    </motion.div>
                  );
                })}

                {/* The reasons, on glass, with the sea moving behind it. */}
                <motion.div className="glass absolute left-1/2 rounded-2xl px-4 py-3.5 text-left" style={{ top: g.cardTop, width: g.cardW, marginLeft: -g.cardW / 2, zIndex: 4, pointerEvents: phase >= 4 ? "auto" : "none" }} initial={false}
                  animate={phase >= 4 ? { opacity: 1, y: 0, z: 40, scale: 1 } : { opacity: 0, y: 14, z: 0, scale: 0.97 }} transition={{ duration: 0.8, ease: EASE }}>
                  <div className="flex items-baseline justify-between">
                    <div className="flex items-baseline gap-1.5">
                      <span className="text-[30px] font-semibold leading-none tracking-tight tabular-nums">{c.scene.score}</span>
                      <span className="text-xs text-ash">{c.scene.overall}</span>
                    </div>
                    <span className="flex items-center gap-1 rounded-full bg-gleam px-2 py-0.5 text-[11px] font-semibold text-gleam-ink"><Gleam size={11} />{c.scene.best}</span>
                  </div>
                  <ul className="mt-3 space-y-2">
                    {c.scene.reasons.map(([label, v], i) => (
                      <li key={label} className="grid grid-cols-[88px_1fr_auto] items-center gap-2.5 text-[12px]">
                        <span className="text-ash">{label}</span>
                        <span className="h-1 overflow-hidden rounded-full bg-line">
                          <motion.i className="block h-full rounded-full bg-gleam" initial={false} animate={{ scaleX: phase >= 4 ? v / 100 : 0 }} style={{ transformOrigin: "0 50%" }} transition={{ duration: 0.9, ease: EASE, delay: 0.25 + i * 0.12 }} />
                        </span>
                        <span className="tabular-nums">{v}</span>
                      </li>
                    ))}
                  </ul>
                  <p className="mt-3 text-[12.5px] leading-snug text-ink/90">{c.scene.why}</p>
                </motion.div>

                <span className="absolute bottom-0 right-1 text-[11px] text-ash/70">{c.scene.sample}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
