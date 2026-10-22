import { isModalOpen } from "../components/Modal";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion, useAnimate, usePresence } from "motion/react";
import { useTranslation } from "react-i18next";
import { DUR, EASE_OUT } from "../motion";
import { useLarge } from "../largeSrc";
import { Decision, Photo, State, pct, thumbSrc } from "../types";
import PhotoTags from "../components/PhotoTags";

type Props = {
  /** false while another page is in front: the keys belong to it then */
  active?: boolean;
  photo: Photo;
  decision?: Decision;
  index: number;
  total: number;
  onClose: () => void;
  onStep: (d: number) => void;
  onDecide: (s: State) => void;
  /** 顺时针（1）或逆时针（-1）转 90°。 */
  onRotate: (dir: 1 | -1) => void;
  /** Every photo in the current view, in order, for the strip along the bottom, and what has been decided about each. */
  photos: Photo[];
  decisions: Map<string, Decision>;
  onJump: (path: string) => void;
  /** Where the photo's tile was on screen when the viewer opened, so the photo can fly out of it. */
  from: { x: number; y: number; width: number; height: number } | null;
};

/**
 * Lightbox. Opening flies the photo out of its tile (shared layoutId); a blurred copy of the photo
 * lights the room behind it; stepping slides the next photo in from the direction of travel.
 */
/** Whether the window can show HDR now (asked once; it only changes when the window moves to another display). */
let hdrSupport: Promise<boolean> | null = null;
const STAGE_MARGIN = 72;
/** Photos of at least this many pixels go through the native layer even without HDR: the web view cannot show them at full detail. */
const BIG_PIXELS = 60e6;
/** Formats that are never HDR here: a plain JPEG without a gain map is flagged at analysis; the others (HEIC, AVIF, RAW...) are asked of the system decoder on opening. */
const SDR_EXT = new Set(["jpg", "jpeg", "jpe", "png", "webp", "gif", "bmp"]);
const isBig = (p: Photo) => p.width * p.height >= BIG_PIXELS;
const maybeHdr = (p: Photo) => p.hdr || !SDR_EXT.has((p.path.split(".").pop() ?? "").toLowerCase());
const hdrSupported = () => (hdrSupport ??= invoke<boolean>("hdr_overlay_supported").catch(() => false));

export default function Viewer({ photo, decision, index, total, onClose, onStep, onDecide, onRotate, photos, decisions, onJump, from, active = true }: Props) {
  const { t } = useTranslation();
  const large = useLarge(photo.path, true, true);
  // The view that is on screen lives in `view` and is painted straight onto the elements once per frame, so zooming and panning never wait for a
  // React render. `scale` and `pos` are that view once it has come to rest (what the sharp-part request and the cursor care about).
  const [scale, setScale] = useState(1);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const [navOn, setNavOn] = useState(false);
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

  // HDR photos are drawn by a native layer over the page (the web view itself would show them as plain SDR): the page only says where the
  // photo is. While it is up, the page's own copy of the photo is hidden.
  const stageRef = useRef<HTMLDivElement>(null);
  const zoomRef = useRef<HTMLDivElement>(null);
  const [nativeHdr, setNativeHdr] = useState(false);
  // The Live menu drops into the photo's area, and a native layer is always above the page: while it is open the layer gives way to it.
  const menuInset = useRef(0);
  const rects = (withMenu = true) => {
    const z = zoomRef.current?.getBoundingClientRect(), st = stageRef.current?.getBoundingClientRect(), box = (scope.current as HTMLElement | null)?.getBoundingClientRect();
    if (!z || !st || !box) return null;
    const inset = withMenu ? menuInset.current : 0;
    const r = (b: { left: number; top: number; width: number; height: number }) => ({ x: b.left, y: b.top, w: b.width, h: b.height });
    // What may be seen of the photo is the photo's own box (the page clips the zoomed picture to it), and never the stage's side margins, where
    // the step arrows are. The Live menu, when open, takes the top of it.
    const x0 = Math.max(box.left, st.left + STAGE_MARGIN), x1 = Math.min(box.right, st.right - STAGE_MARGIN);
    const y0 = Math.max(box.top, st.top) + inset, y1 = Math.min(box.bottom, st.bottom);
    return { img: r(z), clip: { x: x0, y: y0, w: Math.max(0, x1 - x0), h: Math.max(0, y1 - y0) } };
  };
  const nativeRef = useRef(false);
  nativeRef.current = nativeHdr;
  const navRef = useRef<HTMLDivElement>(null);
  const navBoxRef = useRef<HTMLElement>(null);
  const navOnRef = useRef(false);
  const pctRef = useRef<HTMLSpanElement>(null);

  // The view is (scale, pan). `view` is the truth; rapid wheel events build on each other in it.
  const view = useRef({ scale: 1, x: 0, y: 0 });
  /** The part of the photo that is inside its box, as fractions of the photo, worked out from the view alone (nothing is measured). */
  const visibleFrac = () => {
    const { scale: sc, x, y } = view.current;
    const el = scope.current as HTMLElement | null;
    const w = el?.offsetWidth || 1, h = el?.offsetHeight || 1;
    return {
      x0: Math.max(0, 0.5 + (-w / 2 - x) / (w * sc)), x1: Math.min(1, 0.5 + (w / 2 - x) / (w * sc)),
      y0: Math.max(0, 0.5 + (-h / 2 - y) / (h * sc)), y1: Math.min(1, 0.5 + (h / 2 - y) / (h * sc)),
    };
  };
  // The native layer takes the wheel, pinch, drag and double-click over the photo itself (so the picture follows the hand without a round trip
  // through the page) and tells the page the view it ended up with. Each view the page makes itself gets a number, and the layer's answers say
  // which of those it had seen: an answer older than the page's last view is stale and ignored.
  const viewSeq = useRef(0);
  const needSync = useRef(false);
  /** Where the photo's box is, what may be seen of it and the view, for the native layer. */
  const viewArgs = () => {
    const r = rects(), box = (scope.current as HTMLElement | null)?.getBoundingClientRect();
    if (!r || !box) return null;
    const v = view.current;
    return {
      box: { x: box.left, y: box.top, w: box.width, h: box.height }, clip: r.clip, scale: v.scale, x: v.x, y: v.y, seq: viewSeq.current,
      nav: { w: navSize.w, h: navSize.h }, glide: !!zoomRef.current?.classList.contains("smooth"),
    };
  };
  const syncHdr = () => {
    const a = viewArgs();
    if (a) void invoke("hdr_overlay_move", { view: a }).catch(() => { /* no layer */ });
  };
  const paintNavBox = () => {
    const el = navBoxRef.current;
    if (!el) return;
    const f = visibleFrac();
    el.style.left = `${f.x0 * 100}%`; el.style.top = `${f.y0 * 100}%`;
    el.style.width = `${(f.x1 - f.x0) * 100}%`; el.style.height = `${(f.y1 - f.y0) * 100}%`;
  };
  const frame = useRef(0);
  const paintRef = useRef<() => void>(() => {});
  paintRef.current = () => {
    frame.current = 0;
    const { scale: sc, x, y } = view.current;
    const z = zoomRef.current;
    if (!z) return;
    z.style.transform = `translate(${x}px, ${y}px) scale(${sc})`;
    if (pctRef.current) pctRef.current.textContent = `${Math.round(sc * 100)}%`;
    const on = sc > 1.02;
    if (on !== navOnRef.current) { navOnRef.current = on; setNavOn(on); }
    paintNavBox();
    if (nativeRef.current && needSync.current) syncHdr();
    needSync.current = false;
  };
  const queuePaint = () => { if (!frame.current) frame.current = requestAnimationFrame(() => paintRef.current()); };
  // Once the view has rested for a moment it is also put into state.
  const settle = useRef<number | undefined>(undefined);
  const commit = () => {
    window.clearTimeout(settle.current);
    const v = view.current;
    setScale(v.scale);
    setPos((p) => (p.x === v.x && p.y === v.y ? p : { x: v.x, y: v.y }));
  };
  useEffect(() => () => { cancelAnimationFrame(frame.current); frame.current = 0; window.clearTimeout(settle.current); }, []);
  const apply = (sc: number, x: number, y: number) => {
    // The photo may not be dragged off the stage: at most half of the extra size on each side.
    const el = scope.current as HTMLElement | null;
    const lx = el ? ((sc - 1) * el.offsetWidth) / 2 : 0, ly = el ? ((sc - 1) * el.offsetHeight) / 2 : 0;
    const nx = sc <= 1 ? 0 : Math.max(-lx, Math.min(lx, x)), ny = sc <= 1 ? 0 : Math.max(-ly, Math.min(ly, y));
    view.current = { scale: sc, x: nx, y: ny };
    viewSeq.current++;
    needSync.current = true;
    queuePaint();
    window.clearTimeout(settle.current);
    settle.current = window.setTimeout(commit, 70);
  };
  // Eased only for the discrete jumps (double-click, keys, reset); wheel ticks apply at once so they never restart a transition mid-flight.
  const glideTimer = useRef<number | undefined>(undefined);
  const ease = () => { zoomRef.current?.classList.add("smooth"); window.clearTimeout(glideTimer.current); glideTimer.current = window.setTimeout(() => zoomRef.current?.classList.remove("smooth"), 320); };
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

  // Motion photos: the clip inside the file plays over the picture, from the button or the M key.
  const [clip, setClip] = useState<string | null>(null);
  // "Turn Live off": Live photos stop playing by themselves (they can still be played from the menu). Remembered between runs.
  const [liveOn, setLiveOn] = useState(() => { try { return localStorage.getItem("fg.live") !== "0"; } catch { return true; } });
  const toggleLive = () => {
    const v = !liveOn;
    setLiveOn(v);
    try { localStorage.setItem("fg.live", v ? "1" : "0"); } catch { /* storage unavailable */ }
    if (!v) setClip(null);
  };
  const [run, setRun] = useState(0);
  const [liveMenu, setLiveMenu] = useState(false);
  /** Opening a Live photo plays it once by itself, silently; playing it from the menu has sound. */
  const [quiet, setQuiet] = useState(false);
  const startMotion = async (silent = false) => {
    try { const src = convertFileSrc(await invoke<string>("motion_clip", { path: photo.path })); setQuiet(silent); setClip(src); setRun((r) => r + 1); } catch { /* the clip could not be read */ }
  };
  const playMotion = async () => {
    if (!photo.motion) return;
    if (clip) setClip(null); else await startMotion(false);
  };
  const replayMotion = async () => { if (!photo.motion) return; if (clip) { setQuiet(false); setRun((r) => r + 1); } else await startMotion(false); };
  useEffect(() => {
    setClip(null); setLiveMenu(false);
    if (!photo.motion || !liveOn) return;
    // after the photo has settled into place
    const id = window.setTimeout(() => { void startMotion(true); }, 600);
    return () => window.clearTimeout(id);
  }, [photo.path, liveOn]); // eslint-disable-line react-hooks/exhaustive-deps
  const liveRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!liveMenu) return;
    const away = (e: PointerEvent) => { if (!liveRef.current?.contains(e.target as Node)) setLiveMenu(false); };
    window.addEventListener("pointerdown", away);
    return () => window.removeEventListener("pointerdown", away);
  }, [liveMenu]);

  useEffect(() => { reset(); commit(); setLoaded(false); }, [photo.path]);   // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    menuInset.current = liveMenu ? 150 : 0;
    if (nativeHdr) syncHdr();
  }, [liveMenu]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!(maybeHdr(photo) || isBig(photo)) || clip || !isPresent || photo.rotation) return;
    let live = true;
    // let the photo settle first (the fly-out when opening, the slide when stepping)
    const id = window.setTimeout(async () => {
      if (!(await hdrSupported()) || !live) return;
      // wait for the photo to stop moving (the fly-out and the slide are springs), so the layer is first drawn where the photo ends up
      let r = rects();
      for (let i = 0; i < 14 && r && live; i++) {
        await new Promise((ok) => setTimeout(ok, 70));
        const n = rects();
        const still = !!n && Math.abs(n.img.x - r.img.x) < 0.6 && Math.abs(n.img.y - r.img.y) < 0.6 && Math.abs(n.img.w - r.img.w) < 0.6 && Math.abs(n.img.h - r.img.h) < 0.6;
        r = n;
        if (still) break;
      }
      if (!r || !live) return;
      try {
        const view0 = viewArgs();
        if (!view0) return;
        const ok = await invoke<boolean>("hdr_overlay_show", { path: photo.path, requireHdr: !isBig(photo), view: view0 });
        if (!live) void invoke("hdr_overlay_hide", {}).catch(() => { /* no layer */ });
        else if (ok) setNativeHdr(true);
      } catch { /* not in the app, or this photo has no HDR to show */ }
    }, 350);
    return () => { live = false; window.clearTimeout(id); setNativeHdr(false); void invoke("hdr_overlay_hide", {}).catch(() => { /* no layer */ }); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [photo.path, photo.hdr, photo.width, photo.height, clip, isPresent]);
  useEffect(() => () => { void invoke("hdr_overlay_hide", { release: true }).catch(() => { /* no layer */ }); }, []);
  // The native layer moved the photo by hand: take its view over (unless the page has made a newer one since).
  useEffect(() => {
    if (!nativeHdr) return;
    let live = true, off: (() => void) | undefined;
    void listen<{ scale: number; x: number; y: number; ack: number; press: boolean }>("hdr-view", (e) => {
      const p = e.payload;
      if (p.press) setLiveMenu(false);
      if (p.ack !== viewSeq.current) return;
      view.current = { scale: p.scale, x: p.x, y: p.y };
      queuePaint();
      window.clearTimeout(settle.current);
      settle.current = window.setTimeout(commit, 70);
    }).then((f) => { if (live) off = f; else f(); });
    return () => { live = false; off?.(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nativeHdr]);
  // The navigator: while zoomed in, a small map of the whole photo with a frame for the part on screen. Dragging the frame (or clicking the map)
  // moves the view. It sits in the photo's bottom-right corner; the native layer, which covers the page there, draws it itself.
  /** Puts the centre of the view on the given point of the photo (fractions of it). */
  const centreOn = (fx: number, fy: number) => {
    const z = zoomRef.current;
    if (!z) return;
    const s2 = view.current.scale;
    apply(s2, -(fx - 0.5) * z.offsetWidth * s2, -(fy - 0.5) * z.offsetHeight * s2);
  };
  // The map has the photo's own proportions (the frame inside it is in fractions of the photo), within 150 x 110.
  const navAspect = Math.max(0.2, photo.width / Math.max(1, photo.height));
  const navW = Math.min(150, 110 * navAspect), navSize = { w: Math.round(navW), h: Math.round(navW / navAspect) };
  const navDrag = useRef(false);
  // When the map appears, its frame (and the native copy of it) is put in place at once.
  useLayoutEffect(() => { paintNavBox(); if (nativeHdr) syncHdr(); }, [navOn, nativeHdr]); // eslint-disable-line react-hooks/exhaustive-deps
  const navAt = (e: React.PointerEvent<HTMLDivElement>) => {
    const b = e.currentTarget.getBoundingClientRect();
    centreOn(Math.min(1, Math.max(0, (e.clientX - b.left) / b.width)), Math.min(1, Math.max(0, (e.clientY - b.top) / b.height)));
  };

  // The sharp part. A photo larger than what is drawn (the native layer holds at most 4096 px, the web preview 2560) looks soft when zoomed in, so
  // once zoomed far enough the region on screen is decoded from the original at (nearly) its own resolution and drawn over the photo, as the
  // phone's "200 MP preview" does. Asked once the view has rested for a moment; kept while the view stays inside it.
  const [detailSrc, setDetailSrc] = useState<{ src: string; x: number; y: number; w: number; h: number } | null>(null);
  const asked = useRef<{ x0: number; y0: number; x1: number; y1: number; scale: number } | null>(null);
  const detailTok = useRef(0);
  useEffect(() => { setDetailSrc(null); asked.current = null; detailTok.current++; }, [photo.path, photo.rotation]);
  useEffect(() => {
    const long = Math.max(photo.width, photo.height);
    const have = nativeHdr ? (isBig(photo) ? 2048 : Math.min(long, 4096)) : 2560;
    const clear = () => { if (asked.current) { asked.current = null; detailTok.current++; setDetailSrc(null); if (nativeHdr) void invoke("hdr_overlay_detail_hide").catch(() => { /* no layer */ }); } };
    if (long <= have * 1.05 || clip) { clear(); return; }
    const id = window.setTimeout(async () => {
      const r = rects();
      if (!r) return;
      const dpr = window.devicePixelRatio || 1;
      if (Math.max(r.img.w, r.img.h) * dpr <= have * 0.9) { clear(); return; }   // the picture already holds all the pixels the screen shows
      // what is on screen, as fractions of the photo
      const vx0 = Math.max(r.img.x, r.clip.x), vy0 = Math.max(r.img.y, r.clip.y);
      const vx1 = Math.min(r.img.x + r.img.w, r.clip.x + r.clip.w), vy1 = Math.min(r.img.y + r.img.h, r.clip.y + r.clip.h);
      if (vx1 <= vx0 || vy1 <= vy0) return;
      const f = { x0: (vx0 - r.img.x) / r.img.w, y0: (vy0 - r.img.y) / r.img.h, x1: (vx1 - r.img.x) / r.img.w, y1: (vy1 - r.img.y) / r.img.h };
      const a = asked.current;
      if (a && Math.abs(scale / a.scale - 1) < 0.2 && f.x0 >= a.x0 - 0.001 && f.y0 >= a.y0 - 0.001 && f.x1 <= a.x1 + 0.001 && f.y1 <= a.y1 + 0.001) return;   // still inside what was fetched
      // a generous margin around it, so panning around stays inside what is already sharp
      const px = (f.x1 - f.x0) * 0.3, py = (f.y1 - f.y0) * 0.3;
      const g = { x0: Math.max(0, f.x0 - px), y0: Math.max(0, f.y0 - py), x1: Math.min(1, f.x1 + px), y1: Math.min(1, f.y1 + py) };
      const gw = g.x1 - g.x0, gh = g.y1 - g.y0;
      const outW = Math.max(800, Math.min(Math.round(gw * photo.width), Math.round(gw * r.img.w * dpr * 1.1)));
      const tok = ++detailTok.current;
      asked.current = { ...g, scale };
      try {
        if (nativeHdr) {
          await invoke<boolean>("hdr_overlay_detail", { path: photo.path, region: { x: g.x0, y: g.y0, w: gw, h: gh }, outW });
        } else {
          const src = convertFileSrc(await invoke<string>("photo_detail", { path: photo.path, x: g.x0, y: g.y0, w: gw, h: gh, outW }));
          if (tok === detailTok.current) setDetailSrc({ src, x: g.x0, y: g.y0, w: gw, h: gh });
        }
      } catch { if (tok === detailTok.current) asked.current = null; /* could not be decoded: the picture stays as it is */ }
    }, 90);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scale, pos.x, pos.y, nativeHdr, photo.path, photo.width, photo.height, photo.rotation, clip]);

  // follow the photo: while it settles into place, and whenever it is zoomed, panned or the window changes
  useEffect(() => {
    if (!nativeHdr) return;
    let raf = 0;
    const t0 = performance.now();
    const tick = () => { syncHdr(); if (performance.now() - t0 < 900) raf = requestAnimationFrame(tick); };
    tick();
    window.addEventListener("resize", syncHdr);
    return () => { cancelAnimationFrame(raf); window.removeEventListener("resize", syncHdr); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nativeHdr]);

  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if (isModalOpen()) return;
      const k = e.key.toLowerCase();
      if (k === "escape" || k === " ") { e.preventDefault(); onClose(); }
      else if (k === "arrowright") onStep(1);
      else if (k === "arrowleft") onStep(-1);
      else if (k === "+" || k === "=") { ease(); zoom(1.4); }
      else if (k === "-") { ease(); zoom(1 / 1.4); }
      else if (k === "0") reset();
      else if (k === "m") void playMotion();
      else if ((e.metaKey || e.ctrlKey) && (e.key === "]" || e.key === "[")) { e.preventDefault(); onRotate(e.key === "]" ? 1 : -1); }
      else if (k === "p") onDecide("keep");
      else if (k === "r") onDecide("review");
      else if (k === "x") onDecide("reject");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, onStep, onDecide, onRotate, photo.path, photo.motion, clip]); // eslint-disable-line react-hooks/exhaustive-deps

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

  // The strip: fixed-size cells, so only the ones near the current photo are mounted (spacers stand in for the rest) and a long folder stays light.
  const strip = useRef<HTMLDivElement>(null);
  const CELL = 64, GAP = 6, STEP = CELL + GAP, WINDOW = 40;
  const first = Math.max(0, index - WINDOW), last = Math.min(photos.length, index + WINDOW + 1);
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    el.scrollTo({ left: index * STEP + CELL / 2 - el.clientWidth / 2, behavior: "smooth" });
  }, [index]); // eslint-disable-line react-hooks/exhaustive-deps

  const isOrigin = photo.path === origin.current;
  const btn = (s: State, key: string) => (
    <button className={`btn ${decision?.state === s ? "on" : ""}`} onClick={() => onDecide(s)}>{t(`state.${s}`)} <kbd>{key}</kbd></button>
  );

  return (
    <motion.div className={`viewer ${nativeHdr && !liveMenu ? "native-hdr" : ""}`} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: DUR.base, ease: EASE_OUT }}>
      <motion.img
        key={photo.path} className="ambient" src={convertFileSrc(photo.thumb)} alt="" aria-hidden draggable={false}
        initial={{ opacity: 0 }} animate={{ opacity: 0.42 }} transition={{ duration: 0.9, ease: EASE_OUT }}
      />
      <div className="viewer-bar" data-tauri-drag-region>
        <span className="mono viewer-title">{photo.name}</span>
        <PhotoTags photo={photo} skip={["motion"]} />
        {photo.motion && (
          <div className="livepill" ref={liveRef}>
            <button className={`ptag live ${clip ? "playing" : ""} ${liveOn ? "" : "off"}`} aria-haspopup="menu" aria-expanded={liveMenu} onClick={() => setLiveMenu((v) => !v)}>
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.2" aria-hidden><circle cx="6" cy="6" r="1.5" fill="currentColor" stroke="none" /><circle cx="6" cy="6" r="3.2" /><circle cx="6" cy="6" r="5.2" strokeDasharray="1.4 1.4" /></svg>
              {t("tag.motion")}
              <svg width="9" height="9" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d="M2.5 3.8L5 6.3l2.5-2.5" /></svg>
            </button>
            {liveMenu && (
              <div className="livemenu" role="menu">
                <button role="menuitem" onClick={() => { setLiveMenu(false); void playMotion(); }}>{clip ? t("viewer.stopMotion") : t("viewer.playMotion")}</button>
                <button role="menuitem" onClick={() => { setLiveMenu(false); void replayMotion(); }}>{t("viewer.replayMotion")}</button>
                <i className="livesep" aria-hidden />
                <button role="menuitem" onClick={() => { setLiveMenu(false); toggleLive(); }}>{liveOn ? t("viewer.liveOff") : t("viewer.liveOn")}</button>
              </div>
            )}
          </div>
        )}
        <span className="mono ash">{index + 1} / {total} · <span ref={pctRef}>100%</span></span>
        <div className="viewer-actions">
          {btn("keep", "P")}{btn("review", "R")}{btn("reject", "X")}
          <button className="btn" onClick={() => onRotate(1)} title={t("viewer.rotateTip")}>{t("viewer.rotate")} <kbd>⌘]</kbd></button>
          <button className="btn" onClick={onClose}>{t("viewer.close")} <kbd>Esc</kbd></button>
        </div>
      </div>
      <div
        className="viewer-stage" ref={stageRef}
        style={{ cursor: scale > 1 ? (dragging ? "grabbing" : "grab") : "zoom-in" }}
        onWheel={(e) => zoom(Math.exp(-e.deltaY * 0.0022), e.clientX, e.clientY)}
        onDoubleClick={(e) => { if (view.current.scale > 1) reset(); else { ease(); zoomAt(3, e.clientX, e.clientY); } }}
        onPointerDown={(e) => { zoomRef.current?.classList.remove("smooth"); drag.current = { x: e.clientX, y: e.clientY, px: view.current.x, py: view.current.y }; setDragging(true); e.currentTarget.setPointerCapture(e.pointerId); }}
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
          <div ref={zoomRef} className="vzoom">
            <img className="vthumb" src={thumbSrc(photo)} alt="" draggable={false} />
            {detailSrc && <img className="vdetail" src={detailSrc.src} alt="" draggable={false}
              style={{ left: `${detailSrc.x * 100}%`, top: `${detailSrc.y * 100}%`, width: `${detailSrc.w * 100}%`, height: `${detailSrc.h * 100}%` }} />}
            {clip && <video key={run} className="vmotion" src={clip} autoPlay muted={quiet} playsInline onEnded={() => setClip(null)} onClick={() => setClip(null)} />}
            {large && <img className="vfull" src={large} alt={photo.name} draggable={false}
              style={{ opacity: loaded ? 1 : 0 }} onLoad={() => setLoaded(true)} />}
          </div>
          {navOn && (
            <div ref={navRef} className={`vnav ${nativeHdr ? "native" : ""}`} style={{ width: navSize.w, height: navSize.h }}
              aria-label={t("viewer.navigator")} title={t("viewer.navigatorHint")}
              onPointerDown={(e) => { e.stopPropagation(); navDrag.current = true; navAt(e); try { e.currentTarget.setPointerCapture(e.pointerId); } catch { /* the pointer is already gone */ } }}
              onPointerMove={(e) => { if (navDrag.current) navAt(e); }}
              onPointerUp={() => { navDrag.current = false; }}
              onWheel={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()}>
              <img src={thumbSrc(photo)} alt="" draggable={false} />
              <i ref={navBoxRef} className="vnav-box" />
            </div>
          )}
        </motion.div>
        <button className="varrow left" aria-label={t("cmp.prev")} data-tip={`${t("cmp.prev")} · ←`} disabled={index <= 0}
          onPointerDown={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()} onClick={() => onStep(-1)}>
          <svg width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M10 3L5 8l5 5" /></svg>
        </button>
        <button className="varrow right" aria-label={t("cmp.next")} data-tip={`${t("cmp.next")} · →`} disabled={index >= total - 1}
          onPointerDown={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()} onClick={() => onStep(1)}>
          <svg width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M6 3l5 5-5 5" /></svg>
        </button>
      </div>
      <div className="vstrip" ref={strip} role="listbox" aria-label={t("viewer.strip")}>
        <div className="vstrip-in" style={{ width: photos.length * STEP }}>
          {photos.slice(first, last).map((p, i) => {
            const n = first + i, st = decisions.get(p.path)?.state;
            return (
              <button key={p.path} role="option" aria-selected={p.path === photo.path} className={`vcell ${p.path === photo.path ? "cur" : ""} ${st ?? ""}`}
                style={{ left: n * STEP }} title={p.name} onClick={() => onJump(p.path)}>
                <img src={thumbSrc(p)} alt="" loading="lazy" draggable={false} />
                {st === "keep" && <i className="vmark keep" />}
                {st === "reject" && <i className="vmark reject" />}
              </button>
            );
          })}
        </div>
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
