import { useEffect, useRef, type RefObject } from "react";
import { addScene, clamp, lerp, localProg, reduced, smooth } from "../scroll";

/**
 * 隐私那一屏右边的小装置：一台线框手机罩在玻璃穹顶里。
 * - 九张示例照片沿两条倾斜的轨道绕着手机转；随正文逐字点亮（和正文用同一个滚动进度），一张张收拢进手机屏幕。
 * - 罩里有光点在游，碰到穹顶就弹回，碰到的地方亮一小段琥珀色光弧——照片出不去。
 * 一块 Canvas 2D 画完，不在视野里就停；减少动态效果时只画收拢好的一帧。照片颜色不动。
 */
const IDS = [416, 547, 651, 902, 918, 872, 231, 806, 1018];
type Tile = { img: HTMLImageElement; ring: 0 | 1; a0: number };
type Mote = { x: number; y: number; vx: number; vy: number; warm: boolean };
type Ripple = { a: number; t: number };

export default function Vault({ follow }: { follow: RefObject<HTMLElement | null> }) {
  const cv = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = cv.current!, g = c.getContext("2d")!;
    let W = 0, H = 0, R = 0, cx = 0, cy = 0;
    const size = () => {
      const dpr = Math.min(2, devicePixelRatio || 1);
      W = c.clientWidth; H = c.clientHeight; R = Math.min(W, H) * 0.44; cx = W / 2; cy = H / 2;
      c.width = Math.round(W * dpr); c.height = Math.round(H * dpr); g.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    size();
    // 颜色跟主题走：线条用正文色，强调用琥珀色
    let ink = "#f2f0ea", amber = "#e8b45c", dark = true;
    const colors = () => {
      const cs = getComputedStyle(c);
      ink = cs.color; amber = cs.getPropertyValue("--amber").trim() || amber;
      // 正文色是浅的就是深色主题
      const m = ink.match(/\d+/g)?.map(Number) ?? [242, 240, 234];
      dark = (m[0] * 0.3 + m[1] * 0.59 + m[2] * 0.11) > 128;
    };
    colors();
    const rgba = (col: string, a: number) => {
      const m = col.match(/\d+(\.\d+)?/g);
      if (col.startsWith("#")) { const n = parseInt(col.slice(1), 16); return `rgba(${n >> 16},${(n >> 8) & 255},${n & 255},${a})`; }
      return m ? `rgba(${m[0]},${m[1]},${m[2]},${a})` : col;
    };

    const tiles: Tile[] = IDS.map((id, i) => { const img = new Image(); img.src = `/home/wall/${id}.webp`; return { img, ring: (i % 2) as 0 | 1, a0: (i / IDS.length) * Math.PI * 2 }; });
    let seed = 5;
    const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
    const motes: Mote[] = Array.from({ length: 34 }, () => {
      const a = rnd() * Math.PI * 2, r = Math.sqrt(rnd()) * 0.85, s = 14 + rnd() * 22, d = rnd() * Math.PI * 2;
      return { x: Math.cos(a) * r, y: Math.sin(a) * r, vx: Math.cos(d) * s, vy: Math.sin(d) * s, warm: rnd() < 0.3 };
    });
    const ripples: Ripple[] = [];
    // 两条轨道：半长轴、半短轴（相对穹顶半径）、倾角、转向
    const RINGS = [{ rx: 0.78, ry: 0.28, tilt: -0.32, dir: 1 }, { rx: 0.66, ry: 0.5, tilt: 0.42, dir: -1 }];

    let p = 0, raf = 0, on = false, last = performance.now(), time = 0;

    const rr = (x: number, y: number, w: number, h: number, r: number) => { g.beginPath(); g.roundRect(x, y, w, h, r); };
    const photo = (t: Tile, x: number, y: number, w: number, h: number, a: number, keep = 0) => {
      if (!t.img.complete || !t.img.naturalWidth) return;
      // 按格子比例从照片中间裁一块，照片本身不变形
      const iw = t.img.naturalWidth, ih = t.img.naturalHeight, k = Math.min(iw / w, ih / h), sw = w * k, sh = h * k;
      g.save(); g.globalAlpha = a; rr(x, y, w, h, Math.min(4, w * 0.08)); g.clip();
      g.drawImage(t.img, (iw - sw) / 2, (ih - sh) / 2, sw, sh, x, y, w, h); g.restore();
      if (keep > 0.01) {
        g.save(); g.globalAlpha = keep; g.strokeStyle = amber; g.lineWidth = 1.5; g.shadowColor = amber; g.shadowBlur = 10;
        rr(x, y, w, h, Math.min(4, w * 0.08)); g.stroke(); g.restore();
      }
    };

    const draw = (dt: number) => {
      time += dt;
      g.clearRect(0, 0, W, H);
      const lock = smooth(0.55, 1, p);
      // 穹顶：很淡的内光 + 细边 + 一段慢慢转的高光弧
      const fill = g.createRadialGradient(cx, cy, R * 0.1, cx, cy, R);
      fill.addColorStop(0, rgba(amber, (dark ? 0.05 : 0.07) + 0.06 * lock));
      fill.addColorStop(1, rgba(amber, 0));
      g.fillStyle = fill; g.beginPath(); g.arc(cx, cy, R, 0, Math.PI * 2); g.fill();
      g.lineWidth = 1; g.strokeStyle = rgba(ink, dark ? 0.22 : 0.2); g.stroke();
      g.setLineDash([2, 6]); g.strokeStyle = rgba(ink, 0.12); g.beginPath(); g.arc(cx, cy, R * 0.93, 0, Math.PI * 2); g.stroke(); g.setLineDash([]);
      const sweep = time * 0.35;
      g.save(); g.strokeStyle = rgba(amber, 0.35 + 0.5 * lock); g.lineWidth = 1.6; g.shadowColor = amber; g.shadowBlur = 14;
      g.beginPath(); g.arc(cx, cy, R, sweep, sweep + 0.7 + 0.9 * lock); g.stroke(); g.restore();

      // 轨道虚线
      for (const ring of RINGS) {
        g.save(); g.translate(cx, cy); g.rotate(ring.tilt); g.strokeStyle = rgba(ink, 0.1 * (1 - lock * 0.7)); g.lineWidth = 1;
        g.beginPath(); g.ellipse(0, 0, ring.rx * R, ring.ry * R, 0, 0, Math.PI * 2); g.stroke(); g.restore();
      }

      // 手机线框
      const pw = R * 0.46, ph = R * 0.92, px = cx - pw / 2, py = cy - ph / 2, inset = R * 0.04;
      const sx = px + inset, sy = py + inset * 2.2, sw = pw - inset * 2, gap = R * 0.018, cell = (sw - gap * 2) / 3;

      // 每张照片现在的位置：轨道上 → 屏幕里的九宫格
      const items = tiles.map((t, i) => {
        const ring = RINGS[t.ring], ang = t.a0 + time * 0.22 * ring.dir;
        const ox = Math.cos(ang) * ring.rx * R, oy = Math.sin(ang) * ring.ry * R;
        const x0 = cx + ox * Math.cos(ring.tilt) - oy * Math.sin(ring.tilt), y0 = cy + ox * Math.sin(ring.tilt) + oy * Math.cos(ring.tilt);
        const depth = Math.sin(ang) * ring.dir;
        const w0 = R * 0.29 * (0.82 + 0.22 * depth), h0 = w0 / 1.5;
        const gx = sx + (i % 3) * (cell + gap), gy = sy + Math.floor(i / 3) * (cell + gap);
        const e = smooth(0.18 + i * 0.045, 0.52 + i * 0.045, p);
        return {
          t, i, e, front: e > 0.5 || depth > 0,
          x: lerp(x0 - w0 / 2, gx, e), y: lerp(y0 - h0 / 2, gy, e), w: lerp(w0, cell, e), h: lerp(h0, cell, e),
          a: lerp(0.55 + 0.45 * clamp(depth + 0.5, 0, 1), 1, e),
        };
      });
      for (const it of items) if (!it.front) photo(it.t, it.x, it.y, it.w, it.h, it.a * 0.8);

      g.save();
      g.fillStyle = dark ? "rgba(8,8,10,.55)" : "rgba(255,255,255,.55)";
      rr(px, py, pw, ph, R * 0.07); g.fill();
      g.lineWidth = 1.5; g.strokeStyle = rgba(ink, dark ? 0.7 : 0.6); g.stroke();
      rr(cx - pw * 0.13, py + inset * 0.7, pw * 0.26, inset * 0.6, inset * 0.3); g.fillStyle = rgba(ink, 0.35); g.fill();
      g.restore();

      for (const it of items) if (it.front) photo(it.t, it.x, it.y, it.w, it.h, it.a, it.i === 4 ? smooth(0.85, 1, p) : 0);
      // 留下的那一张上一颗小星
      const best = items[4];
      if (best.e > 0.98) {
        const k = smooth(0.85, 1, p), s = cell * 0.16, x = best.x + best.w - s * 1.4, y = best.y + s * 1.4;
        g.save(); g.globalAlpha = k; g.fillStyle = amber; g.beginPath();
        for (let j = 0; j < 8; j++) { const a = (j * Math.PI) / 4, r2 = j % 2 ? s * 0.3 : s; g.lineTo(x + Math.sin(a) * r2, y - Math.cos(a) * r2); }
        g.closePath(); g.fill(); g.restore();
      }
      // 屏幕下半截：两条细线当界面
      g.fillStyle = rgba(ink, 0.18 * smooth(0.6, 1, p));
      g.fillRect(sx, sy + cell * 3 + gap * 3 + inset, sw * 0.7, 2); g.fillRect(sx, sy + cell * 3 + gap * 3 + inset * 2, sw * 0.45, 2);

      // 罩里的光点：碰到穹顶就弹回，留一段光弧
      const lim = 0.97;
      for (const m of motes) {
        m.x += (m.vx * dt) / R; m.y += (m.vy * dt) / R;
        const d = Math.hypot(m.x, m.y);
        if (d > lim) {
          const nx = m.x / d, ny = m.y / d, dot = m.vx * nx + m.vy * ny;
          if (dot > 0) { m.vx -= 2 * dot * nx; m.vy -= 2 * dot * ny; ripples.push({ a: Math.atan2(ny, nx), t: time }); }
          m.x = nx * lim; m.y = ny * lim;
        }
        g.fillStyle = m.warm ? rgba(amber, 0.8) : rgba(ink, dark ? 0.55 : 0.4);
        g.beginPath(); g.arc(cx + m.x * R, cy + m.y * R, m.warm ? 1.6 : 1.1, 0, Math.PI * 2); g.fill();
      }
      for (let i = ripples.length - 1; i >= 0; i--) {
        const age = (time - ripples[i].t) / 0.9;
        if (age >= 1) { ripples.splice(i, 1); continue; }
        const a = ripples[i].a, span = 0.12 + age * 0.22;
        g.save(); g.strokeStyle = rgba(amber, 0.85 * (1 - age)); g.lineWidth = 2; g.shadowColor = amber; g.shadowBlur = 10;
        g.beginPath(); g.arc(cx, cy, R, a - span, a + span); g.stroke(); g.restore();
      }
    };

    const loop = (t: number) => { const dt = Math.min(0.05, (t - last) / 1000); last = t; draw(dt); raf = on ? requestAnimationFrame(loop) : 0; };
    const onResize = () => { size(); colors(); if (!on) draw(0); };
    addEventListener("resize", onResize);
    // 主题切换时重新取色
    const mo = new MutationObserver(() => { colors(); if (!on) draw(0); });
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    if (reduced()) {
      p = 1;
      Promise.all(tiles.map((t) => t.img.decode().catch(() => {}))).then(() => draw(0));
      return () => { removeEventListener("resize", onResize); mo.disconnect(); };
    }
    // 宽屏和正文逐字点亮用同一个进度；窄屏装置在文字下面，改跟着装置自己经过窗口的进度，免得滚到时已经收拢完
    const wide = matchMedia("(min-width: 1024px)").matches;
    const stop = addScene(wide ? follow.current! : c, (el) => localProg(el, wide ? 0.85 : 1, wide ? 0.55 : 0.9), (q) => { p = q; });
    const io = new IntersectionObserver(([e]) => { on = e.isIntersecting; if (on && !raf) { last = performance.now(); raf = requestAnimationFrame(loop); } });
    io.observe(c);
    return () => { stop(); io.disconnect(); cancelAnimationFrame(raf); removeEventListener("resize", onResize); mo.disconnect(); };
  }, [follow]);
  return <canvas ref={cv} className="v-vault" aria-hidden />;
}
