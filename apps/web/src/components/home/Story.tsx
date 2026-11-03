import { useEffect, useRef } from "react";
import { copy, type Lang } from "../../content";
import { Gleam } from "../Gleam";
import { addScene, lerp, pinProg, reduced, smooth } from "../scroll";
import { ONE_SLOT, Wall } from "./Backdrops";
import { home } from "./copy";

/**
 * 第二屏（照片墙）和"几千张 → 几张 → 那一张"合成一幕：舞台钉住约四屏，一个进度驱动全部画面。
 * 1. 斜放的照片墙在流动，"每次出门，回来都是几千张"，计数跟着滚动走；
 * 2. 墙放平成一张接触印样、停止流动，大部分照片变淡（只变淡，颜色不动），每列留下的那张亮起琥珀边——"分组，打分"；
 * 3. 那一张从墙里它自己那一格放大、浮到前面铺满舞台，评分理由浮在玻璃卡上；墙里那一格留下一个暗位。
 */
export default function Story({ lang }: { lang: Lang }) {
  const s = home[lang].story, w = home[lang].wall, scene = copy[lang].hero.scene;
  const sec = useRef<HTMLElement>(null), one = useRef<HTMLDivElement>(null);
  const t1 = useRef<HTMLDivElement>(null), t2 = useRef<HTMLDivElement>(null), t3 = useRef<HTMLDivElement>(null), num = useRef<HTMLSpanElement>(null);
  const why = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = sec.current!;
    const show = (e: HTMLElement, v: number, dy = 24) => {
      e.style.opacity = v.toFixed(3);
      e.style.transform = `translateY(${((1 - v) * dy).toFixed(1)}px)`;
      e.style.filter = v > 0.99 ? "none" : `blur(${((1 - v) * 8).toFixed(1)}px)`;
      e.style.visibility = v < 0.01 ? "hidden" : "";
    };
    const pin = el.querySelector<HTMLElement>(".v-story-pin")!;
    const cols = Array.from(el.querySelectorAll<HTMLElement>(".v-wall-col"));
    const ones = Array.from(el.querySelectorAll<HTMLElement>(".v-wall-tile.is-one"));
    const fmt = (n: number) => n.toLocaleString(lang === "zh" ? "zh-CN" : "en-US");
    /**
     * 墙放平时停下：各列从流动停在当前位置；"那一张"所在的那一列再滑到固定位置，让那一格落在舞台中间偏右。
     * 往回滚、墙重新斜起来时，各列从停下的位置接着流动（用负的 animation-delay 接上），不会跳。
     */
    let held = false;
    const hold = (on: boolean) => {
      if (on === held) return;
      held = on;
      cols.forEach((col, c) => {
        const half = col.offsetHeight / 2, dur = Number(col.dataset.dur) * 1000;
        const y = -new DOMMatrixReadOnly(getComputedStyle(col).transform).m42;
        if (on) {
          col.style.animation = "none"; col.style.transition = "none"; col.style.transform = `translateY(${-y}px)`;
          if (c === ONE_SLOT.c) {
            const t = ones[0];
            let target = t.offsetTop + t.offsetHeight / 2 - half;
            if (target < 0) target += half;
            void col.offsetHeight;
            col.style.transition = "transform 900ms cubic-bezier(.22,1,.36,1)";
            col.style.transform = `translateY(${-target}px)`;
          }
        } else {
          // 偶数列（第 2、4、6 列）是反着走的
          const frac = Math.min(1, Math.max(0, y / half)), prog = c % 2 ? 1 - frac : frac;
          col.style.transition = ""; col.style.transform = ""; col.style.animation = "";
          col.style.animationDelay = `${(-prog * dur).toFixed(0)}ms`;
        }
      });
    };
    const render = (p: number) => {
      const fl = smooth(0.28, 0.5, p), g = smooth(0.4, 0.56, p), fin = smooth(0.62, 0.84, p);
      el.style.setProperty("--fl", fl.toFixed(4));
      el.style.setProperty("--g", g.toFixed(4));
      el.style.setProperty("--fin", fin.toFixed(4));
      el.classList.toggle("is-held", fl > 0.02);
      el.classList.toggle("is-graded", g > 0.01);
      el.classList.toggle("is-flying", fin > 0.001);
      if (!reduced()) hold(fl > 0.02);
      // 那一张：从墙里那一格的位置出发（每帧现量，往回滚就原路缩回），长到四边留白的画框
      const o = one.current!, narrow = el.clientWidth < 768, m = narrow ? 12 : 24;
      const W = el.clientWidth, H = pin.clientHeight, base = pin.getBoundingClientRect();
      let from = { x: (W - 200) / 2, y: (H - 133) / 2, w: 200, h: 133 };
      // 两份复制里取离舞台中心近的那一格
      let best = Infinity;
      for (const t of ones) {
        const r = t.getBoundingClientRect();
        const d = Math.hypot(r.left + r.width / 2 - base.left - W / 2, r.top + r.height / 2 - base.top - H / 2);
        if (r.width > 0 && d < best) { best = d; from = { x: r.left - base.left, y: r.top - base.top, w: r.width, h: r.height }; }
      }
      const ww = lerp(from.w, W - 2 * m, fin), hh = lerp(from.h, H - 2 * m, fin);
      Object.assign(o.style, {
        width: `${ww.toFixed(1)}px`, height: `${hh.toFixed(1)}px`,
        left: `${lerp(from.x, m, fin).toFixed(1)}px`, top: `${lerp(from.y, m, fin).toFixed(1)}px`,
        borderRadius: `${lerp(6, narrow ? 18 : 24, fin).toFixed(1)}px`,
        opacity: fin > 0.001 ? "1" : "0",
      });
      show(t1.current!, 1 - smooth(0.24, 0.3, p));
      show(t2.current!, smooth(0.33, 0.4, p) * (1 - smooth(0.56, 0.61, p)));
      show(t3.current!, smooth(0.76, 0.84, p));
      Array.from(why.current!.children).forEach((c, i) => show(c as HTMLElement, smooth(0.82 + i * 0.03, 0.9 + i * 0.03, p), 16));
    };
    if (reduced()) {
      num.current!.textContent = fmt(2438);
      render(1);
      const r = () => render(1);
      addEventListener("resize", r); return () => removeEventListener("resize", r);
    }
    // 计数：这一幕一进视野就自己数上去（锚点直接跳过来也一样），不用等滚动
    let counted = false, raf = 0;
    const io = new IntersectionObserver(([e]) => {
      if (!e.isIntersecting || counted) return;
      counted = true;
      const t0 = performance.now();
      const step = (t: number) => {
        const k = Math.min(1, (t - t0) / 1400);
        num.current!.textContent = fmt(Math.round(2438 * (1 - Math.pow(1 - k, 3))));
        if (k < 1) raf = requestAnimationFrame(step);
      };
      raf = requestAnimationFrame(step);
    }, { threshold: 0.05 });
    io.observe(el);
    const stop = addScene(el, pinProg, render);
    return () => { stop(); io.disconnect(); cancelAnimationFrame(raf); };
  }, [lang]);

  return (
    <section ref={sec} className="v-story" id="story" aria-label={`${w.title} ${s.t2} ${s.t3}`}>
      <div className="v-story-pin">
        <Wall />
        <div className="v-story-shade" aria-hidden />
        <div className="v-one" ref={one} aria-hidden style={{ opacity: 0 }}>
          <img src="/home/one-1800.webp" srcSet="/home/one-900.webp 900w, /home/one-1800.webp 1800w" sizes="100vw" alt="" loading="lazy" draggable={false} />
          <span className="v-one-star"><Gleam size={22} /></span>
        </div>
        <div className="v-story-text" ref={t1}>
          <h2 className={lang === "zh" ? "is-zh" : undefined}>{w.title}</h2>
          <p className="v-mono"><span ref={num}>0</span> {s.count}</p>
        </div>
        <div className="v-story-text" ref={t2} style={{ opacity: 0, visibility: "hidden" }}>
          <h2>{s.t2}</h2>
          <p>{s.t2s}</p>
        </div>
        <div className={`v-story-one${lang === "zh" ? " is-zh" : ""}`} ref={t3} style={{ opacity: 0, visibility: "hidden" }}>
          <h2>{s.t3}</h2>
        </div>
        <div className="v-why" ref={why}>
          {scene.reasons.map(([k, v]) => (
            <div key={k} className="v-glass v-why-row" style={{ opacity: 0 }}><span>{k}</span><b className="v-mono">{v}</b></div>
          ))}
          <div className="v-glass v-why-row v-why-line" style={{ opacity: 0 }}><Gleam size={14} /><span>{scene.why}</span></div>
          <p className="v-why-note v-mono" style={{ opacity: 0 }}>{scene.sample}</p>
        </div>
      </div>
    </section>
  );
}
