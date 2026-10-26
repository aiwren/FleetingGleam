import { AnimatePresence, motion } from "motion/react";
import { ReactNode, RefObject, forwardRef, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState } from "react";

export type WallItem = { key: string; ratio: number; /** 相似组的末尾：后面多留一点间隙。 */ gapAfter?: boolean };
/** 组末多留的间隙。 */
const GROUP_GAP = 6;
export type Box = { left: number; top: number; width: number; height: number };
export type WallHandle = {
  reveal: (key: string) => void;
  /** 往下（n > 0）或往上 n 行里横向离 `key` 最近、`ok` 认可的格子；那一行没有就接着往同一方向找。到头了返回 null。 */
  rowStep: (key: string, n: number, ok?: (key: string) => boolean) => string | null;
  /** 往下（1）或往上（-1）翻一屏，落在那一行里横向最近的格子。 */
  pageStep: (key: string, dir: 1 | -1, ok?: (key: string) => boolean) => string | null;
};

export type WallLayout = "justified" | "grid";

const GAP = 6;

/** Justified rows: every row is as wide as the wall and every photo keeps its own aspect ratio, so nothing is cropped. */
function justify(items: WallItem[], width: number, target: number): Box[] {
  const boxes: Box[] = new Array(items.length);
  let from = 0;
  let y = 0;
  // 一行里的间隙：相邻两张 GAP，组末那张后面再加 GROUP_GAP（行尾不算）
  const gaps = (a: number, b: number) => { let g = 0; for (let i = a; i < b - 1; i++) g += GAP + (items[i].gapAfter ? GROUP_GAP : 0); return g; };
  const close = (to: number, last: boolean) => {
    if (to <= from) return;
    let s = 0;
    for (let i = from; i < to; i++) s += items[i].ratio;
    let h = (width - gaps(from, to)) / s;
    if (last && h > target * 1.35) h = target; // a short last row keeps the natural height instead of stretching
    let x = 0;
    for (let i = from; i < to; i++) { const w = items[i].ratio * h; boxes[i] = { left: x, top: y, width: w, height: h }; x += w + GAP + (items[i].gapAfter && i < to - 1 ? GROUP_GAP : 0); }
    y += h + GAP;
    from = to;
  };
  let sum = 0;
  for (let i = 0; i < items.length; i++) {
    sum += items[i].ratio;
    if ((width - gaps(from, i + 1)) / sum <= target) { close(i + 1, false); sum = 0; }
  }
  close(items.length, true);
  return boxes;
}

/** Uniform square cells (photos are cropped to fill them). */
function grid(items: WallItem[], width: number): Box[] {
  const cols = Math.max(2, Math.round(width / 150));
  const size = (width - GAP * (cols - 1)) / cols;
  return items.map((_, i) => ({ left: (i % cols) * (size + GAP), top: Math.floor(i / cols) * (size + GAP), width: size, height: size }));
}

function place(items: WallItem[], width: number, layout: WallLayout) {
  const boxes = layout === "grid" ? grid(items, width) : justify(items, width, 200);
  let height = 0;
  for (const b of boxes) height = Math.max(height, b.top + b.height);
  return { boxes, height };
}

/**
 * A windowed justified wall: only rows near the viewport are mounted, so 300 or 5000 photos cost the same.
 * `render` is called for each mounted item with its final box.
 */
const Wall = forwardRef<WallHandle, {
  items: WallItem[];
  scrollRef: RefObject<HTMLElement | null>;
  render: (index: number, box: Box) => ReactNode;
  layout?: WallLayout;
  /** Key of the selected item: a single focus frame glides to it, instead of each tile drawing its own. */
  selected?: string | null;
  /** 多选时其余选中的格子：同一种角框，原地不动（不另加颜色）。 */
  marked?: ReadonlySet<string> | null;
  /**
   * While a side panel slides open (negative) or shut (positive) the wall is laid out at the width it will end up with, right away,
   * instead of following the shrinking container frame by frame: the photos glide once to their final places while the panel expands.
   */
  widthDelta?: number | null;
}>(function Wall({ items, scrollRef, render, layout: kind = "justified", selected, marked = null, widthDelta = null }, ref) {
  const wrap = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [view, setView] = useState({ top: 0, height: 900 });

  const latest = useRef(0);
  const frozen = useRef<number | null>(null);
  const put = useCallback((w: number) => { latest.current = w; setWidth(w); }, []);
  const measure = useCallback(() => Math.floor((wrap.current?.getBoundingClientRect().width) ?? 0), []);

  useLayoutEffect(() => {
    const el = wrap.current;
    if (!el) return;
    // Otherwise, while a panel or the sidebar slides open the width changes every frame; re-justifying the rows each time makes the photos
    // hop between rows. So the layout waits until the width has stopped changing (and if it ends where it began, nothing moves at all).
    let timer = 0;
    const ro = new ResizeObserver(() => {
      if (frozen.current !== null) return;
      window.clearTimeout(timer);
      timer = window.setTimeout(() => put(measure()), 120);
    });
    ro.observe(el);
    put(measure());
    return () => { ro.disconnect(); window.clearTimeout(timer); };
  }, [put, measure]);

  // A panel starts to open or close: lay the wall out at its final width now, and ignore the container until the panel has settled.
  useLayoutEffect(() => {
    if (widthDelta) {
      frozen.current = Math.max(100, latest.current + widthDelta);
      put(frozen.current);
    } else if (frozen.current !== null) {
      frozen.current = null;
      put(measure());
    }
  }, [widthDelta, put, measure]);

  const offset = useCallback(() => {
    const s = scrollRef.current, w = wrap.current;
    if (!s || !w) return 0;
    return w.getBoundingClientRect().top - s.getBoundingClientRect().top + s.scrollTop;
  }, [scrollRef]);

  useEffect(() => {
    const s = scrollRef.current;
    if (!s) return;
    let raf = 0;
    // Only re-render when the scroll position crosses a 160 px step: the mounted rows are the same in between.
    const update = () => {
      raf = 0;
      const top = Math.floor((s.scrollTop - offset()) / 160) * 160, height = s.clientHeight;
      setView((v) => (v.top === top && v.height === height ? v : { top, height }));
    };
    const on = () => { if (!raf) raf = requestAnimationFrame(update); };
    update();
    s.addEventListener("scroll", on, { passive: true });
    window.addEventListener("resize", on);
    return () => { s.removeEventListener("scroll", on); window.removeEventListener("resize", on); if (raf) cancelAnimationFrame(raf); };
  }, [scrollRef, offset, items.length]);

  const layout = useMemo(() => (width > 0 ? place(items, width, kind) : { boxes: [] as Box[], height: 0 }), [items, width, kind]);
  const index = useMemo(() => new Map(items.map((it, i) => [it.key, i])), [items]);
  /** reveal 发起的平滑滚动要去的位置和发起时间。 */
  const gliding = useRef<{ top: number; t: number } | null>(null);
  // 行：同一行的格子 top 相同（拼贴和宫格都是），按顺序连在一起
  const rows = useMemo(() => {
    const list: number[][] = [];
    const of = new Int32Array(layout.boxes.length);
    let top = NaN;
    layout.boxes.forEach((b, i) => { if (b.top !== top) { list.push([]); top = b.top; } list[list.length - 1].push(i); of[i] = list.length - 1; });
    return { list, of };
  }, [layout]);

  const rowStep = useCallback((key: string, n: number, ok?: (key: string) => boolean) => {
    const i = index.get(key), b = i === undefined ? undefined : layout.boxes[i];
    if (i === undefined || !b || n === 0) return null;
    const cx = b.left + b.width / 2, dir = n > 0 ? 1 : -1;
    for (let r = Math.max(0, Math.min(rows.list.length - 1, rows.of[i] + n)); r >= 0 && r < rows.list.length; r += dir) {
      if (r === rows.of[i]) break;
      let best = -1, dist = Infinity;
      for (const j of rows.list[r]) {
        if (ok && !ok(items[j].key)) continue;
        const c = layout.boxes[j], d = Math.abs(c.left + c.width / 2 - cx);
        if (d < dist) { dist = d; best = j; }
      }
      if (best >= 0) return items[best].key;
    }
    return null;
  }, [index, layout, rows, items]);

  useImperativeHandle(ref, () => ({
    rowStep,
    pageStep(key, dir, ok) {
      const i = index.get(key), s = scrollRef.current;
      if (i === undefined || !s) return null;
      // 落点：往那个方向挪一屏（少一行的高度，翻过去还看得见原来那一行的边）
      const b = layout.boxes[i], y = b.top + dir * Math.max(b.height, s.clientHeight - b.height);
      let target = rows.of[i];
      for (let r = rows.of[i]; r >= 0 && r < rows.list.length; r += dir) {
        target = r;
        const top = layout.boxes[rows.list[r][0]].top;
        if (dir > 0 ? top >= y : top <= y) break;
      }
      return rowStep(key, target === rows.of[i] ? dir : target - rows.of[i], ok);
    },
    reveal(key) {
      const i = index.get(key), s = scrollRef.current;
      const b = i === undefined ? undefined : layout.boxes[i];
      if (!b || !s) return;
      const top = offset() + b.top, bottom = top + b.height;
      // 上一次平滑滚动还在路上（连按 End、Home 时）：按它要去的位置算，不按眼下还没动的位置
      const p = gliding.current, at = p && performance.now() - p.t < 700 ? p.top : s.scrollTop;
      const go = (y: number) => { const t = Math.max(0, Math.min(y, s.scrollHeight - s.clientHeight)); gliding.current = { top: t, t: performance.now() }; s.scrollTo({ top: t, behavior: "smooth" }); };
      if (top < at + 8) go(top - 24);
      else if (bottom > at + s.clientHeight - 8) go(bottom - s.clientHeight + 24);
    },
  }), [index, layout, scrollRef, offset, rows, rowStep]);

  // A screen and a half above and two screens below are prepared ahead: their images load before the rows scroll into view.
  const from = view.top - view.height * 1.5;
  const to = view.top + view.height * 3;
  const mounted: ReactNode[] = [];
  for (let i = 0; i < layout.boxes.length; i++) {
    const b = layout.boxes[i];
    if (b.top + b.height < from || b.top > to) continue;
    mounted.push(render(i, b));
  }

  const si = selected ? index.get(selected) : undefined;
  const sb = si === undefined ? undefined : layout.boxes[si];
  const marks: ReactNode[] = [];
  marked?.forEach((key) => {
    const i = index.get(key), b = i === undefined ? undefined : layout.boxes[i];
    if (!b || key === selected || b.top + b.height < from || b.top > to) return;
    marks.push(<div key={key} className="selframe" aria-hidden style={{ left: b.left, top: b.top, width: b.width, height: b.height }}><i /><i /><i /><i /></div>);
  });
  return (
    <div ref={wrap} className={`wall virtual ${kind}`} style={{ height: layout.height }}>
      {mounted}
      {marks}
      {/* One frame for the whole wall: it travels from the previous photo to the new one, across rows too, and is never clipped by a tile. */}
      <AnimatePresence>
        {sb && (
          <motion.div key="selframe" className="selframe" aria-hidden
            initial={{ opacity: 0, left: sb.left, top: sb.top, width: sb.width, height: sb.height, scale: 1.04 }}
            animate={{ opacity: 1, left: sb.left, top: sb.top, width: sb.width, height: sb.height, scale: 1 }}
            exit={{ opacity: 0, transition: { duration: 0.15 } }}
            transition={{ type: "spring", stiffness: 380, damping: 36, opacity: { duration: 0.18 } }}>
            <i /><i /><i /><i />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});
export default Wall;
