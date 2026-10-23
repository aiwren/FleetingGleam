import { forwardRef, RefObject, useImperativeHandle, useRef, useState } from "react";
import { motion } from "motion/react";
import { Decision } from "../types";
import { Star } from "./Tile";

type Mote = { id: number; keep: boolean; from: { x: number; y: number }; to: { x: number; y: number }; delay: number };

export type FlightsHandle = {
  /** 刚判出来（或改判）的那些：屏幕上看得见的格子各放一粒光，飘进对应的页签行。 */
  judged: (items: { path: string; decision: Decision }[]) => void;
};

/** 一次最多同时飘这么多粒。 */
const MAX_MOTES = 10;
const SIZE = 16;

/**
 * 分析中的"分拣"动效（Android 的 JudgeFlights 的桌面版）：一张照片判为保留 / 待复查时，一粒光（保留是琥珀四角星，待复查是灰色空心星）
 * 从格子右上角的星点升起、飘进左侧筛选栏里对应那一行的数字旁，到了就消散；淘汰不飞。只给屏幕上看得见的格子做，一批错开 60 ms 出发。
 * 标记在这个应用里一直是"光"，所以飘过去的也是光，不是搬一张小图。盖在整个窗口上。
 */
const Flights = forwardRef<FlightsHandle, { scrollRef: RefObject<HTMLDivElement | null> }>(function Flights({ scrollRef }, ref) {
  const [list, setList] = useState<Mote[]>([]);
  const seq = useRef(0);
  const inFlight = useRef(0);
  useImperativeHandle(ref, () => ({
    judged(items) {
      const view = scrollRef.current?.getBoundingClientRect();
      const add: Mote[] = [];
      for (const j of items) {
        if (j.decision.state === "reject") continue;
        if (inFlight.current + add.length >= MAX_MOTES) break;
        const tile = document.getElementById(`t-${j.path}`);
        const row = document.getElementById(`fp-${j.decision.state}`);
        if (!tile || !row) continue;
        const from = tile.getBoundingClientRect();
        const count = row.querySelector("em")?.getBoundingClientRect() ?? row.getBoundingClientRect();
        if (from.width === 0 || (view && (from.bottom < view.top || from.top > view.bottom))) continue;
        add.push({ id: ++seq.current, keep: j.decision.state === "keep", from: { x: from.right - 18, y: from.top + 18 }, to: { x: count.left - 10, y: count.top + count.height / 2 }, delay: add.length * 0.06 });
      }
      if (add.length) { inFlight.current += add.length; setList((l) => [...l, ...add]); }
    },
  }));
  return (
    <div className="flights" aria-hidden>
      {list.map((m) => {
        const half = SIZE / 2;
        const midY = (m.from.y + m.to.y) / 2 - 28;
        return (
          <motion.span key={m.id} className={`mote ${m.keep ? "keep" : "review"}`}
            initial={{ x: m.from.x - half, y: m.from.y - half, scale: 1, opacity: 0 }}
            animate={{ x: [m.from.x - half, (m.from.x + m.to.x) / 2 - half, m.to.x - half], y: [m.from.y - half, midY - half, m.to.y - half], scale: [1, 0.85, 0.6], opacity: [0, 1, 1, 0] }}
            transition={{ duration: 0.76, ease: [0.4, 0, 0.2, 1], delay: m.delay, times: [0, 0.5, 1], opacity: { times: [0, 0.12, 0.72, 1], duration: 0.76, delay: m.delay } }}
            onAnimationComplete={() => { inFlight.current -= 1; setList((l) => l.filter((x) => x.id !== m.id)); }}>
            <Star filled={m.keep} size={SIZE} />
          </motion.span>
        );
      })}
    </div>
  );
});

export default Flights;
