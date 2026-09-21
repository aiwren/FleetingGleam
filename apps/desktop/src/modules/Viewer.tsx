import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { motion, useAnimate, usePresence } from "motion/react";
import { useTranslation } from "react-i18next";
import { DUR, EASE_OUT } from "../motion";
import { useLarge } from "../largeSrc";
import { Decision, Photo, State, pct } from "../types";

type Props = {
  photo: Photo;
  decision?: Decision;
  index: number;
  total: number;
  onClose: () => void;
  onStep: (d: number) => void;
  onDecide: (s: State) => void;
  /** Where the photo's tile was on screen when the viewer opened, so the photo can fly out of it. */
  from: { x: number; y: number; width: number; height: number } | null;
};

/**
 * Lightbox. Opening flies the photo out of its tile (shared layoutId); a blurred copy of the photo
 * lights the room behind it; stepping slides the next photo in from the direction of travel.
 */
export default function Viewer({ photo, decision, index, total, onClose, onStep, onDecide, from }: Props) {
  const { t } = useTranslation();
  const large = useLarge(photo.path);
  const [scale, setScale] = useState(1);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const [loaded, setLoaded] = useState(false);
  const [dragging, setDragging] = useState(false);
  const drag = useRef<{ x: number; y: number; px: number; py: number } | null>(null);
  const origin = useRef(photo.path);
  const start = useRef(from);
  const [scope, animate] = useAnimate();
  const [isPresent, safeToRemove] = usePresence();
  const prevIndex = useRef(index);
  const dir = index >= prevIndex.current ? 1 : -1;
  useEffect(() => { prevIndex.current = index; }, [index]);

  // The view is (scale, pan). Keeping a mirror in a ref lets rapid wheel events build on each other instead of on a stale render.
  const view = useRef({ scale: 1, x: 0, y: 0 });
  const apply = (s: number, x: number, y: number) => {
    // The photo may not be dragged off the stage: at most half of the extra size on each side.
    const box = (scope.current as HTMLElement | null)?.getBoundingClientRect();
    const lx = box ? ((s - 1) * box.width) / 2 : 0, ly = box ? ((s - 1) * box.height) / 2 : 0;
    const nx = s <= 1 ? 0 : Math.max(-lx, Math.min(lx, x)), ny = s <= 1 ? 0 : Math.max(-ly, Math.min(ly, y));
    view.current = { scale: s, x: nx, y: ny };
    setScale(s);
    setPos({ x: nx, y: ny });
  };
  // Eased only for the discrete jumps (double-click, keys, reset); wheel ticks apply at once so they never restart a transition mid-flight.
  const [glide, setGlide] = useState(false);
  const glideTimer = useRef<number | undefined>(undefined);
  const ease = () => { setGlide(true); window.clearTimeout(glideTimer.current); glideTimer.current = window.setTimeout(() => setGlide(false), 320); };
  const reset = () => { ease(); apply(1, 0, 0); };
  /** Zooms by `f` keeping the point under (clientX, clientY) fixed; without a point it zooms around the centre. */
  const zoomAt = (target: number, clientX?: number, clientY?: number) => {
    const { scale: s, x, y } = view.current;
    const n = Math.min(10, Math.max(1, target));
    const box = (scope.current as HTMLElement | null)?.getBoundingClientRect();
    const cx = box && clientX !== undefined ? clientX - (box.left + box.width / 2) : 0;
    const cy = box && clientY !== undefined ? clientY - (box.top + box.height / 2) : 0;
    // A point u of the photo sits at pan + scale * u (from the centre); solve for the pan that leaves it under the cursor.
    apply(n, cx - (n / s) * (cx - x), cy - (n / s) * (cy - y));
  };
  const zoom = (f: number, clientX?: number, clientY?: number) => zoomAt(view.current.scale * f, clientX, clientY);

  useEffect(() => { reset(); setLoaded(false); }, [photo.path]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const k = e.key.toLowerCase();
      if (k === "escape" || k === " ") { e.preventDefault(); onClose(); }
      else if (k === "arrowright") onStep(1);
      else if (k === "arrowleft") onStep(-1);
      else if (k === "+" || k === "=") { ease(); zoom(1.4); }
      else if (k === "-") { ease(); zoom(1 / 1.4); }
      else if (k === "0") reset();
      else if (k === "p") onDecide("keep");
      else if (k === "r") onDecide("review");
      else if (k === "x") onDecide("reject");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, onStep, onDecide]);

  // Fly out of the tile on open...
  useLayoutEffect(() => {
    const el = scope.current as HTMLElement | null, f = start.current;
    if (!el || !f) return;
    const to = el.getBoundingClientRect();
    animate(el, { x: [f.x + f.width / 2 - (to.x + to.width / 2), 0], y: [f.y + f.height / 2 - (to.y + to.height / 2), 0], scaleX: [f.width / to.width, 1], scaleY: [f.height / to.height, 1] },
      { type: "spring", stiffness: 230, damping: 30 });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  // ...and back into whichever tile is now current on close (if that tile is on screen; otherwise just fade).
  useEffect(() => {
    if (isPresent) return;
    const el = scope.current as HTMLElement | null;
    const tile = document.getElementById(`t-${photo.path}`);
    if (!el || !tile) { safeToRemove?.(); return; }
    const t = tile.getBoundingClientRect(), r = el.getBoundingClientRect();
    // Removal must never depend on an animation callback alone: finish on completion or after a fixed time, whichever comes first.
    let done = false;
    const finish = () => { if (!done) { done = true; safeToRemove?.(); } };
    animate(el, { x: t.x + t.width / 2 - (r.x + r.width / 2), y: t.y + t.height / 2 - (r.y + r.height / 2), scaleX: t.width / r.width, scaleY: t.height / r.height }, { type: "spring", stiffness: 260, damping: 32 })
      .then(finish, finish);
    const timer = setTimeout(finish, 650);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPresent]);

  const isOrigin = photo.path === origin.current;
  const btn = (s: State, key: string) => (
    <button className={`btn ${decision?.state === s ? "on" : ""}`} onClick={() => onDecide(s)}>{t(`state.${s}`)} <kbd>{key}</kbd></button>
  );

  return (
    <motion.div className="viewer" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: DUR.base, ease: EASE_OUT }}>
      <motion.img
        key={photo.path} className="ambient" src={convertFileSrc(photo.thumb)} alt="" aria-hidden draggable={false}
        initial={{ opacity: 0 }} animate={{ opacity: 0.42 }} transition={{ duration: 0.9, ease: EASE_OUT }}
      />
      <div className="viewer-bar" data-tauri-drag-region>
        <span className="mono viewer-title">{photo.name}</span>
        <span className="mono ash">{index + 1} / {total} · {Math.round(scale * 100)}%</span>
        <div className="viewer-actions">
          {btn("keep", "P")}{btn("review", "R")}{btn("reject", "X")}
          <button className="btn" onClick={onClose}>{t("viewer.close")} <kbd>Esc</kbd></button>
        </div>
      </div>
      <div
        className="viewer-stage"
        style={{ cursor: scale > 1 ? (dragging ? "grabbing" : "grab") : "zoom-in" }}
        onWheel={(e) => zoom(Math.exp(-e.deltaY * 0.0022), e.clientX, e.clientY)}
        onDoubleClick={(e) => { if (view.current.scale > 1) reset(); else { ease(); zoomAt(3, e.clientX, e.clientY); } }}
        onPointerDown={(e) => { drag.current = { x: e.clientX, y: e.clientY, px: view.current.x, py: view.current.y }; setDragging(true); e.currentTarget.setPointerCapture(e.pointerId); }}
        onPointerMove={(e) => { if (drag.current && view.current.scale > 1) apply(view.current.scale, drag.current.px + e.clientX - drag.current.x, drag.current.py + e.clientY - drag.current.y); }}
        onPointerUp={() => { drag.current = null; setDragging(false); }}
      >
        <motion.div
          key={photo.path}
          ref={scope}
          className="vwrap"
          style={{ ["--r" as string]: photo.width / photo.height }}
          initial={isOrigin ? undefined : { opacity: 0, x: dir * 56, scale: 0.985 }}
          animate={isOrigin ? undefined : { opacity: 1, x: 0, scale: 1 }}
          transition={{ duration: 0.5, ease: EASE_OUT }}
        >
          <div className={`vzoom ${glide && !dragging ? "smooth" : ""}`} style={{ transform: `translate(${pos.x}px, ${pos.y}px) scale(${scale})` }}>
            <img className="vthumb" src={convertFileSrc(photo.thumb)} alt="" draggable={false} />
            {large && <img className="vfull" src={large} alt={photo.name} draggable={false}
              style={{ opacity: loaded ? 1 : 0 }} onLoad={() => setLoaded(true)} />}
          </div>
        </motion.div>
      </div>
      <div className="viewer-foot mono">
        <span>{t("viewer.overall")} {pct(photo.score)}</span>
        <span>{t("viewer.ai")} {photo.ai_score === null ? "—" : pct(photo.ai_score)}</span>
        <span>{t("viewer.tech")} {pct(photo.tech_score)}</span>
        <span>{photo.width} × {photo.height}</span>
        <span className="ash viewer-hint">{t("viewer.hint")}</span>
      </div>
    </motion.div>
  );
}
