import { PointerEvent, useState } from "react";
import { useTranslation } from "react-i18next";

/** A panel width that is remembered between runs (and always kept between `min` and `max`). */
export function usePanelWidth(key: string, def: number, min: number, max: number) {
  const clamp = (n: number) => Math.min(max, Math.max(min, Math.round(n)));
  const [width, setWidth] = useState(() => { try { const v = Number(localStorage.getItem(key)); return v ? clamp(v) : def; } catch { return def; } });
  const set = (n: number, save = false) => {
    const w = clamp(n);
    setWidth(w);
    if (save) { try { localStorage.setItem(key, String(w)); } catch { /* storage unavailable */ } }
  };
  return { width, set, reset: () => { set(def); try { localStorage.removeItem(key); } catch { /* ignore */ } }, min, max };
}

/**
 * The thin strip on a panel's edge that drags it wider or narrower. `edge` is the side of the panel it sits on: "right" for a panel
 * on the left of the window (dragging right widens it), "left" for one on the right. Double-click puts the width back.
 * Arrow keys work too when it is focused.
 */
export default function Resizer({ panel, edge, label, onDragging, className = "" }: {
  panel: ReturnType<typeof usePanelWidth>; edge: "left" | "right"; label: string; onDragging?: (on: boolean) => void; className?: string;
}) {
  const { t } = useTranslation();
  const [on, setOn] = useState(false);
  const sign = edge === "right" ? 1 : -1;
  const down = (e: PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    const x0 = e.clientX, w0 = panel.width;
    setOn(true); onDragging?.(true);
    const move = (m: globalThis.PointerEvent) => panel.set(w0 + sign * (m.clientX - x0));
    const up = (m: globalThis.PointerEvent) => {
      panel.set(w0 + sign * (m.clientX - x0), true);
      setOn(false); onDragging?.(false);
      el.removeEventListener("pointermove", move); el.removeEventListener("pointerup", up); el.removeEventListener("pointercancel", up);
    };
    el.addEventListener("pointermove", move); el.addEventListener("pointerup", up); el.addEventListener("pointercancel", up);
  };
  return (
    <div className={`resizer ${edge} ${on ? "on" : ""} ${className}`} role="separator" aria-orientation="vertical" aria-label={label} tabIndex={0}
      aria-valuenow={panel.width} aria-valuemin={panel.min} aria-valuemax={panel.max} title={t("cull.resizeHint")}
      onPointerDown={down} onDoubleClick={panel.reset}
      onKeyDown={(e) => {
        const step = e.shiftKey ? 40 : 12;
        if (e.key === "ArrowLeft") { e.preventDefault(); panel.set(panel.width - sign * step, true); }
        if (e.key === "ArrowRight") { e.preventDefault(); panel.set(panel.width + sign * step, true); }
      }} />
  );
}
