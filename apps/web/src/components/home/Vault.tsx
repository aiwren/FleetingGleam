import { useEffect, useRef } from "react";
import type { Lang } from "../../content";
import { addScene, clamp, lerp, localProg, reduced, smooth } from "../scroll";
import { home } from "./copy";

/**
 * 隐私那一屏右边的主体：一台手机罩在玻璃穹顶里，滚动讲完"在本机处理"的整个过程。
 * 1. 九张示例照片在穹顶里绕着手机转，随后一张张飞进屏幕；
 * 2. 屏幕里按连拍归成三组，一道扫描线扫过，每张跳出评分，每组最好的那张亮起琥珀边和星芒（其余的只变淡，颜色不动）；
 * 3. 状态从"本机分析中"变成"完成"，底部的上传计数始终是 0；
 * 一直在发生的：穹顶外一朵划掉的云，几条网络虚线想连进来，碰到穹顶就断开、断口闪一下。
 * 一块 Canvas 2D，不在视野里就停；减少动态效果时只画处理完的一帧。
 */
const IDS = [416, 547, 651, 902, 918, 872, 231, 806, 1018];
// 每张的评分；三行是三组连拍，每组最高的那张留下
const SCORES = [71, 88, 64, 92, 79, 83, 58, 86, 74];
const BEST = new Set([1, 3, 7]);

export default function Vault({ lang }: { lang: Lang }) {
  const cv = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const t = home[lang].vault;
    const c = cv.current!, g = c.getContext("2d")!;
    let W = 0, H = 0, R = 0, cx = 0, cy = 0;
    const size = () => {
      const dpr = Math.min(2, devicePixelRatio || 1);
      W = c.clientWidth; H = c.clientHeight; R = Math.min(W, H) * 0.43; cx = W * 0.5; cy = H * 0.52;
      c.width = Math.round(W * dpr); c.height = Math.round(H * dpr); g.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    size();
    // 颜色跟主题走：线条用正文色，强调用琥珀色
    let ink = [242, 240, 234], amber = [232, 180, 92], dark = true, font = "sans-serif", mono = "monospace";
    const colors = () => {
      const cs = getComputedStyle(c);
      ink = (cs.color.match(/\d+/g) ?? ["242", "240", "234"]).slice(0, 3).map(Number);
      const a = cs.getPropertyValue("--amber").trim();
      if (a.startsWith("#")) { const n = parseInt(a.slice(1), 16); amber = [n >> 16, (n >> 8) & 255, n & 255]; }
      dark = ink[0] * 0.3 + ink[1] * 0.59 + ink[2] * 0.11 > 128;
      font = cs.fontFamily; mono = cs.getPropertyValue("--font-mono").trim() || "monospace";
    };
    colors();
    const I = (a: number) => `rgba(${ink[0]},${ink[1]},${ink[2]},${a})`;
    const A = (a: number) => `rgba(${amber[0]},${amber[1]},${amber[2]},${a})`;

    const imgs = IDS.map((id) => { const im = new Image(); im.src = `/home/wall/${id}.webp`; return im; });
    let p = 0, raf = 0, on = false, last = performance.now(), time = 0;

    const rr = (x: number, y: number, w: number, h: number, r: number) => { g.beginPath(); g.roundRect(x, y, w, h, r); };
    const photo = (im: HTMLImageElement, x: number, y: number, w: number, h: number, a: number) => {
      if (!im.complete || !im.naturalWidth || w < 1) return;
      const iw = im.naturalWidth, ih = im.naturalHeight, k = Math.min(iw / w, ih / h), sw = w * k, sh = h * k;
      g.save(); g.globalAlpha = a; rr(x, y, w, h, Math.min(5, w * 0.08)); g.clip();
      g.drawImage(im, (iw - sw) / 2, (ih - sh) / 2, sw, sh, x, y, w, h); g.restore();
    };
    const star = (x: number, y: number, s: number) => {
      g.beginPath();
      for (let j = 0; j < 8; j++) { const a = (j * Math.PI) / 4, r2 = j % 2 ? s * 0.3 : s; g.lineTo(x + Math.sin(a) * r2, y - Math.cos(a) * r2); }
      g.closePath(); g.fill();
    };
    // 划掉的云
    const cloud = (x: number, y: number, s: number) => {
      g.save(); g.translate(x, y); g.strokeStyle = I(dark ? 0.55 : 0.5); g.lineWidth = 1.5; g.lineJoin = "round";
      g.beginPath();
      g.moveTo(-s * 0.9, s * 0.35); g.arc(-s * 0.55, s * 0.05, s * 0.35, Math.PI * 0.6, Math.PI * 1.5);
      g.arc(-s * 0.05, -s * 0.2, s * 0.5, Math.PI * 1.1, Math.PI * 1.95); g.arc(s * 0.55, s * 0.08, s * 0.32, Math.PI * 1.45, Math.PI * 0.45);
      g.closePath(); g.stroke();
      g.strokeStyle = A(0.9); g.lineWidth = 1.8; g.beginPath(); g.moveTo(-s * 0.95, -s * 0.75); g.lineTo(s * 0.95, s * 0.75); g.stroke();
      g.restore();
    };

    const draw = (dt: number) => {
      time += dt;
      g.clearRect(0, 0, W, H);
      const fly = (i: number) => smooth(0.08 + i * 0.03, 0.34 + i * 0.03, p);
      const group = smooth(0.36, 0.5, p), scan = smooth(0.5, 0.78, p), keep = smooth(0.78, 0.92, p);

      // 穹顶
      const fill = g.createRadialGradient(cx, cy, R * 0.15, cx, cy, R);
      fill.addColorStop(0, A(dark ? 0.12 : 0.1)); fill.addColorStop(0.7, A(dark ? 0.05 : 0.04)); fill.addColorStop(1, A(0));
      g.fillStyle = fill; g.beginPath(); g.arc(cx, cy, R, 0, Math.PI * 2); g.fill();
      g.lineWidth = 1.2; g.strokeStyle = I(dark ? 0.32 : 0.28); g.stroke();
      const sweep = time * 0.3;
      g.save(); g.strokeStyle = A(0.75); g.lineWidth = 2; g.shadowColor = A(1); g.shadowBlur = 16;
      g.beginPath(); g.arc(cx, cy, R, sweep, sweep + 1.1); g.stroke(); g.restore();

      // 穹顶外：云和几条想连进来的网络虚线，碰到穹顶断开
      // 云放在穹顶右上外侧，贴着画布边也不出界
      const cs = R * 0.15, cloudX = Math.min(cx + R * 1.0, W - cs * 1.2), cloudY = Math.max(cy - R * 0.98, cs * 1.1);
      cloud(cloudX, cloudY, cs);
      const lines = [{ from: [cloudX - cs * 0.6, cloudY + cs * 0.6], ang: -0.78 }, { from: [cx - R * 1.25, cy - R * 0.55], ang: -2.7 }, { from: [cx + R * 1.28, cy + R * 0.42], ang: 0.33 }];
      lines.forEach((ln, k) => {
        const tx = cx + Math.cos(ln.ang) * R, ty = cy + Math.sin(ln.ang) * R;
        const [fx, fy] = ln.from;
        const ph = (time * 0.45 + k * 0.33) % 1; // 每条线一个周期：伸过来 → 碰到穹顶 → 断开淡掉
        const reach = smooth(0, 0.55, ph), fade = 1 - smooth(0.6, 1, ph);
        const ex = lerp(fx, tx, reach), ey = lerp(fy, ty, reach);
        g.save(); g.setLineDash([3, 5]); g.lineDashOffset = -time * 20; g.strokeStyle = I((dark ? 0.35 : 0.3) * fade); g.lineWidth = 1.2;
        g.beginPath(); g.moveTo(fx, fy); g.lineTo(ex, ey); g.stroke(); g.restore();
        if (reach > 0.99) {
          // 断口：一小段琥珀光弧和一个小叉
          const k2 = 1 - smooth(0.55, 0.85, ph);
          g.save(); g.strokeStyle = A(0.9 * k2); g.lineWidth = 2.2; g.shadowColor = A(1); g.shadowBlur = 12;
          g.beginPath(); g.arc(cx, cy, R, ln.ang - 0.13, ln.ang + 0.13); g.stroke();
          const ox = tx + Math.cos(ln.ang) * 10, oy = ty + Math.sin(ln.ang) * 10, s = 4;
          g.lineWidth = 1.6; g.shadowBlur = 0; g.beginPath(); g.moveTo(ox - s, oy - s); g.lineTo(ox + s, oy + s); g.moveTo(ox + s, oy - s); g.lineTo(ox - s, oy + s); g.stroke();
          g.restore();
        }
      });

      // 手机
      const pw = R * 0.9, ph = R * 1.56, px = cx - pw / 2, py = cy - ph / 2, pad = R * 0.05;
      const sx = px + pad, sw = pw - pad * 2;
      const headY = py + pad * 2.4, gridY = headY + R * 0.17;
      const gap = R * 0.03, cell = (sw - gap * 2) / 3, rowGap = lerp(gap, gap * 3.6, group);
      // 三行照片铺满屏幕中间（顶上状态、底下计数之间）
      const footY = py + ph - pad * 2.2, ch = Math.min(cell * 1.15, (footY - R * 0.1 - gridY - gap * 3.6 * 2) / 3);
      const rowY = (r: number) => gridY + r * (ch + rowGap);

      // 还没飞进来的照片：在穹顶里绕着转（背面的先画）
      const orbit = imgs.map((im, i) => {
        const ring = i % 2 ? { rx: 0.84, ry: 0.32, tilt: 0.4, dir: -1 } : { rx: 0.9, ry: 0.26, tilt: -0.3, dir: 1 };
        const ang = (i / 9) * Math.PI * 2 + time * 0.25 * ring.dir;
        const ox = Math.cos(ang) * ring.rx * R, oy = Math.sin(ang) * ring.ry * R;
        const depth = Math.sin(ang) * ring.dir;
        return { im, i, depth, x: cx + ox * Math.cos(ring.tilt) - oy * Math.sin(ring.tilt), y: cy + ox * Math.sin(ring.tilt) + oy * Math.cos(ring.tilt), w: R * 0.3 * (0.85 + 0.2 * depth) };
      });
      const place = (o: (typeof orbit)[number]) => {
        const e = fly(o.i), col = o.i % 3, row = Math.floor(o.i / 3);
        const w = lerp(o.w, cell, e), h = lerp(o.w / 1.5, ch, e);
        return { x: lerp(o.x - o.w / 2, sx + col * (cell + gap), e), y: lerp(o.y - o.w / 3, rowY(row), e), w, h, e };
      };
      for (const o of orbit) { const q = place(o); if (q.e < 0.5 && o.depth < 0) photo(o.im, q.x, q.y, q.w, q.h, 0.7); }

      g.save();
      g.shadowColor = dark ? "rgba(0,0,0,.6)" : "rgba(20,20,30,.18)"; g.shadowBlur = 40; g.shadowOffsetY = 18;
      g.fillStyle = dark ? "rgba(14,14,16,.92)" : "rgba(255,255,255,.92)"; rr(px, py, pw, ph, R * 0.1); g.fill(); g.restore();
      g.lineWidth = 1.6; g.strokeStyle = I(dark ? 0.75 : 0.55); rr(px, py, pw, ph, R * 0.1); g.stroke();
      rr(cx - pw * 0.12, py + pad * 0.8, pw * 0.24, pad * 0.55, pad * 0.3); g.fillStyle = I(0.4); g.fill();

      // 屏幕顶上的状态
      const done = keep > 0.5;
      const dotX = sx + R * 0.02, labelY = headY + R * 0.03;
      g.beginPath(); g.arc(dotX, labelY, R * 0.016, 0, Math.PI * 2);
      g.fillStyle = done ? A(1) : A(0.5 + 0.5 * Math.abs(Math.sin(time * 3))); g.fill();
      g.font = `500 ${Math.max(10, R * 0.062)}px ${font}`; g.textBaseline = "middle";
      g.fillStyle = I(0.85); g.fillText(done ? t.done : t.working, dotX + R * 0.045, labelY);
      g.font = `${Math.max(9, R * 0.05)}px ${mono}`; g.fillStyle = I(0.45);
      g.textAlign = "right"; g.fillText(t.onDevice, sx + sw, labelY); g.textAlign = "left";

      // 屏幕里的照片：分三组，扫描线扫过后跳出评分，每组最好的留下
      const scanY = lerp(gridY - R * 0.05, rowY(2) + ch + R * 0.05, scan);
      for (const o of orbit) {
        const q = place(o);
        if (q.e < 0.5 && o.depth < 0) continue;
        const inScreen = q.e > 0.98, scanned = inScreen && scan > 0 && scanY > q.y + q.h * 0.6;
        const best = BEST.has(o.i);
        photo(o.im, q.x, q.y, q.w, q.h, inScreen && !best ? 1 - 0.45 * keep : 1);
        if (best && keep > 0.01) {
          g.save(); g.strokeStyle = A(keep); g.lineWidth = 1.8; g.shadowColor = A(1); g.shadowBlur = 12;
          rr(q.x, q.y, q.w, q.h, Math.min(5, q.w * 0.08)); g.stroke(); g.restore();
          g.fillStyle = A(keep); star(q.x + q.w - R * 0.04, q.y + R * 0.04, R * 0.028);
        }
        if (scanned) {
          // 评分小牌子
          const bw = R * 0.105, bh = R * 0.055, bx = q.x + R * 0.012, by = q.y + q.h - bh - R * 0.012, hot = best && keep > 0.5;
          g.fillStyle = hot ? A(0.95) : "rgba(10,10,12,.72)"; rr(bx, by, bw, bh, bh / 2); g.fill();
          g.font = `600 ${Math.max(9, R * 0.04)}px ${mono}`; g.textAlign = "center";
          g.fillStyle = hot ? "#0b0b0c" : "#f2f0ea"; g.fillText(String(SCORES[o.i]), bx + bw / 2, by + bh / 2 + 0.5); g.textAlign = "left";
        }
      }
      // 组标签
      if (group > 0.01) {
        g.font = `${Math.max(9, R * 0.045)}px ${mono}`; g.fillStyle = I(0.45 * group);
        for (let r = 0; r < 3; r++) g.fillText(`${t.group} ${r + 1}`, sx, rowY(r) - rowGap * 0.5);
      }
      // 扫描线
      if (scan > 0.001 && scan < 0.999) {
        const grad = g.createLinearGradient(0, scanY - R * 0.08, 0, scanY);
        grad.addColorStop(0, A(0)); grad.addColorStop(1, A(0.28));
        g.fillStyle = grad; g.fillRect(sx - 2, scanY - R * 0.08, sw + 4, R * 0.08);
        g.save(); g.strokeStyle = A(0.95); g.lineWidth = 1.5; g.shadowColor = A(1); g.shadowBlur = 10;
        g.beginPath(); g.moveTo(sx - 2, scanY); g.lineTo(sx + sw + 2, scanY); g.stroke(); g.restore();
      }
      // 屏幕底部：留下几张、上传 0 张
      g.font = `${Math.max(9, R * 0.05)}px ${mono}`; g.fillStyle = I(0.5);
      g.fillText(t.kept(keep > 0.5 ? 3 : 0), sx, footY);
      g.textAlign = "right"; g.fillStyle = A(0.9); g.fillText(t.uploaded, sx + sw, footY); g.textAlign = "left";
    };

    const loop = (tm: number) => { const dt = Math.min(0.05, (tm - last) / 1000); last = tm; draw(dt); raf = on ? requestAnimationFrame(loop) : 0; };
    const onResize = () => { size(); colors(); if (!on) draw(0); };
    addEventListener("resize", onResize);
    const mo = new MutationObserver(() => { colors(); if (!on) draw(0); });
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    if (reduced()) {
      p = 1; time = 2.2;
      Promise.all(imgs.map((im) => im.decode().catch(() => {}))).then(() => draw(0));
      return () => { removeEventListener("resize", onResize); mo.disconnect(); };
    }
    // 跟着装置自己经过窗口：从露出底边开始，到它大约居中时处理完
    const stop = addScene(c, (el) => localProg(el, 1, 0.85), (q) => { p = clamp(q, 0, 1); });
    const io = new IntersectionObserver(([e]) => { on = e.isIntersecting; if (on && !raf) { last = performance.now(); raf = requestAnimationFrame(loop); } });
    io.observe(c);
    return () => { stop(); io.disconnect(); cancelAnimationFrame(raf); removeEventListener("resize", onResize); mo.disconnect(); };
  }, [lang]);
  return <canvas ref={cv} className="v-vault" aria-hidden />;
}
