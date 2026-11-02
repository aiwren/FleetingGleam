import { isModalOpen } from "../components/Modal";
import { PointerEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { Star } from "../components/Tile";
import PhotoTags from "../components/PhotoTags";
import { FaceAvatar } from "../components/FilterPanel";
import { useLarge } from "../largeSrc";
import { SPRING } from "../motion";
import { Decision, Photo, State, pct, thumbSrc } from "../types";
import { decisionOf, keyFor, keyVars, useKeymap } from "../keymap";

const GAP = 12;
/** The shared view: zoom factor and pan, the pan as a fraction of a frame's own size so every frame shows the same spot. */
type View = { s: number; x: number; y: number };
const FIT: View = { s: 1, x: 0, y: 0 };

type Metric = { key: string; label: string; get: (p: Photo) => number | null };

function Cell({ p, index, st, manual, view, dragging, metrics, set, count, personFilter, onPerson, onDecide, onOpen, onHover, zoomAt, pan, endPan, resetOrZoom }: {
  p: Photo; index: number; st: State; manual: boolean; view: View; dragging: boolean; metrics: Metric[]; set: Photo[]; count: number;
  personFilter: number | null; onPerson: (id: number | null) => void;
  onDecide: (s: State | null) => void; onOpen: () => void; onHover: (path: string | null) => void;
  zoomAt: (e: { clientX: number; clientY: number; currentTarget: HTMLElement }, factor: number) => void;
  pan: (e: PointerEvent<HTMLElement>, phase: "down" | "move") => void; endPan: () => void;
  resetOrZoom: (e: { clientX: number; clientY: number; currentTarget: HTMLElement }) => void;
}) {
  const { t } = useTranslation();
  const km = useKeymap(); // 保留 / 淘汰按钮提示里显示的键
  // The big preview is prepared shortly after the photo is shown, so zooming in finds it ready; the thumbnail stays until it has loaded.
  const [warm, setWarm] = useState(false);
  useEffect(() => { setWarm(false); const id = setTimeout(() => setWarm(true), 500); return () => clearTimeout(id); }, [p.path]);
  const large = useLarge(p.path, warm || view.s > 1.02);
  const [sharp, setSharp] = useState(false);
  useEffect(() => { setSharp(false); }, [p.path]);
  // Zoom changes the picture's real size and position (not a scaled bitmap), so the browser keeps it crisp instead of stretching a raster.
  const img = { width: `${view.s * 100}%`, height: `${view.s * 100}%`, left: `${(0.5 - view.s / 2 + view.x) * 100}%`, top: `${(0.5 - view.s / 2 + view.y) * 100}%` };
  return (
    <div className="cmp2-cell">
      <div className="cmp2-stack" style={{ ["--r" as string]: p.width / p.height }}>
      <motion.div className={`cmp2-fit ${st}`}
        initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: index * 0.03 }}
        onWheel={(e) => zoomAt(e, Math.exp(-e.deltaY * 0.0022))}
        onPointerDown={(e) => pan(e, "down")} onPointerMove={(e) => pan(e, "move")} onPointerUp={() => endPan()} onPointerEnter={() => onHover(p.path)} onPointerLeave={() => onHover(null)}
        onDoubleClick={(e) => resetOrZoom(e)}>
        <div className={`cmp2-img ${dragging ? "" : "smooth"}`} style={img}>
          <img src={thumbSrc(p)} alt="" draggable={false} />
          {large && <img className={`hi ${sharp ? "in" : ""}`} src={large} alt="" draggable={false} onLoad={() => setSharp(true)} />}
        </div>

        <div className="cmp2-tags">
          <PhotoTags photo={p} />
          {/* "推荐"只用文字：四角星只表示保留（R2） */}
          {p.group_best && count > 1 && <span className="cmp2-tag best">{t("cmp.best")}</span>}
          {st === "keep" && <span className="cmp2-tag kept"><Star filled size={11} />{t("cmp.kept")}</span>}
          {st === "reject" && <span className="cmp2-tag out">{t("cmp.rejected")}</span>}
        </div>
        <div className="cmp2-top">
          {view.s > 1.02 && <span className="cmp2-tag mono">{view.s.toFixed(1)}×</span>}
          <button className="cmp2-round" aria-label={t("inspector.zoomLabel")} data-tip={t("inspector.zoomLabel")} onPointerDown={(e) => e.stopPropagation()} onPointerUp={(e) => e.stopPropagation()} onClick={onOpen}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9" /></svg>
          </button>
        </div>

        <div className="cmp2-foot" onPointerDown={(e) => e.stopPropagation()} onPointerUp={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()}>
          <div className="cmp2-info">
            <div className="cmp2-head">
              <span className="cmp2-key mono">{index + 1}</span>
              {(() => {
                // Only when the picture has two or more recognised people: with one there is nothing to tell apart.
                const ids = [...new Set(p.people.filter((id) => id >= 0))];
                if (ids.length < 2) return null;
                return (
                  <span className="cmp2-who">
                    {ids.slice(0, 4).map((id) => {
                      const box = p.face_boxes[p.people.indexOf(id)];
                      return box && (
                        <button key={id} className={personFilter === id ? "on" : ""} aria-label={t("cull.people")} data-tip={t("cmp.onlyThem")}
                          onPointerDown={(e) => e.stopPropagation()} onClick={() => onPerson(personFilter === id ? null : id)}>
                          <FaceAvatar photo={p} box={box} size={22} />
                        </button>
                      );
                    })}
                  </span>
                );
              })()}
            </div>
            <div className="cmp2-chips">
              {metrics.map((m) => {
                const v = m.get(p);
                const vals = set.map(m.get).filter((x): x is number => x !== null);
                const top = v !== null && vals.length > 1 && v === Math.max(...vals) && v > Math.min(...vals);
                return <span key={m.key} className={`cmp2-chip ${top ? "top" : ""} ${m.key === "score" ? "main" : ""}`}>{m.label}<b className="mono">{v ?? "—"}</b></span>;
              })}
            </div>
          </div>
          <div className="cmp2-act">
            <button className={`cmp2-decide keep ${st === "keep" ? "on" : ""}`} aria-label={t("state.keep")} data-tip={`${t("state.keep")} · ${keyFor("keep", km)}`} onClick={() => onDecide(st === "keep" && manual ? null : "keep")}><Star filled size={17} /></button>
            <button className={`cmp2-decide reject ${st === "reject" ? "on" : ""}`} aria-label={t("state.reject")} data-tip={`${t("state.reject")} · ${keyFor("reject", km)}`} onClick={() => onDecide(st === "reject" && manual ? null : "reject")}>
              <svg width="13" height="13" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" aria-hidden><path d="M3 3l6 6M9 3L3 9" /></svg>
            </button>
          </div>
        </div>
      </motion.div>
      </div>
    </div>
  );
}

/**
 * Pick the best of a burst. The photos fill the whole area, as large as they can be, side by side; wheel, drag and double-click zoom and
 * pan all of them together, so the same detail (an eye, a hand) can be checked in every frame. The numbers sit on each photo.
 * Left / right step through groups of similar shots, the keys 1-4 choose a photo, the decision keys (keymap.ts; P / X by default) decide.
 */
export default function Compare({ photos, all, scope: scopeProp, personFilter, onPerson, selected, decisions, onSelect, onDecide, onOpen, enabled = true, showHint = true }: {
  photos: Photo[]; all: Photo[];
  /** What counts as "the same shot": a burst of look-alikes, or (when a scene is picked in the filter) the whole scene. */
  scope: "group" | "scene";
  personFilter: number | null; onPerson: (id: number | null) => void; selected: string | null; decisions: Map<string, Decision>;
  onSelect: (path: string) => void; onDecide: (path: string, s: State | null) => void; onOpen: (path: string) => void;
  /** false while another page is in front or a viewer / dialog is open: the keys belong to them then */
  enabled?: boolean;
  /** 操作提示只在前几次显示（M6.5）。 */
  showHint?: boolean;
}) {
  const { t } = useTranslation();
  const km = useKeymap(); // 操作提示里显示的键
  const cur = photos.find((p) => p.path === selected) ?? photos[0];
  // The scope follows the scene filter, and can also be switched here: "scene" steps through whole scenes instead of bursts.
  const [scope, setScope] = useState(scopeProp);
  useEffect(() => setScope(scopeProp), [scopeProp]);
  const key = useCallback((p: Photo) => (scope === "scene" ? p.scene : p.group), [scope]);

  const members = useMemo(() => {
    const m = new Map<number, Photo[]>();
    for (const p of all) { const k = key(p); const g = m.get(k); if (g) g.push(p); else m.set(k, [p]); }
    return m;
  }, [all, key]);
  const groups = useMemo(() => {
    const ids: number[] = [];
    for (const p of photos) { const k = key(p); if ((members.get(k)?.length ?? 0) > 1 && !ids.includes(k)) ids.push(k); }
    return ids;
  }, [photos, members, key]);
  const gi = cur ? groups.indexOf(key(cur)) : -1;
  const ranked = useCallback((id: number) => [...(members.get(id) ?? [])].sort((a, b) => b.score - a.score), [members]);
  // Base photo (the selected one) plus challengers from the same group, best first. Two side by side is the default because it gives each
  // photo the most room; 3 and 4 are one click away.
  const [count, setCount] = useState<number>(() => { try { const v = Number(localStorage.getItem("fg.cmpn")); return v >= 2 && v <= 4 ? v : 2; } catch { return 2; } });
  const pickCount = (n: number) => { setCount(n); try { localStorage.setItem("fg.cmpn", String(n)); } catch { /* storage unavailable */ } };
  // The photos on screen live in fixed slots and nothing about them changes when you touch one. Only the last slot (the challenger)
  // is ever replaced, by the rail or by the up/down keys. There is no notion of a selected photo here: each photo has its own buttons.
  const groupPhotos = useMemo(() => (gi >= 0 && cur ? ranked(key(cur)) : []), [gi, cur?.group, cur?.scene, ranked, key]); // eslint-disable-line react-hooks/exhaustive-deps
  const shownN = Math.min(count, groupPhotos.length);
  const [slots, setSlots] = useState<string[]>([]);
  useEffect(() => {
    setSlots((prev) => {
      const ids = groupPhotos.map((p) => p.path);
      const next = prev.filter((x) => ids.includes(x)).slice(0, shownN);
      for (const id of ids) if (next.length < shownN && !next.includes(id)) next.push(id);
      if (cur && ids.includes(cur.path) && !next.includes(cur.path)) next[next.length - 1] = cur.path;
      return next.length === prev.length && next.every((x, i) => x === prev[i]) ? prev : next;
    });
  }, [cur?.path, cur?.group, cur?.scene, shownN, groupPhotos]); // eslint-disable-line react-hooks/exhaustive-deps
  const set = useMemo(() => {
    const byPath = new Map(groupPhotos.map((p) => [p.path, p]));
    const list = slots.map((id) => byPath.get(id)).filter((p): p is Photo => !!p);
    return list.length ? list : groupPhotos.slice(0, shownN);
  }, [slots, groupPhotos, shownN]);
  const candidates = useMemo(() => groupPhotos.filter((p) => !set.some((x) => x.path === p.path)), [groupPhotos, set]);
  /** Swaps a photo into the last slot. */
  const challenge = useCallback((path: string, from?: number) => {
    setSlots((prev) => {
      const next = [...prev];
      next[from ?? prev.length - 1] = path;
      return next;
    });
  }, []);
  const cycle = useCallback((dir: number) => {
    const ids = groupPhotos.map((p) => p.path);
    const i = slots.length - 1;
    const from = ids.indexOf(slots[i]);
    for (let k = 1; k <= ids.length; k++) {
      const id = ids[(from + dir * k + ids.length * k) % ids.length];
      if (!slots.includes(id)) { challenge(id, i); return; }
    }
  }, [groupPhotos, slots, challenge]);
  const jump = useCallback((d: number) => {
    const from = gi >= 0 ? gi : d > 0 ? -1 : groups.length;
    const id = groups[Math.min(groups.length - 1, Math.max(0, from + d))];
    if (id !== undefined) onSelect(ranked(id)[0].path);
  }, [gi, groups, ranked, onSelect]);

  // Shared zoom / pan. A ref mirrors the state so quick wheel events build on each other.
  const [view, setView] = useState<View>(FIT);
  const viewRef = useRef<View>(FIT);
  const [dragging, setDragging] = useState(false);
  // Eased only for the discrete jumps (double-click, reset); a wheel tick is applied at once, since restarting a transition on every
  // tick makes the picture jitter and re-rasterise, which showed up as flicker.
  const [glide, setGlide] = useState(false);
  const glideTimer = useRef<number | undefined>(undefined);
  const ease = useCallback(() => { setGlide(true); window.clearTimeout(glideTimer.current); glideTimer.current = window.setTimeout(() => setGlide(false), 260); }, []);
  const apply = useCallback((v: View) => {
    const lim = Math.max(0, (v.s - 1) / 2);
    const n = { s: v.s, x: v.s <= 1 ? 0 : Math.max(-lim, Math.min(lim, v.x)), y: v.s <= 1 ? 0 : Math.max(-lim, Math.min(lim, v.y)) };
    viewRef.current = n;
    setView(n);
  }, []);
  useEffect(() => { apply(FIT); }, [cur?.group, cur?.scene, apply]);

  const zoomAt = useCallback((e: { clientX: number; clientY: number; currentTarget: HTMLElement }, factor: number, absolute?: number) => {
    const r = e.currentTarget.getBoundingClientRect();
    const cx = (e.clientX - (r.left + r.width / 2)) / r.width, cy = (e.clientY - (r.top + r.height / 2)) / r.height;
    const { s, x, y } = viewRef.current;
    const n = Math.min(12, Math.max(1, absolute ?? s * factor));
    // the point of the photo under the cursor stays under the cursor
    apply({ s: n, x: cx - (n / s) * (cx - x), y: cy - (n / s) * (cy - y) });
  }, [apply]);
  const resetOrZoom = useCallback((e: { clientX: number; clientY: number; currentTarget: HTMLElement }) => {
    ease();
    if (viewRef.current.s > 1.02) apply(FIT); else zoomAt(e, 1, 2.5);
  }, [apply, zoomAt, ease]);

  const drag = useRef<{ x: number; y: number; vx: number; vy: number; moved: boolean } | null>(null);
  const pan = useCallback((e: PointerEvent<HTMLElement>, phase: "down" | "move") => {
    if (phase === "down") {
      drag.current = { x: e.clientX, y: e.clientY, vx: viewRef.current.x, vy: viewRef.current.y, moved: false };
      e.currentTarget.setPointerCapture(e.pointerId);
      return;
    }
    const d = drag.current;
    if (!d) return;
    if (Math.abs(e.clientX - d.x) + Math.abs(e.clientY - d.y) > 4) { d.moved = true; setDragging(true); }
    if (d.moved && viewRef.current.s > 1) {
      const r = e.currentTarget.getBoundingClientRect();
      apply({ ...viewRef.current, x: d.vx + (e.clientX - d.x) / r.width, y: d.vy + (e.clientY - d.y) / r.height });
    }
  }, [apply]);
  const endPan = useCallback(() => {
    drag.current = null;
    setDragging(false);
  }, []);

  const hover = useRef<string | null>(null);
  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA" || e.metaKey || e.ctrlKey || isModalOpen()) return;
      if (e.key === "ArrowRight") { e.preventDefault(); jump(1); }
      else if (e.key === "ArrowLeft") { e.preventDefault(); jump(-1); }
      else if (e.key === "ArrowDown" || e.key === "Tab") { e.preventDefault(); cycle(1); }
      else if (e.key === "ArrowUp") { e.preventDefault(); cycle(-1); }
      else if (e.key === "0") apply(FIT);
      else if (hover.current && decisionOf(e)) {
        // The photo under the pointer is the target of the keys (which keys: keymap.ts).
        const path = hover.current, target = decisionOf(e) as State;
        // pressing the key of a state the app merely suggested confirms it; pressing it again on one you marked clears the mark
        const d = decisions.get(path);
        onDecide(path, d?.state === target && d.manual ? null : target);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enabled, jump, apply, cycle, onDecide, decisions]);

  // Best-fit arrangement: the number of columns that lets the photos be largest in the space available.
  const stage = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 0, h: 0 });
  useEffect(() => {
    const el = stage.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setBox({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, [set.length > 0]);
  const cols = useMemo(() => {
    const n = set.length;
    if (!n || !box.w || !box.h) return Math.max(1, n);
    const ratio = set.reduce((a, p) => a + p.width / p.height, 0) / n;
    let best = { cols: n, area: 0 };
    for (let c = 1; c <= n; c++) {
      const rows = Math.ceil(n / c);
      const cw = (box.w - GAP * (c - 1)) / c, ch = (box.h - GAP * (rows - 1)) / rows;
      const w = Math.min(cw, ch * ratio);
      if (w * (w / ratio) > best.area) best = { cols: c, area: w * (w / ratio) };
    }
    return best.cols;
  }, [set, box]);

  if (!cur) return null;
  const scopeToggle = (
    <div className="cmp2-count wide" role="group" aria-label={t("cmp.scope")}>
      <button aria-pressed={scope === "group"} onClick={() => setScope("group")}>{t("cmp.byGroup")}</button>
      <button aria-pressed={scope === "scene"} onClick={() => setScope("scene")}>{t("cmp.byScene")}</button>
    </div>
  );

  if (!groups.length || gi < 0) {
    return (
      <div className="cmp-empty">
        <img src={thumbSrc(cur)} alt="" draggable={false} />
        <h3>{t("cmp.emptyTitle")}</h3>
        <p className="ash">{t("cmp.emptyBody")}</p>
        {groups.length > 0 && <button className="btn primary" onClick={() => jump(1)}>{t("cmp.jump")}</button>}
        {scopeToggle}
      </div>
    );
  }

  const faces = set.some((p) => p.face_count > 0);
  const metrics: Metric[] = [
    { key: "score", label: t("inspector.overall"), get: (p) => pct(p.score) },
    { key: "sharp", label: t("inspector.sharp"), get: (p) => pct(p.sharp_score) },
    { key: "expo", label: t("inspector.expo"), get: (p) => pct(p.expo_score) },
    ...(faces ? [{ key: "face", label: t("inspector.faces"), get: (p: Photo) => (p.face_count ? pct(p.face_min ?? 0) : null) }] : []),
  ];

  return (
    <div className="cmp2">
      <button className="sarrow left" onClick={() => jump(-1)} disabled={gi <= 0} aria-label={t("cmp.prev")} data-tip={`${t("cmp.prev")} · ←`}>
        <svg width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M10 3L5 8l5 5" /></svg>
      </button>
      <div className="cmp2-stage" ref={stage} style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${Math.ceil(set.length / cols)}, minmax(0, 1fr))` }}>
        {set.map((p, i) => (
          <Cell key={p.path} p={p} index={i} st={decisions.get(p.path)?.state ?? "review"} manual={!!decisions.get(p.path)?.manual} view={view} dragging={dragging || !glide} metrics={metrics} set={set} count={set.length} personFilter={personFilter} onPerson={onPerson}
            onDecide={(s) => onDecide(p.path, s)} onOpen={() => onOpen(p.path)} onHover={(path) => { hover.current = path; }}
            zoomAt={zoomAt} pan={pan} endPan={endPan} resetOrZoom={resetOrZoom} />
        ))}
      </div>
      <button className="sarrow right" onClick={() => jump(1)} disabled={gi >= groups.length - 1} aria-label={t("cmp.next")} data-tip={`${t("cmp.next")} · →`}>
        <svg width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M6 3l5 5-5 5" /></svg>
      </button>
      <div className="cmp2-bottom">
        {scopeToggle}
        <span className="mono">{scope === "scene" ? t("cmp.sceneOf", { n: groupPhotos.length }) : t("cmp.groupOf", { i: gi + 1, n: groups.length })}</span>
        {groupPhotos.length > 2 && (
          <div className="cmp2-count" role="group" aria-label={t("cmp.count")}>
            {[2, 3, 4].filter((n) => n <= groupPhotos.length).map((n) => (
              <button key={n} aria-pressed={shownN === n} onClick={() => pickCount(n)}>{n}</button>
            ))}
          </div>
        )}
        {candidates.length > 0 && (
          <div className="cmp2-rail" aria-label={t("cmp.rail")}>
            {candidates.map((p) => (
              <button key={p.path} onClick={() => challenge(p.path)} data-tip={`${p.name} · ${pct(p.score)}`}>
                <img src={thumbSrc(p)} alt="" draggable={false} />
              </button>
            ))}
          </div>
        )}
        {showHint && <span className="ash cmp2-hint">{t("cmp.hint", keyVars(km))}</span>}
      </div>
    </div>
  );
}
