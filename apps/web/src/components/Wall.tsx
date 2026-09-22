import { useEffect, useRef, useState } from "react";
import { Check } from "@phosphor-icons/react";
import { copy, type Lang } from "../content";

/**
 * The app's own photo wall, alive in the page. Not a picture of the product and not a drawing of one: these are real tiles in
 * justified rows, carrying the marks the app really uses, and scrolling runs the pass across them. Frames 8 to 10 are three takes
 * of one moment, and only one of the three survives, which is the whole job in miniature.
 *
 * Everything animates from a single custom property written once per frame, so sixteen tiles cost no React renders at all.
 */

type Tile = { i: number; a: number; kept: boolean };

/** Aspect ratio per frame, in reading order. The three takes of one moment sit together, as the app groups them. */
const TILES: Tile[] = [
  { i: 1, a: 1.5, kept: true },
  { i: 2, a: 1.5, kept: false },
  { i: 3, a: 1.5, kept: true },
  { i: 4, a: 1.0, kept: false },
  { i: 5, a: 1.5, kept: false },
  { i: 6, a: 0.667, kept: true },
  { i: 7, a: 1.5, kept: false },
  { i: 8, a: 1.5, kept: false },
  { i: 9, a: 1.5, kept: false },
  { i: 10, a: 1.5, kept: true },
  { i: 11, a: 1.5, kept: false },
  { i: 12, a: 1.5, kept: false },
  { i: 13, a: 0.667, kept: true },
  { i: 14, a: 1.5, kept: false },
  { i: 15, a: 0.667, kept: true },
];

/** A phone shows the first six frames: the whole wall would be taller than the screen, and a cropped wall reads as a mistake. */
const PHONE = 6;
const pad = (n: number) => String(n).padStart(2, "0");

export default function Wall({ lang, progress }: { lang: Lang; progress: () => number }) {
  const c = copy[lang].wall;
  const host = useRef<HTMLDivElement>(null);
  const readout = useRef<HTMLSpanElement>(null);
  const [phone, setPhone] = useState<boolean | null>(null);
  useEffect(() => {
    const m = window.matchMedia("(max-width: 640px)");
    const read = () => setPhone(m.matches);
    read();
    m.addEventListener("change", read);
    return () => m.removeEventListener("change", read);
  }, []);
  const tiles = phone ? TILES.slice(0, PHONE) : TILES;
  const total = tiles.length;
  const kept = tiles.filter((t) => t.kept).length;

  useEffect(() => {
    const el = host.current;
    if (!el || phone === null) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      // The verdicts are already in: the reader sees the result rather than the performance.
      el.style.setProperty("--p", "1");
      if (readout.current) readout.current.textContent = String(kept);
      return;
    }

    let raf = 0, at = 0, seen = -1, live = true;
    const tick = () => {
      raf = 0;
      if (!live) return;
      // Eased towards the scroll position, so a flick of the wheel still lands softly.
      at += (progress() - at) * 0.12;
      el.style.setProperty("--p", at.toFixed(4));
      const done = tiles.slice(0, Math.round(at * total)).filter((t) => t.kept).length;
      if (done !== seen && readout.current) {
        readout.current.textContent = String(done);
        seen = done;
      }
      raf = requestAnimationFrame(tick);
    };
    const io = new IntersectionObserver(([e]) => {
      if (e.isIntersecting && !raf) raf = requestAnimationFrame(tick);
      else if (!e.isIntersecting && raf) { cancelAnimationFrame(raf); raf = 0; }
    }, { threshold: 0 });
    io.observe(el);

    return () => {
      live = false;
      if (raf) cancelAnimationFrame(raf);
      io.disconnect();
    };
  }, [progress, phone, total, kept, tiles]);

  return (
    <div ref={host} className="wall-live" style={{ ["--n" as string]: total }}>
      {/* The app's own progress line, with the glint it carries on its leading edge. */}
      <div className="mb-3 flex items-baseline justify-between gap-4">
        <span className="text-xs tabular-nums text-ash">
          <span ref={readout}>0</span> / {total} {c.kept}
        </span>
        <span className="hidden text-xs text-ash/70 sm:inline">{c.note}</span>
      </div>
      <div className="wall-bar" aria-hidden><i /></div>

      <div className="wall-rows">
        {tiles.map((t, k) => (
          <figure key={t.i} className={`wall-tile ${t.kept ? "keep" : "out"}`}
            style={{ ["--a" as string]: t.a, ["--i" as string]: k }}>
            <img src={`/wall/${pad(t.i)}.webp`} alt="" width={600} height={400} draggable={false}
              loading={k < 6 ? "eager" : "lazy"} fetchPriority={k < 3 ? "high" : "auto"} decoding="async" />
            {t.kept
              ? <s className="wall-check" aria-hidden><Check size={12} weight="bold" /></s>
              : <s className="wall-out" aria-hidden>{c.reject}</s>}
          </figure>
        ))}
      </div>
    </div>
  );
}
