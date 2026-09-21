import { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { Star } from "../components/Tile";
import { EASE_OUT, SPRING } from "../motion";
import { useLarge } from "../largeSrc";
import { Decision, Photo } from "../types";

const ITEM = 68; // thumbnail width + gap

const PAD = 20;     // empty space at both ends of the filmstrip (the paging arrows sit outside it, in their own columns)
const EASE_IO = [0.65, 0, 0.35, 1] as const;

/**
 * Depth page-turn. The old photo swings away on one side, turned slightly like a page and shrinking into the distance; the new one
 * swings in from the other side and squares up to the viewer. Inside each frame the picture itself drifts the other way (parallax),
 * so the two photos are clearly two objects passing each other, whatever their proportions.
 */
const slide = {
  enter: (d: number) => ({ x: `${108 * d}%`, scale: 0.88, rotateY: -22 * d, opacity: 1 }),
  center: { x: "0%", scale: 1, rotateY: 0, opacity: 1, transition: { duration: 0.62, ease: EASE_IO } },
  exit: (d: number) => ({ x: `${-108 * d}%`, scale: 0.86, rotateY: 22 * d, opacity: 0.35, transition: { duration: 0.62, ease: EASE_IO } }),
};
const inner = {
  enter: (d: number) => ({ x: `${-7 * d}%`, scale: 1.16 }),
  center: { x: "0%", scale: 1, transition: { duration: 0.62, ease: EASE_IO } },
  exit: (d: number) => ({ x: `${7 * d}%`, scale: 1.16, transition: { duration: 0.62, ease: EASE_IO } }),
};

/**
 * One photo at a time: the picture as large as the window allows and a filmstrip below. The numbers and the verdict are in the shared
 * inspector on the right, the same one the wall uses. Left / right (or the arrows) step through; P / R / X decide.
 */
export default function Single({ photos, selected, decisions, onSelect, onStep, onOpen }: {
  photos: Photo[]; selected: string | null; decisions: Map<string, Decision>;
  onSelect: (path: string) => void; onStep: (d: number) => void; onOpen: (path: string) => void;
}) {
  const { t } = useTranslation();
  const idx = Math.max(0, photos.findIndex((p) => p.path === selected));
  const photo = photos[idx];
  // The filmstrip is windowed: with thousands of photos only the thumbnails near the visible part exist.
  const prev = useRef(idx);
  const dir = idx >= prev.current ? 1 : -1;
  useEffect(() => { prev.current = idx; }, [idx]);
  const large = useLarge(photo?.path ?? "");
  const [full, setFull] = useState<string | null>(null);
  useEffect(() => { setFull(null); }, [photo?.path]);
  const strip = useRef<HTMLDivElement>(null);
  const [win, setWin] = useState({ left: 0, width: 800 });
  const measure = useCallback(() => {
    const el = strip.current;
    if (el) setWin((w) => (Math.abs(w.left - el.scrollLeft) < ITEM * 2 && w.width === el.clientWidth ? w : { left: el.scrollLeft, width: el.clientWidth }));
  }, []);
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [measure]);
  useEffect(() => {
    const el = strip.current;
    if (el) el.scrollTo({ left: PAD + idx * ITEM + ITEM / 2 - el.clientWidth / 2, behavior: "smooth" });
  }, [idx]);
  /** Turns the filmstrip by one visible page (a little less, so the last thumbnail stays as an anchor). */
  const page = (dir: number) => strip.current?.scrollBy({ left: dir * Math.max(ITEM * 3, (strip.current.clientWidth - PAD * 2) - ITEM), behavior: "smooth" });
  if (!photo) return null;

  return (
    <div className="single">
      <AnimatePresence initial={false}>
        <motion.img key={photo.path} className="sglow" src={convertFileSrc(photo.thumb)} alt="" aria-hidden draggable={false}
          initial={{ opacity: 0 }} animate={{ opacity: 0.5 }} exit={{ opacity: 0 }} transition={{ duration: 0.7, ease: EASE_OUT }} />
      </AnimatePresence>
      <div className="single-main">
        <button className="sarrow left" onClick={() => onStep(-1)} disabled={idx === 0} aria-label="←">
          <svg width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M10 3L5 8l5 5" /></svg>
        </button>
        <div className="sstage">
          <AnimatePresence initial={false} custom={dir} mode="popLayout">
            <motion.div key={photo.path} className="simg" custom={dir} variants={slide} initial="enter" animate="center" exit="exit">
              <div className="sfit" style={{ ["--r" as string]: photo.width / photo.height }} onDoubleClick={() => onOpen(photo.path)} title={t("inspector.zoomHint")}>
                <motion.div className="sinner" custom={dir} variants={inner}>
                  <img src={convertFileSrc(photo.thumb)} alt="" draggable={false} />
                  {large && <img className={`hi ${full === photo.path ? "in" : ""}`} src={large} alt="" draggable={false} onLoad={() => setFull(photo.path)} />}
                </motion.div>
                <button className="szoom" aria-label={t("inspector.zoomLabel")} onClick={() => onOpen(photo.path)} onDoubleClick={(e) => e.stopPropagation()}>
                  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9" /></svg>
                </button>
              </div>
            </motion.div>
          </AnimatePresence>
          <span className="scount mono">{idx + 1} / {photos.length}</span>
        </div>
        <button className="sarrow right" onClick={() => onStep(1)} disabled={idx === photos.length - 1} aria-label="→">
          <svg width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M6 3l5 5-5 5" /></svg>
        </button>

      </div>

      <div className="stripwrap">
        <button className="strip-arrow left" onClick={() => page(-1)} disabled={win.left <= 2} aria-label={t("single.pagePrev")} data-tip={t("single.pagePrev")}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M10 3L5 8l5 5" /></svg>
        </button>
        <div className="strip" ref={strip} onScroll={measure}>
          <div className="strip-in" style={{ width: photos.length * ITEM }}>
            <motion.i className="sind" initial={false} animate={{ x: idx * ITEM + 31 - 8 }} transition={SPRING.glide} />
            {(() => {
              const from = Math.max(0, Math.floor((win.left - PAD) / ITEM) - 8), to = Math.min(photos.length, Math.ceil((win.left + win.width - PAD) / ITEM) + 8);
              return photos.slice(from, to).map((p, k) => {
                const i = from + k;
                const st = decisions.get(p.path)?.state;
                return (
                  <button key={p.path} className={`sthumb ${i === idx ? "cur" : ""} ${st ?? ""}`} style={{ left: i * ITEM }} onClick={() => onSelect(p.path)}>
                    <img src={convertFileSrc(p.thumb)} alt="" decoding="async" draggable={false} />
                    {st === "keep" && <i className="sdot"><Star filled size={9} /></i>}
                  </button>
                );
              });
            })()}
          </div>
        </div>
        <button className="strip-arrow right" onClick={() => page(1)} disabled={strip.current ? win.left + win.width >= PAD * 2 + photos.length * ITEM - 2 : false} aria-label={t("single.pageNext")} data-tip={t("single.pageNext")}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M6 3l5 5-5 5" /></svg>
        </button>
      </div>
    </div>
  );
}
