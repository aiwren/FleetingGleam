/**
 * 首屏以下几段共用的滚动引擎（照新设计稿）：每段一个 0 到 1 的进度，画面带一点惯性跟上，静止就停帧。
 * 每段在自己的 effect 里登记，卸载时注销；窗口尺寸变了先让各段清掉自己量好的布局，再统一重画。
 */
export const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));
export const smooth = (a: number, b: number, t: number) => { const x = clamp((t - a) / (b - a), 0, 1); return x * x * (3 - 2 * x); };
export const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
export const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

/** 钉住的一段：从这一段顶到窗口顶，到这一段底到窗口底。 */
export const pinProg = (el: HTMLElement) => { const r = el.getBoundingClientRect(), span = r.height - innerHeight; return span > 0 ? clamp(-r.top / span, 0, 1) : (r.top < 0 ? 1 : 0); };
/** 普通的一段：从刚露出窗口底，到完全离开窗口顶。 */
export const viewProg = (el: HTMLElement) => { const r = el.getBoundingClientRect(); return clamp((innerHeight - r.top) / (innerHeight + r.height), 0, 1); };
/** 窄屏上不钉住时，一块演示自己经过窗口的进度。 */
export const localProg = (el: HTMLElement, a = 0.9, b = 0.55) => { const r = el.getBoundingClientRect(); return clamp((innerHeight * a - r.top) / (innerHeight * b), 0, 1); };

type Scene = { el: HTMLElement; prog: (el: HTMLElement) => number; render: (p: number, v: number) => void; t: number; n: number; v: number; last: number };
const scenes = new Set<Scene>();
let raf = 0, bound = false;

function frame() {
  let moving = false;
  for (const s of scenes) {
    if (s.n < 0) s.n = s.t;
    s.n += (s.t - s.n) * 0.12;
    if (Math.abs(s.t - s.n) < 0.0004) s.n = s.t; else moving = true;
    s.v = s.t - s.n;
    if (s.n !== s.last) { s.last = s.n; s.render(s.n, s.v); }
  }
  raf = moving ? requestAnimationFrame(frame) : 0;
}
const kick = () => { for (const s of scenes) s.t = s.prog(s.el); if (!raf) raf = requestAnimationFrame(frame); };
/** 布局变了（窗口尺寸、断点）：每段都按当前进度重画一次。 */
const forceRender = () => { for (const s of scenes) { s.t = s.prog(s.el); s.last = -2; } if (!raf) raf = requestAnimationFrame(frame); };

/** 登记一段；返回注销函数。减少动态效果时各段自己画最终状态，不登记。 */
export function addScene(el: HTMLElement, prog: Scene["prog"], render: Scene["render"]) {
  const s: Scene = { el, prog, render, t: prog(el), n: -1, v: 0, last: -2 };
  scenes.add(s);
  if (!bound) { bound = true; addEventListener("scroll", kick, { passive: true }); addEventListener("resize", forceRender); }
  if (!raf) raf = requestAnimationFrame(frame);
  return () => { scenes.delete(s); };
}

/** 进场：元素进到窗口里一点，就加上 is-in，里面的 .ln / .rise 开始动。 */
export function reveal(el: HTMLElement) {
  const io = new IntersectionObserver((entries) => entries.forEach((en) => {
    if (en.isIntersecting) { en.target.classList.add("is-in"); io.unobserve(en.target); }
  }), { rootMargin: "0px 0px -15% 0px" });
  io.observe(el);
  return () => io.disconnect();
}
