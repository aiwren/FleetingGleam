import { useEffect, useRef } from "react";
import type { Lang } from "../../content";
import { Gleam } from "../Gleam";
import { clamp, lerp, reduced, smooth } from "../scroll";
import { home } from "./copy";

/**
 * 替掉原来视频的两块画面：
 * - Focus（首屏、收尾）：一张大图，取景框跟着鼠标走，框里清楚、框外虚，停下来就"定格"。
 * - Wall（"几千张 → 那一张"那一幕）：斜放在纵深里的一整面照片墙，各列慢慢上下流动，鼠标让墙轻轻偏转，偶尔有几张亮起琥珀细边（被留下）。
 * 照片都是 Lorem Picsum 的示例照片（Unsplash 许可），颜色一律不动。
 */

// 同一趟旅行的样子：雪山、高原、星空、晨光（Picsum / Unsplash 许可，来源见 docs/THIRD_PARTY.md）
const WALL_IDS = [29, 46, 128, 184, 191, 231, 256, 287, 296, 327, 377, 406, 416, 426, 450, 474, 482, 507, 519, 523, 537, 547, 558, 565, 566, 572, 575, 599, 651, 676, 683, 688, 724, 772, 806, 814, 830, 872, 873, 902, 908, 910, 916, 918, 1016, 1018, 1036, 1015, 1043];
const PICS = WALL_IDS.map((id) => `/home/wall/${id}.webp`);

/** 首屏滚出去的进度：0 是刚打开，1 是首屏底边到了窗口顶。 */
const outProg = (el: HTMLElement) => { const r = el.getBoundingClientRect(); return clamp(-r.top / Math.max(1, r.height), 0, 1); };

/* ---------------- 照片墙 ---------------- */
const COLS = 7, ROWS = 7;
/** "那一张"在墙里的格子：中间偏右一列，正好是这一列会亮琥珀边的那一格。 */
export const ONE_SLOT = { c: 4, r: 4 };
const ONE_SRC = "/home/wall/664.webp";
/**
 * 只用在"几千张 → 那一张"那一幕里：倾斜由舞台进度管（--fl 等），这里只管鼠标偏转和不在视野时暂停。
 * 每列内容复制一遍，往上（或往下）走一半正好接上，所以能无缝循环。
 */
export function Wall() {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = box.current!;
    if (reduced()) return;
    let tx = 0, ty = 0, x = 0, y = 0, raf = 0, on = false;
    const move = (e: PointerEvent) => { tx = (e.clientX / innerWidth - 0.5) * 2; ty = (e.clientY / innerHeight - 0.5) * 2; };
    const tick = () => {
      x += (tx - x) * 0.05; y += (ty - y) * 0.05;
      el.style.setProperty("--px", x.toFixed(4)); el.style.setProperty("--py", y.toFixed(4));
      raf = on ? requestAnimationFrame(tick) : 0;
    };
    const io = new IntersectionObserver(([e]) => {
      on = e.isIntersecting;
      el.classList.toggle("is-paused", !on);
      if (on && !raf) raf = requestAnimationFrame(tick);
    });
    io.observe(el);
    addEventListener("pointermove", move, { passive: true });
    return () => { io.disconnect(); cancelAnimationFrame(raf); removeEventListener("pointermove", move); };
  }, []);
  return (
    <div ref={box} className="v-bd v-wall is-staged" aria-hidden>
      <div className="v-wall-plane">
        {Array.from({ length: COLS }, (_, c) => {
          const list = Array.from({ length: ROWS }, (_, r) => (c === ONE_SLOT.c && r === ONE_SLOT.r ? ONE_SRC : PICS[(c * ROWS + r) % PICS.length]));
          return (
            <div key={c} className="v-wall-col" data-dur={70 + (c % 3) * 14} style={{ animationDuration: `${70 + (c % 3) * 14}s` }}>
              {[...list, ...list].map((src, r) => {
                // 每列有一张会周期性亮起琥珀细边，各列错开时间
                const k = r % ROWS === (c * 2 + 3) % ROWS, one = c === ONE_SLOT.c && r % ROWS === ONE_SLOT.r;
                return (
                  <div key={r} className={`v-wall-tile${k ? " is-k" : ""}${one ? " is-one" : ""}`} style={k ? { animationDelay: `${-(c * 2.3).toFixed(1)}s` } : undefined}>
                    <img src={src} alt="" draggable={false} loading="lazy" />
                    {k && <span className="v-wall-star"><Gleam size={14} /></span>}
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/* ---------------- 取景对焦 ---------------- */
// 主体（山尖）在原图里的位置，和 object-position 一起换算成屏幕坐标
const IMG = { w: 2400, h: 1600, px: 0.7, py: 0.6, sx: 0.72, sy: 0.64 };
export function Focus({ lang, quiet }: { lang: Lang; quiet?: boolean }) {
  const box = useRef<HTMLDivElement>(null), read = useRef<HTMLSpanElement>(null);
  const t = home[lang].focus;
  useEffect(() => {
    const el = box.current!;
    if (quiet) return;
    let W = 0, H = 0, sub = { x: 0, y: 0 };
    const imgs = Array.from(el.querySelectorAll("img"));
    const measure = () => {
      W = el.clientWidth; H = el.clientHeight;
      // 竖屏手机上山尖会落在标题后面：图放大到 1.8 倍高、底边贴齐，山尖就到了上三分之一
      const tall = W < 768 && H > W;
      const s = Math.max(W / IMG.w, H / IMG.h) * (tall ? 1.8 : 1), iw = IMG.w * s, ih = IMG.h * s;
      const left = tall ? clamp(W * 0.6 - IMG.sx * iw, W - iw, 0) : (W - iw) * IMG.px, top = tall ? H - ih : (H - ih) * IMG.py;
      for (const im of imgs) Object.assign(im.style, { left: `${left}px`, top: `${top}px`, width: `${iw}px`, height: `${ih}px` });
      el.style.setProperty("--ox", `${left.toFixed(1)}px`); el.style.setProperty("--oy", `${top.toFixed(1)}px`);
      sub = { x: left + IMG.sx * iw, y: top + IMG.sy * ih };
    };
    measure();
    const set = (x: number, y: number, r: number, k: number) => {
      el.style.setProperty("--fx", `${x.toFixed(1)}px`); el.style.setProperty("--fy", `${y.toFixed(1)}px`);
      el.style.setProperty("--fr", `${r.toFixed(1)}px`); el.style.setProperty("--k", k.toFixed(3));
    };
    const label = (locked: boolean) => { el.classList.toggle("is-locked", locked); read.current!.textContent = locked ? t.locked : t.seeking; };
    if (reduced()) {
      set(sub.x, sub.y, Math.max(W, H), 1); label(true);
      const r = () => { measure(); set(sub.x, sub.y, Math.max(W, H), 1); };
      addEventListener("resize", r); return () => removeEventListener("resize", r);
    }
    const fine = matchMedia("(hover: hover) and (pointer: fine)").matches;
    // 开场：取景框从画面中间滑到山尖，再锁定
    let x = W * 0.5, y = H * 0.45, tx = sub.x, ty = sub.y, r = 0, k = 0, still = 0, raf = 0, on = false, last = performance.now(), user = false;
    const t0 = performance.now() + 500;
    const move = (e: PointerEvent) => { const b = el.getBoundingClientRect(); tx = e.clientX - b.left; ty = e.clientY - b.top; user = true; };
    const tap = (e: PointerEvent) => { if (e.pointerType !== "mouse") move(e); };
    const tick = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000); last = now;
      if (now < t0) { raf = on ? requestAnimationFrame(tick) : 0; return; }
      const ease = user ? 0.14 : 0.045;
      const dx = tx - x, dy = ty - y;
      x += dx * ease; y += dy * ease;
      const v = Math.hypot(dx, dy);
      still = v < 2 ? still + dt : 0;
      // 动的时候框里也有点虚，停住 0.35 秒后锁定：清楚的范围张开，框变琥珀
      const lock = smooth(0.35, 0.7, still);
      k += (lock - k) * 0.12;
      const base = Math.min(W, H) * (W < 768 ? 0.28 : 0.2);
      const sp = outProg(el);
      // 往下滚时清楚的范围一路张到整张图
      const target = lerp(base * (0.55 + 0.45 * k), Math.hypot(W, H), smooth(0.05, 0.7, sp));
      r += (target - r) * 0.1;
      set(x, y, r, k);
      if ((k > 0.6) !== el.classList.contains("is-locked")) label(k > 0.6);
      raf = on ? requestAnimationFrame(tick) : 0;
    };
    const io = new IntersectionObserver(([e]) => { on = e.isIntersecting; if (on && !raf) { last = performance.now(); raf = requestAnimationFrame(tick); } });
    io.observe(el);
    const onResize = () => { measure(); if (!user) { tx = sub.x; ty = sub.y; } };
    addEventListener("resize", onResize);
    if (fine) el.parentElement!.addEventListener("pointermove", move, { passive: true });
    else el.parentElement!.addEventListener("pointerdown", tap, { passive: true });
    set(x, y, 0, 0);
    return () => {
      io.disconnect(); cancelAnimationFrame(raf); removeEventListener("resize", onResize);
      el.parentElement?.removeEventListener("pointermove", move); el.parentElement?.removeEventListener("pointerdown", tap);
    };
  }, [quiet, lang]);
  return (
    <div ref={box} className={`v-bd v-focus${quiet ? " is-quiet" : ""}`} aria-hidden>
      <img className="v-focus-soft" src="/home/peak-soft.webp" alt="" />
      <img className="v-focus-sharp" src="/home/peak-2400.webp" srcSet="/home/peak-1200.webp 1200w, /home/peak-2400.webp 2400w" sizes="100vw" alt="" fetchPriority={quiet ? "auto" : "high"} loading={quiet ? "lazy" : "eager"} />
      {!quiet && (
        <div className="v-vf">
          <i /><i /><i /><i />
          <span className="v-vf-dot" />
          <span className="v-vf-star"><Gleam size={16} /></span>
          <span className="v-vf-read v-mono" ref={read}>{t.seeking}</span>
        </div>
      )}
    </div>
  );
}
