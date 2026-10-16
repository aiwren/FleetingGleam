import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { motion, useMotionValueEvent, useReducedMotion, useScroll } from "motion/react";
import { copy, type Lang } from "../content";
import { Gleam } from "./Gleam";
import { Motes, ST, armPoint, paletteFromTheme, sampleImage, type FrameSpec, type Pose } from "./motes";

const EASE = [0.16, 1, 0.3, 1] as const;
const POP = [0.34, 1.56, 0.64, 1] as const;
/** The canvas runs past the pinned screen on every side, so the tilted nebula still covers it to the corners. */
const OVER = 0.2;
/** How far the nebula's plane leans back, until the best is chosen and the camera comes round to face it. */
const LEAN = 16;

/** The eight frames the sea condenses into, in the order they form. Three are kept; of those, one is best. */
const SHOTS = ["/wall/01.webp", "/wall/16.webp", "/wall/07.webp", "/wall/04.webp", "/wall/12.webp", "/wall/09.webp", "/wall/13.webp", "/wall/15.webp"] as const;
const KEPT = [1, 5, 6] as const;
const BEST = 1;
const isKept = (i: number) => (KEPT as readonly number[]).includes(i);

/**
 * The stage in its own units, scaled as one piece to fit the column. A phone gets its own stage rather than a shrunk copy of the
 * desktop one, so the frames and the card stay a size that can be read. The nebula's heart is the stage's centre; `spots` says
 * which arm each frame rides and how far out; `final` is where the three kept ones end up once the best is chosen (left, top, scale).
 */
type Spec = {
  W: number; H: number; FW: number; FH: number; reach: number;
  spots: readonly (readonly [number, number])[];
  final: Record<number, readonly [number, number, number]>; cardTop: number; cardW: number; cols: number; rows: number; field: number; max: number; travel: string;
};
type Geo = Spec & { grid: readonly (readonly [number, number])[] };
/** The frames' resting places (left, top) with the nebula unturned: the layout the photographs are measured and animated from. */
function lay(sp: Spec): Geo {
  const grid = sp.spots.map(([arm, r]) => { const [x, y] = armPoint(sp.W / 2, sp.H / 2, arm, r, sp.reach); return [x - sp.FW / 2, y - sp.FH / 2] as const; });
  return { ...sp, grid };
}
const DESK: Geo = lay({
  W: 780, H: 560, FW: 140, FH: 94, reach: 340,
  spots: [[0, 110], [1, 110], [2, 120], [1, 210], [0, 240], [2, 250], [0, 320], [1, 320]],
  final: { 1: [320, 100, 1.6], 5: [48, 236, 1.05], 6: [592, 236, 1.05] },
  cardTop: 372, cardW: 324, cols: 30, rows: 20, field: 2000, max: 1.2, travel: "320vh",
});
const PHONE: Geo = lay({
  W: 360, H: 330, FW: 72, FH: 48, reach: 165,
  spots: [[0, 44], [1, 52], [2, 52], [0, 100], [2, 100], [1, 108], [0, 148], [1, 148]],
  final: { 1: [144, 70, 1.7], 5: [10, 8, 1], 6: [278, 8, 1] },
  cardTop: 156, cardW: 300, cols: 18, rows: 12, field: 520, max: 1.15, travel: "280vh",
});

/** How far the story has come: 0 the sea, 1 the photographs, 2 a few kept, 3 one best, 4 the reasons. */
export type Phase = 0 | 1 | 2 | 3 | 4;
const phaseAt = (p: number): Phase => (p >= ST.explain ? 4 : p >= ST.best ? 3 : p >= ST.decide ? 2 : p >= ST.developEnd - 0.03 ? 1 : 0);

/** Layout position of `el` inside `root`, from the offset chain: what CSS laid out, untouched by any transform. */
function offsetIn(el: HTMLElement, root: HTMLElement): [number, number] {
  let x = 0, y = 0, e: HTMLElement | null = el;
  while (e && e !== root) { x += e.offsetLeft; y += e.offsetTop; e = e.offsetParent as HTMLElement | null; }
  return [x, y];
}

/**
 * The first screen, pinned while the reader scrolls through it. A nebula of light motes fills it, leaning back a little in
 * perspective; the photographs live on its arms and turn with it, each on a thread of light from the heart. Scrolling condenses
 * the motes into the eight frames, develops the photographs under them, keeps three (the motes of the other five come loose and
 * drift back into the sea), then the camera comes round, the best of the three rises, and the reasons arrive on glass.
 * The reader sets the pace and can scroll it backwards. Whatever the scroll, the sea answers the pointer and the whole plane,
 * photographs and all, leans toward it. The words of the first screen are passed in and told how far the story has come.
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
  const rides = useRef<(HTMLDivElement | null)[]>([]); // the element that carries each frame round with the nebula
  const motes = useRef<Motes | null>(null);
  const geom = useRef({ u: 1, sx: 0, sy: 0, pw: 1, ph: 1 }); // stage scale and its place in canvas pixels, for the ride
  const orbit = useRef(1); // how much of the ride is applied: 1 on the arms, eased to 0 once the frames leave them
  const posesRef = useRef<Pose[]>([]);
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

  /** Puts each photograph where the nebula has carried its frame to, as a plain transform, every tick; no React render. */
  const ride = (poses: Pose[]) => {
    posesRef.current = poses;
    const { u, sx, sy } = geom.current;
    const target = phaseRef.current >= 3 ? 0 : 1;
    orbit.current += (target - orbit.current) * 0.08;
    const w = orbit.current < 0.002 ? 0 : orbit.current;
    poses.forEach((pose, i) => {
      const el = rides.current[i];
      if (!el) return;
      const [gx, gy] = g.grid[i];
      const left = (pose.x - sx) / u - g.FW / 2, top = (pose.y - sy) / u - g.FH / 2;
      el.style.transform = w === 0 ? "" : `translate(${(left - gx) * w}px, ${(top - gy) * w}px) rotate(${pose.rot * w}rad)`;
    });
  };

  // The motes. Reduced motion skips them altogether: the photographs are simply there, chosen.
  useEffect(() => {
    if (reduce || !u || !canvas.current || !pin.current) return;
    const pinned = pin.current, cv = canvas.current;
    const { cols, rows } = g;
    let m = motes.current;
    if (!m) {
      m = new Motes(cv, paletteFromTheme(), { field: g.field });
      motes.current = m;
      if (import.meta.env.DEV) (window as unknown as { __fgMotes?: Motes }).__fgMotes = m; // 开发时便于在控制台查看
    }
    if (!m.ok) return;
    const mm = m;
    // the stage's place in canvas pixels (the canvas starts OVER of the screen above and to the left of it)
    const measure = () => {
      const pw = pinned.clientWidth, ph = pinned.clientHeight;
      const [ox, oy] = offsetIn(stage.current!, pinned);
      geom.current = { u, sx: ox + OVER * pw, sy: oy + OVER * ph, pw, ph };
      mm.resize(pw * (1 + 2 * OVER), ph * (1 + 2 * OVER), window.devicePixelRatio || 1);
      // the nebula's heart is the stage's centre, and the canvas turns about it too
      const cx = geom.current.sx + (g.W / 2) * u, cy = geom.current.sy + (g.H / 2) * u;
      mm.setFocus(cx / (pw * (1 + 2 * OVER)), cy / (ph * (1 + 2 * OVER)), g.reach * u);
      cv.style.transformOrigin = `${cx}px ${cy}px`;
    };
    const frames = (): FrameSpec[] => SHOTS.map((_, i) => {
      const img = imgs.current[i];
      const colors = img && img.complete && img.naturalWidth ? sampleImage(img, cols, rows) : null;
      const [arm, r] = g.spots[i];
      return { arm, r: r * u, w: g.FW * u, h: g.FH * u, cols, rows, colors, order: i, out: !isKept(i) };
    });
    measure();
    mm.setFrames(frames());
    mm.setProgress(scrollYProgress.get());
    mm.onTick = (p, poses) => { follow(p); ride(poses); };
    mm.start();
    // the pixels arrive when the photographs have loaded; until then the motes carry their own light
    Promise.all(SHOTS.map((_, i) => imgs.current[i]?.decode?.().catch(() => undefined))).then(() => { if (motes.current === mm) mm.setFrames(frames()); });
    const ro = new ResizeObserver(() => { measure(); mm.setFrames(frames()); });
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
    const m = motes.current;
    if (!m || reduce) return;
    const { u, sx, sy } = geom.current;
    const fin = g.final[i], pose = posesRef.current[i];
    let cx: number, cy: number;
    if (phaseRef.current >= 3 && fin) { cx = sx + (fin[0] + g.FW / 2) * u; cy = sy + (fin[1] + g.FH / 2) * u; }
    else if (pose) { cx = pose.x; cy = pose.y; }
    else return;
    m.burst(cx + (g.FW / 2 - 14) * scale * u, cy - (g.FH / 2 - 14) * scale * u, 18);
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
    const id = setTimeout(() => burstAt(BEST, g.final[BEST][2]), 900);
    return () => clearTimeout(id);
  }, [bested]); // eslint-disable-line react-hooks/exhaustive-deps

  const onMove = (e: React.PointerEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - r.left, y = e.clientY - r.top;
    motes.current?.pointer(x + OVER * r.width, y + OVER * r.height);
    if (reduce) return;
    const nx = (x / r.width - 0.5) * 2, ny = (y / r.height - 0.5) * 2;
    e.currentTarget.style.setProperty("--ry", `${nx * 6}deg`);
    e.currentTarget.style.setProperty("--rx", `${ny * -4}deg`);
  };
  const onLeave = (e: React.PointerEvent<HTMLElement>) => { motes.current?.pointerOut(); e.currentTarget.style.setProperty("--ry", "0deg"); e.currentTarget.style.setProperty("--rx", "0deg"); };
  // a touch or click anywhere on the first screen lets a few motes go from that point
  const onDown = (e: React.PointerEvent<HTMLElement>) => {
    if ((e.target as HTMLElement).closest("a, button")) return;
    const r = e.currentTarget.getBoundingClientRect();
    motes.current?.burst(e.clientX - r.left + OVER * r.width, e.clientY - r.top + OVER * r.height);
  };

  const stageStyle: CSSProperties = { width: g.W, height: g.H, transform: `scale(${u || 1})`, transformOrigin: "0 0", opacity: u ? 1 : 0 };
  const developed = reduce ? 1 : dev;
  const pinStyle = { "--lean": reduce || bested ? "0deg" : `${LEAN}deg`, perspective: 1400 } as CSSProperties;

  return (
    <section ref={track} className="relative" style={{ height: reduce ? "auto" : g.travel }}>
      <div ref={pin} className="sticky top-16 flex h-[calc(100dvh-4rem)] items-center overflow-hidden" style={pinStyle} onPointerMove={onMove} onPointerLeave={onLeave} onPointerDown={onDown}>
        {/* The nebula, in the same leaning plane as the photographs; it runs past the screen so the lean never shows an edge. */}
        {!reduce && <canvas ref={canvas} aria-hidden className="hero-sea hero-tilt pointer-events-none absolute z-0" style={{ left: `${-OVER * 100}%`, top: `${-OVER * 100}%`, width: `${100 + 2 * OVER * 100}%`, height: `${100 + 2 * OVER * 100}%` }} />}

        <div className="relative z-10 mx-auto grid w-full max-w-[1400px] items-center gap-x-10 gap-y-6 px-5 py-6 md:grid-cols-[minmax(0,5.2fr)_minmax(0,6.8fr)] md:gap-x-14 md:py-10">
          <div className="relative z-10">{children(phase)}</div>

          <div ref={box} className="relative mx-auto w-full max-w-[420px] md:max-w-none" style={{ height: g.H * (u || 1) }}>
            <div aria-hidden className="pointer-events-none absolute left-1/2 top-1/2 -z-10 h-[62%] w-[70%] -translate-x-1/2 -translate-y-1/2 rounded-full opacity-70 blur-3xl"
              style={{ background: "radial-gradient(closest-side, color-mix(in srgb, var(--gleam) 22%, transparent), transparent)" }} />
            <div ref={stage} style={stageStyle}>
              <div className="hero-tilt relative h-full w-full">
                {SHOTS.map((src, i) => {
                  const kept = isKept(i), best = i === BEST;
                  const [gx, gy] = g.grid[i];
                  const fin = g.final[i];
                  // its resting place on the arm until the best is chosen, then its final place (turned-down frames are gone by then)
                  const to = bested && fin ? { x: fin[0] - gx, y: fin[1] - gy, scale: fin[2] } : { x: 0, y: 0, scale: 1 };
                  const out = chosen && !kept;
                  return (
                    <div key={src} ref={(el) => { rides.current[i] = el; }} className="absolute will-change-transform" style={{ left: gx, top: gy, width: g.FW, height: g.FH, zIndex: best ? 3 : kept ? 2 : 1, pointerEvents: out ? "none" : "auto" }}>
                      <motion.div className="relative h-full w-full" initial={false}
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
                    </div>
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
