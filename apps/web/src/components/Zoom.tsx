import { useEffect, useRef } from "react";
import { copy, type Lang } from "../content";
import { EASE, addScene, countTo, lerp, pinProg, reduced, smooth } from "./scroll";

/** 照片墙里的一格是目标照片，其余是示例墙的照片。 */
const WALL = ["01", "02", "03", "05", "07", "08", "TARGET", "09", "10", "11", "12", "14"];

/**
 * 推进：一面照片墙先从后仰的状态立起来，再一路推近到其中一张，推到底时一次快门闪光，评分卡跟着出来。
 */
export default function Zoom({ lang }: { lang: Lang }) {
  const c = copy[lang].zoom;
  const sec = useRef<HTMLElement>(null), pin = useRef<HTMLDivElement>(null), head = useRef<HTMLDivElement>(null), tilt = useRef<HTMLDivElement>(null);
  const wall = useRef<HTMLDivElement>(null), flash = useRef<HTMLDivElement>(null), insp = useRef<HTMLElement>(null);

  useEffect(() => {
    const reduce = reduced();
    const zmWall = wall.current!, zmTilt = tilt.current!, zmPin = pin.current!, zmHead = head.current!, zmInsp = insp.current!, zmFlash = flash.current!;
    const cells = Array.from(zmWall.querySelectorAll<HTMLElement>(".ph")), target = zmWall.querySelector<HTMLElement>("[data-target]")!;
    const fills = Array.from(zmInsp.querySelectorAll<HTMLElement>(".fill")), vals = Array.from(zmInsp.querySelectorAll<HTMLElement>(".v"));
    let zg: { W: number; H: number; wx: number; wy: number; cx: number; cy: number; S: number } | null = null;
    let flashed = false, shown = false;
    function layout() {
      const prev = zmWall.style.transform, prevT = zmTilt.style.transform;
      zmWall.style.transform = "none"; zmTilt.style.transform = "none";
      const wr = zmWall.getBoundingClientRect(), cr = target.getBoundingClientRect(), pr = zmPin.getBoundingClientRect();
      zmWall.style.transform = prev; zmTilt.style.transform = prevT;
      zg = {
        W: pr.width, H: pr.height, wx: wr.left - pr.left, wy: wr.top - pr.top,
        cx: cr.left - wr.left + cr.width / 2, cy: cr.top - wr.top + cr.height / 2,
        S: Math.max(pr.width / cr.width, pr.height / cr.height) * 1.01,
      };
    }
    function render(p: number) {
      if (!zg) layout();
      const z = zg!;
      // 照片墙先从后仰的状态立起来，再开始推进
      const t0 = smooth(0, 0.18, p);
      zmTilt.style.transform = t0 < 1 ? `rotateX(${(1 - t0) * 32}deg) scale(${0.8 + 0.2 * t0})` : "";
      const k = smooth(0.16, 0.62, p);
      // 推到底的一瞬间：一次快门闪光
      if (k > 0.985 && !flashed) { flashed = true; if (!reduce) zmFlash.animate([{ opacity: 0.6 }, { opacity: 0 }], { duration: 600, easing: EASE }); }
      else if (k < 0.9) flashed = false;
      // 指数缩放让推进速度感受均匀；目标照片中心同步移到画面中心
      const s = Math.pow(z.S, k);
      const X = lerp(z.wx + z.cx, z.W / 2, k), Y = lerp(z.wy + z.cy, z.H / 2, k);
      zmWall.style.transform = `translate(${X - z.wx - s * z.cx}px, ${Y - z.wy - s * z.cy}px) scale(${s})`;
      cells.forEach((cell) => { if (cell !== target) cell.style.opacity = String(1 - 0.7 * k); });
      const h = smooth(0.04, 0.22, p);
      zmHead.style.opacity = String(1 - h);
      zmHead.style.transform = `translateY(${-h * 40}px)`;
      const ci = smooth(0.64, 0.78, p);
      zmInsp.style.opacity = String(ci);
      zmInsp.style.transform = `translateY(${(1 - ci) * 32}px)`;
      fills.forEach((f) => { f.style.transform = `scaleX(${+f.dataset.v! * ci})`; });
      const show = ci > 0.5;
      if (show !== shown) { shown = show; vals.forEach((v) => { if (show) countTo(v, +v.dataset.to!, 700); else v.textContent = "0"; }); }
    }
    const onResize = () => { zg = null; if (reduce) render(1); };
    addEventListener("resize", onResize);
    let stop = () => {};
    if (reduce) render(1); else stop = addScene(sec.current!, pinProg, render);
    return () => { stop(); removeEventListener("resize", onResize); };
  }, []);

  return (
    <section ref={sec} className="zm" id="zoom" aria-labelledby="zm-title">
      <div className="pin zm-pin" ref={pin}>
        <div className="wrap zm-head" ref={head}>
          <p className="eyebrow">{c.eyebrow}</p>
          <h2 className="h2" id="zm-title" style={{ marginTop: "var(--s-3)" }}>{c.title}</h2>
          <p className="lead">{c.lead}</p>
        </div>
        <div className="zm-stage">
          <div className="zm-tilt" ref={tilt}>
            <div className="zm-wall" ref={wall} aria-hidden>
              {WALL.map((n) => n === "TARGET"
                ? <div key={n} className="ph" data-target><img src="/photos/sunset-1600.webp" width={1600} height={900} alt="" draggable={false} /></div>
                : <div key={n} className="ph"><img src={`/wall/${n}.webp`} width={600} height={400} alt="" loading="lazy" draggable={false} /></div>)}
            </div>
          </div>
        </div>
        <div className="zm-flash" ref={flash} aria-hidden />
        <aside className="zm-insp" ref={insp} aria-label={c.aria}>
          <div className="top"><span>{c.selected}</span><span>{c.sample}</span></div>
          {c.metrics.map(([k, v]) => (
            <div key={k} className="metric">
              <div className="row"><span className="k">{k}</span><span className="v" data-to={v}>0</span></div>
              <div className="track"><div className="fill" data-v={v / 100} /></div>
            </div>
          ))}
          <p className="zm-keys">{c.keys.map(([k, l]) => <span key={k}><kbd>{k}</kbd> {l} </span>)}</p>
        </aside>
      </div>
    </section>
  );
}
