import { useEffect, useRef } from "react";
import type { Lang } from "../../content";
import { addScene, clamp, lerp, reduced, smooth } from "../scroll";
import { home } from "./copy";

/**
 * 隐私那一屏右边的主体：一台开着飞行模式的深色手机，立在一圈发光的轨道中间。
 * 1. 九张示例照片在轨道上绕着手机转（转到后面就被手机挡住），滚动时一张张被吸进屏幕；
 * 2. 屏幕里按连拍归成三组，一道扫描线扫过，每张跳出评分，每组最好的那张亮起琥珀边和星芒（其余的只变淡，颜色不动）；
 * 3. 状态从"本机分析中"变成"完成"；
 * 一直在发生的：轨道上一圈细光点慢慢转，两块玻璃标签"上传 0 张""断网也能用"轻轻浮动——照片从头到尾没离开这一圈。
 * 一块 Canvas 2D，不在视野里就停；减少动态效果时只画处理完的一帧。
 */
const IDS = [416, 547, 651, 902, 918, 872, 231, 806, 1018];
// 每张的评分；三行是三组连拍，每组最高的那张留下
const SCORES = [71, 88, 64, 92, 79, 83, 58, 86, 74];
const BEST = new Set([1, 3, 7]);
// 轨道上的光点：固定的随机角度、半径偏移、亮度
const MOTES = Array.from({ length: 70 }, (_, i) => { const r = Math.sin(i * 91.7) * 43758.5; const f = r - Math.floor(r); return { a: (i / 70) * Math.PI * 2 + f, k: 0.9 + f * 0.22, s: 0.6 + ((i * 7) % 5) * 0.25, b: 0.3 + ((i * 13) % 7) / 10 }; });

export default function Vault({ lang }: { lang: Lang }) {
  const cv = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const t = home[lang].vault;
    const c = cv.current!, g = c.getContext("2d")!;
    let W = 0, H = 0;
    const size = () => {
      const dpr = Math.min(2, devicePixelRatio || 1);
      W = c.clientWidth; H = c.clientHeight;
      c.width = Math.round(W * dpr); c.height = Math.round(H * dpr); g.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    size();
    // 线条、标签跟主题走；手机本身始终是深色的
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
    // 屏幕里固定用深色界面上的颜色
    const S = (a: number) => `rgba(242,240,234,${a})`;
    const SA = (a: number) => `rgba(232,180,92,${a})`;

    const imgs = IDS.map((id) => { const im = new Image(); im.src = `/home/wall/${id}.webp`; return im; });
    let p = 0, raf = 0, on = false, last = performance.now(), time = 0;

    const rr = (x: number, y: number, w: number, h: number, r: number) => { g.beginPath(); g.roundRect(x, y, w, h, r); };
    const photo = (im: HTMLImageElement, x: number, y: number, w: number, h: number, a: number, r: number) => {
      if (!im.complete || !im.naturalWidth || w < 1) return;
      const iw = im.naturalWidth, ih = im.naturalHeight, k = Math.min(iw / w, ih / h), sw = w * k, sh = h * k;
      g.save(); g.globalAlpha = a; rr(x, y, w, h, r); g.clip();
      g.drawImage(im, (iw - sw) / 2, (ih - sh) / 2, sw, sh, x, y, w, h); g.restore();
    };
    const star = (x: number, y: number, s: number) => {
      g.beginPath();
      for (let j = 0; j < 8; j++) { const a = (j * Math.PI) / 4, r2 = j % 2 ? s * 0.3 : s; g.lineTo(x + Math.sin(a) * r2, y - Math.cos(a) * r2); }
      g.closePath(); g.fill();
    };
    // 飞机（飞行模式图标），朝上
    const plane = (x: number, y: number, s: number) => {
      g.save(); g.translate(x, y); g.scale(s / 10, s / 10); g.beginPath();
      g.moveTo(0, -10); g.quadraticCurveTo(1.6, -10, 1.6, -6); g.lineTo(1.6, -2); g.lineTo(9, 2.5); g.lineTo(9, 4.5); g.lineTo(1.6, 2.2);
      g.lineTo(1.6, 6.5); g.lineTo(4, 8.5); g.lineTo(4, 10); g.lineTo(0, 9); g.lineTo(-4, 10); g.lineTo(-4, 8.5); g.lineTo(-1.6, 6.5);
      g.lineTo(-1.6, 2.2); g.lineTo(-9, 4.5); g.lineTo(-9, 2.5); g.lineTo(-1.6, -2); g.lineTo(-1.6, -6); g.quadraticCurveTo(-1.6, -10, 0, -10);
      g.fill(); g.restore();
    };
    // 上传箭头加一道斜杠
    const noUpload = (x: number, y: number, s: number) => {
      g.save(); g.translate(x, y); g.lineCap = "round"; g.lineJoin = "round"; g.lineWidth = Math.max(1.4, s * 0.12);
      g.beginPath(); g.moveTo(0, s * 0.5); g.lineTo(0, -s * 0.45); g.moveTo(-s * 0.38, -s * 0.08); g.lineTo(0, -s * 0.48); g.lineTo(s * 0.38, -s * 0.08);
      g.moveTo(-s * 0.5, s * 0.6); g.lineTo(s * 0.5, s * 0.6); g.stroke();
      g.strokeStyle = A(1); g.beginPath(); g.moveTo(-s * 0.6, -s * 0.6); g.lineTo(s * 0.6, s * 0.6); g.stroke(); g.restore();
    };
    // 浮在手机旁边的玻璃标签
    const chip = (x: number, y: number, h: number, draw: (x: number, y: number, h: number) => number, w: number) => {
      g.save();
      g.shadowColor = dark ? "rgba(0,0,0,.5)" : "rgba(20,20,30,.14)"; g.shadowBlur = 24; g.shadowOffsetY = 10;
      g.fillStyle = dark ? "rgba(22,22,26,.72)" : "rgba(255,255,255,.82)"; rr(x, y, w, h, h / 2); g.fill(); g.restore();
      const edge = g.createLinearGradient(0, y, 0, y + h); edge.addColorStop(0, I(dark ? 0.28 : 0.16)); edge.addColorStop(1, I(dark ? 0.06 : 0.06));
      g.strokeStyle = edge; g.lineWidth = 1; rr(x + 0.5, y + 0.5, w - 1, h - 1, h / 2); g.stroke();
      draw(x, y, h);
    };

    const draw = (dt: number) => {
      time += dt;
      g.clearRect(0, 0, W, H);
      const fly = (i: number) => smooth(0.08 + i * 0.03, 0.3 + i * 0.03, p);
      const group = smooth(0.52, 0.64, p), scan = smooth(0.6, 0.84, p), keep = smooth(0.84, 0.96, p);
      const cx = W / 2;

      // 手机与轨道的尺寸
      const ph = Math.min(H * 0.8, W * 1.3), pw = ph * 0.5, px = cx - pw / 2, py = H * 0.05;
      const ox = cx, oy = py + ph * 0.86, rx = Math.min(W * 0.47, pw * 1.75), ry = rx * 0.24;
      const pr = pw * 0.15, bezel = pw * 0.035;
      const sx0 = px + bezel, sy0 = py + bezel, sw0 = pw - bezel * 2, sh0 = ph - bezel * 2, sr = pr - bezel;
      const pad = sw0 * 0.06, sx = sx0 + pad, sw = sw0 - pad * 2;

      // 背后的光晕
      const halo = g.createRadialGradient(cx, py + ph * 0.5, 0, cx, py + ph * 0.5, rx * 1.05);
      halo.addColorStop(0, A(dark ? 0.16 + 0.04 * Math.sin(time * 1.2) : 0.12)); halo.addColorStop(0.55, A(dark ? 0.05 : 0.04)); halo.addColorStop(1, A(0));
      g.fillStyle = halo; g.fillRect(0, 0, W, H);

      // 轨道：三圈细椭圆，外圈一段琥珀光在转
      const ring = (k: number, a: number) => { g.beginPath(); g.ellipse(ox, oy, rx * k, ry * k, 0, 0, Math.PI * 2); g.strokeStyle = I(a); g.lineWidth = 1; g.stroke(); };
      ring(0.62, dark ? 0.08 : 0.1); ring(0.82, dark ? 0.1 : 0.12); ring(1.02, dark ? 0.16 : 0.18);
      const floor = g.createRadialGradient(ox, oy, 0, ox, oy, rx);
      floor.addColorStop(0, A(dark ? 0.18 : 0.12)); floor.addColorStop(1, A(0));
      g.save(); g.translate(ox, oy); g.scale(1, ry / rx); g.fillStyle = floor; g.beginPath(); g.arc(0, 0, rx, 0, Math.PI * 2); g.fill(); g.restore();
      const sweep = time * 0.5;
      const arc = (back: boolean) => {
        // 只画在手机后面（上半圈）或前面（下半圈）的那部分，前后遮挡才对
        g.save(); g.beginPath(); g.rect(0, back ? 0 : oy, W, back ? oy : H - oy); g.clip();
        g.strokeStyle = A(0.9); g.lineWidth = 1.8; g.shadowColor = A(1); g.shadowBlur = 14;
        g.beginPath(); g.ellipse(ox, oy, rx * 1.02, ry * 1.02, 0, sweep, sweep + 1.3); g.stroke(); g.restore();
      };

      // 光点与照片在轨道上的位置；sin(角度) > 0 是转到了手机前面
      const motes = MOTES.map((m) => { const a = m.a + time * 0.12; return { x: ox + Math.cos(a) * rx * m.k, y: oy + Math.sin(a) * ry * m.k, front: Math.sin(a) > 0, m }; });
      const drawMotes = (front: boolean) => {
        for (const o of motes) if (o.front === front) { g.fillStyle = A(o.m.b * (front ? 1 : 0.6) * (0.6 + 0.4 * Math.sin(time * 2 + o.m.a * 5))); g.beginPath(); g.arc(o.x, o.y, o.m.s, 0, Math.PI * 2); g.fill(); }
      };
      const orbit = imgs.map((im, i) => {
        const a = (i / 9) * Math.PI * 2 + time * 0.22, depth = Math.sin(a);
        const w = pw * 0.38 * (0.75 + 0.25 * (depth + 1) / 2);
        return { im, i, depth, x: ox + Math.cos(a) * rx * 0.92, y: oy + depth * ry * 0.92 - w * 0.55, w };
      });

      // 屏幕里的布局：顶上状态栏和标题，中间三组连拍，底下计数
      const statusY = sy0 + sh0 * 0.045, headY = sy0 + sh0 * 0.13, footY = sy0 + sh0 - sh0 * 0.07;
      const gridY = headY + sh0 * 0.07, gap = sw * 0.04, cell = (sw - gap * 2) / 3;
      const labelH = sh0 * 0.045 * group;
      const ch = Math.min(cell * 1.3, (footY - sh0 * 0.05 - gridY - labelH * 3 - gap * 2) / 3);
      const rowY = (r: number) => gridY + labelH * (r + 1) + r * (ch + lerp(gap, gap * 0.6, group));
      const place = (o: (typeof orbit)[number]) => {
        const e = fly(o.i), col = o.i % 3, row = Math.floor(o.i / 3);
        return { x: lerp(o.x - o.w / 2, sx + col * (cell + gap), e), y: lerp(o.y, rowY(row), e), w: lerp(o.w, cell, e), h: lerp(o.w * 0.68, ch, e), e };
      };
      const outside = (o: (typeof orbit)[number], front: boolean) => {
        const q = place(o);
        if (q.e >= 0.999 || (o.depth > 0) !== front) return;
        // 飞进来的路上越来越亮、越来越小；后面那半圈的稍暗
        const a = (front ? 1 : 0.55) * (1 - smooth(0.85, 1, q.e)) + smooth(0.85, 1, q.e);
        g.save(); g.shadowColor = "rgba(0,0,0,.45)"; g.shadowBlur = 18; g.shadowOffsetY = 8;
        g.fillStyle = "rgba(0,0,0,.2)"; rr(q.x, q.y, q.w, q.h, q.w * 0.08); g.fill(); g.restore();
        photo(o.im, q.x, q.y, q.w, q.h, a, q.w * 0.08);
        g.strokeStyle = S(0.25 * (1 - q.e)); g.lineWidth = 1; rr(q.x + 0.5, q.y + 0.5, q.w - 1, q.h - 1, q.w * 0.08); g.stroke();
      };

      // 手机后面：后半圈的光点、光弧、照片
      drawMotes(false); arc(true);
      for (const o of orbit) outside(o, false);

      // 手机机身：深色金属边 + 玻璃高光
      g.save(); g.shadowColor = dark ? "rgba(0,0,0,.7)" : "rgba(20,20,30,.3)"; g.shadowBlur = 50; g.shadowOffsetY = 24;
      const body = g.createLinearGradient(px, py, px + pw, py + ph);
      body.addColorStop(0, "#3a3a40"); body.addColorStop(0.5, "#16161a"); body.addColorStop(1, "#2a2a30");
      g.fillStyle = body; rr(px, py, pw, ph, pr); g.fill(); g.restore();
      const screen = g.createLinearGradient(0, sy0, 0, sy0 + sh0);
      screen.addColorStop(0, "#121216"); screen.addColorStop(1, "#0a0a0c");
      g.fillStyle = screen; rr(sx0, sy0, sw0, sh0, sr); g.fill();
      // 灵动岛
      rr(cx - sw0 * 0.15, sy0 + sh0 * 0.018, sw0 * 0.3, sh0 * 0.032, sh0 * 0.016); g.fillStyle = "#000"; g.fill();

      g.save(); rr(sx0, sy0, sw0, sh0, sr); g.clip();
      // 状态栏：左边时间，右边飞行模式
      const fs = sw0 * 0.05;
      g.font = `600 ${fs}px ${font}`; g.textBaseline = "middle"; g.fillStyle = S(0.9);
      g.fillText("9:41", sx + sw0 * 0.02, statusY);
      g.fillStyle = SA(1); plane(sx + sw - fs * 0.4, statusY, fs * 0.55);
      // 标题
      const done = keep > 0.5;
      g.beginPath(); g.arc(sx + fs * 0.35, headY, fs * 0.28, 0, Math.PI * 2);
      g.fillStyle = done ? SA(1) : SA(0.4 + 0.6 * Math.abs(Math.sin(time * 3))); g.fill();
      g.font = `600 ${fs * 1.2}px ${font}`; g.fillStyle = S(0.95); g.fillText(done ? t.done : t.working, sx + fs * 1.05, headY);
      g.font = `${fs * 0.85}px ${mono}`; g.fillStyle = S(0.45); g.textAlign = "right"; g.fillText(t.onDevice, sx + sw, headY); g.textAlign = "left";

      // 已经飞进屏幕的照片：扫描线扫过后跳出评分，每组最好的留下
      const scanY = lerp(gridY, rowY(2) + ch + sh0 * 0.02, scan);
      for (const o of orbit) {
        const q = place(o);
        if (q.e < 0.999) continue;
        const scanned = scan > 0 && scanY > q.y + q.h * 0.6, best = BEST.has(o.i), r = q.w * 0.07;
        photo(o.im, q.x, q.y, q.w, q.h, best ? 1 : 1 - 0.5 * keep, r);
        if (best && keep > 0.01) {
          g.save(); g.strokeStyle = SA(keep); g.lineWidth = 1.8; g.shadowColor = SA(1); g.shadowBlur = 12;
          rr(q.x, q.y, q.w, q.h, r); g.stroke(); g.restore();
          g.fillStyle = SA(keep); star(q.x + q.w - fs * 0.7, q.y + fs * 0.7, fs * 0.45);
        }
        if (scanned) {
          const bw = fs * 2.1, bh = fs * 1.15, bx = q.x + fs * 0.25, by = q.y + q.h - bh - fs * 0.25, hot = best && keep > 0.5;
          g.fillStyle = hot ? SA(0.95) : "rgba(10,10,12,.72)"; rr(bx, by, bw, bh, bh / 2); g.fill();
          g.font = `600 ${fs * 0.8}px ${mono}`; g.textAlign = "center";
          g.fillStyle = hot ? "#0b0b0c" : "#f2f0ea"; g.fillText(String(SCORES[o.i]), bx + bw / 2, by + bh / 2 + 0.5); g.textAlign = "left";
        }
      }
      if (group > 0.01) {
        g.font = `${fs * 0.72}px ${mono}`; g.fillStyle = S(0.42 * group);
        for (let r = 0; r < 3; r++) g.fillText(`${t.group} ${r + 1}`, sx, rowY(r) - labelH * 0.5);
      }
      if (scan > 0.001 && scan < 0.999) {
        const grad = g.createLinearGradient(0, scanY - sh0 * 0.06, 0, scanY);
        grad.addColorStop(0, SA(0)); grad.addColorStop(1, SA(0.3));
        g.fillStyle = grad; g.fillRect(sx0, scanY - sh0 * 0.06, sw0, sh0 * 0.06);
        g.save(); g.strokeStyle = SA(0.95); g.lineWidth = 1.5; g.shadowColor = SA(1); g.shadowBlur = 10;
        g.beginPath(); g.moveTo(sx0, scanY); g.lineTo(sx0 + sw0, scanY); g.stroke(); g.restore();
      }
      g.font = `${fs * 0.8}px ${mono}`; g.fillStyle = S(0.55);
      g.fillText(t.kept(keep > 0.5 ? 3 : 0), sx, footY);
      // 底部横条
      rr(cx - sw0 * 0.18, sy0 + sh0 - sh0 * 0.018, sw0 * 0.36, sh0 * 0.007, 2); g.fillStyle = S(0.5); g.fill();
      // 玻璃上的一道斜高光
      const gl = g.createLinearGradient(sx0, sy0, sx0 + sw0, sy0 + sh0 * 0.6);
      gl.addColorStop(0, "rgba(255,255,255,.07)"); gl.addColorStop(0.4, "rgba(255,255,255,0)");
      g.fillStyle = gl; g.fillRect(sx0, sy0, sw0, sh0);
      g.restore();
      // 机身描边
      g.strokeStyle = "rgba(255,255,255,.16)"; g.lineWidth = 1; rr(px + 0.5, py + 0.5, pw - 1, ph - 1, pr); g.stroke();

      // 手机前面：前半圈的光弧、光点、照片
      arc(false); drawMotes(true);
      for (const o of orbit) outside(o, true);

      // 两块浮动的玻璃标签
      const ch2 = Math.max(28, pw * 0.17), cf = ch2 * 0.36;
      g.font = `600 ${cf}px ${font}`;
      const upW = ch2 * 1.05 + g.measureText(t.uploaded).width + ch2 * 0.55;
      const ux = Math.min(px + pw - ch2 * 0.3, W - upW - 4), uy = py + ph * 0.55 + Math.sin(time * 1.1) * 4;
      chip(ux, uy, ch2, (x, y, h) => {
        g.strokeStyle = I(dark ? 0.8 : 0.7); noUpload(x + h * 0.55, y + h / 2, h * 0.36);
        g.font = `600 ${cf}px ${font}`; g.fillStyle = A(1); g.textBaseline = "middle"; g.fillText(t.uploaded, x + h * 1.05, y + h / 2 + 0.5); return 0;
      }, upW);
      g.font = `500 ${cf * 0.92}px ${font}`;
      const offW = ch2 * 1.05 + g.measureText(t.offline).width + ch2 * 0.55;
      const fx = Math.max(4, px - offW + ch2 * 0.3), fy = py + ph * 0.2 + Math.sin(time * 1.1 + 2) * 4;
      chip(fx, fy, ch2 * 0.92, (x, y, h) => {
        g.fillStyle = A(1); plane(x + h * 0.55, y + h / 2, h * 0.3);
        g.font = `500 ${cf * 0.92}px ${font}`; g.fillStyle = I(dark ? 0.85 : 0.8); g.textBaseline = "middle"; g.fillText(t.offline, x + h * 1.05, y + h / 2 + 0.5); return 0;
      }, offW);
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
    // 跟着装置自己经过窗口：轨道整圈露出来之后照片才开始飞进去，装置居中时处理完（桌面和手机的装置高度不同，按它自己的高度算）
    const prog = (el: HTMLElement) => {
      const r = el.getBoundingClientRect(), end = (innerHeight - r.height) / 2 + 10, start = Math.max(end + 120, innerHeight - r.height * 0.82);
      return clamp((start - r.top) / (start - end), 0, 1);
    };
    const stop = addScene(c, prog, (q) => { p = clamp(q, 0, 1); });
    const io = new IntersectionObserver(([e]) => { on = e.isIntersecting; if (on && !raf) { last = performance.now(); raf = requestAnimationFrame(loop); } });
    io.observe(c);
    return () => { stop(); io.disconnect(); cancelAnimationFrame(raf); removeEventListener("resize", onResize); mo.disconnect(); };
  }, [lang]);
  return <canvas ref={cv} className="v-vault" aria-hidden />;
}
