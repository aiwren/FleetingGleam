import { useEffect, useRef, useState } from "react";

/**
 * The burst, seen the way a projectionist sees film: one deep stack of six frames of the same moment, advancing towards the reader
 * a frame at a time as they scroll. Only the frame at the gate is in focus, the rest fall away soft, which is the whole point:
 * these takes are nearly identical, and the work is choosing between them. Each frame is read by a line of warm light that leaves
 * its score behind, then either drifts out of shot or, if it is the one worth keeping, stops dead centre and takes the amber edge.
 *
 * It is plain WebGL2: a few textured quads and one shader do not justify shipping a 3D engine to a reader who came to look at
 * photographs. No WebGL, reduced motion, a hidden tab, a section scrolled past: each has its own answer rather than a dead canvas.
 * The frames are one photograph re-taken six ways (drifted, defocused, underexposed) and the scores are illustrative.
 */

const TAKES = 6;
const KEEPER = 5;
/** Illustrative, not measured: the soft frame and the dark frame score low, the keeper scores highest. */
const SCORES = [61, 58, 34, 42, 66, 79];
const TEX_W = 512, TEX_H = 342;

/** The gate is where a frame is read: sharp, centred, and as large as the stage allows. */
const FIT = {
  wide: { w: 2.62, gap: 1.6, gate: 1.5, camZ: 7, camX: -1.12, lift: 0, yaw: 0.24, pitch: -0.07 },
  phone: { w: 1.55, gap: 1.05, gate: 1.4, camZ: 6.8, camX: 0.28, lift: 0.78, yaw: 0.2, pitch: -0.05 },
};

const VERT = `#version 300 es
  in vec2 aPos;
  out vec2 vUv;
  uniform mat4 uProj, uView, uModel;
  void main() {
    vUv = vec2(aPos.x + 0.5, 0.5 - aPos.y);
    gl_Position = uProj * uView * uModel * vec4(aPos, 0.0, 1.0);
  }
`;

const FRAG = `#version 300 es
  precision highp float;
  in vec2 vUv;
  out vec4 outColor;
  uniform sampler2D uMap;
  uniform vec2 uSize;        // the quad in world units, so the corner radius stays round at any scale
  uniform vec3 uBg, uAmber;
  uniform float uRadius;
  uniform float uDepth;      // a rejected frame falls back into the canvas: it never turns grey
  uniform float uKept;       // the amber edge that means "kept"
  uniform float uGleam;      // 0 to 1, the band crossing once as the frame is chosen
  uniform float uAlpha;
  uniform float uScan, uScanX, uFrameX;   // the reading line, in world space, so it crosses the frames as one movement
  uniform float uBlur;      // real defocus, read from the mip chain: a frame the app rejects goes soft, as it would in a viewfinder
  uniform float uFlat;      // 1 for a shadow or a mark: no photograph treatment, just the texture

  float roundRect(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
  }

  void main() {
    vec2 p = (vUv - 0.5) * uSize;
    float d = roundRect(p, uSize * 0.5, uRadius);
    float aa = fwidth(d) * 1.1;
    vec4 tex = textureLod(uMap, vUv, uBlur);
    float a = (1.0 - smoothstep(-aa, aa, d)) * uAlpha * tex.a;
    if (a <= 0.002) discard;

    if (uFlat > 0.5) {
      outColor = vec4(tex.rgb * a, a);
      return;
    }

    // Aerial perspective: distance and rejection are the canvas washing over the frame, so it keeps every bit of its colour.
    vec3 col = mix(tex.rgb, uBg, uDepth);

    float rim = smoothstep(-0.015, -0.002, d);
    col = mix(col, uAmber, rim * 0.8 * uKept);

    // The reading line, and the gleam that follows the verdict.
    float wx = uFrameX + (vUv.x - 0.5) * uSize.x;
    col += uAmber * exp(-pow((wx - uScanX) / 0.1, 2.0)) * uScan * 0.55;
    float band = exp(-pow((vUv.x + vUv.y * 0.45 - uGleam * 1.9 + 0.35) / 0.115, 2.0));
    col += uAmber * band * uGleam * (1.0 - uGleam) * 2.1;

    outColor = vec4(col * a, a);   // premultiplied, to composite cleanly over the page
  }
`;

type M4 = Float32Array;
const m4 = (): M4 => new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
/** out = a * b, column-major, the order WebGL wants. */
function mul(a: M4, b: M4, out: M4): M4 {
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      out[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
    }
  }
  return out;
}

const clamp01 = (x: number) => (x < 0 ? 0 : x > 1 ? 1 : x);
const seg = (t: number, a: number, b: number) => clamp01((t - a) / (b - a));
const out4 = (x: number) => 1 - Math.pow(1 - x, 4);
const lerp = (a: number, b: number, k: number) => a + (b - a) * k;

/** The score chip and the keep badge: the app's own marks, drawn once into textures. */
function chipTexture(label: string, ink: string, bg: string): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = 160; c.height = 88;
  const g = c.getContext("2d")!;
  g.fillStyle = bg;
  g.beginPath();
  g.roundRect(4, 4, c.width - 8, c.height - 8, 30);
  g.fill();
  g.fillStyle = ink;
  g.font = "600 48px ui-monospace, SFMono-Regular, Menlo, monospace";
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.fillText(label, c.width / 2, c.height / 2 + 2);
  return c;
}

/** A soft ellipse of shade, so a frame in front reads as resting above the ones behind it. */
function shadowTexture(): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = c.height = 128;
  const g = c.getContext("2d")!;
  const grad = g.createRadialGradient(64, 64, 0, 64, 64, 62);
  grad.addColorStop(0, "rgba(0,0,0,0.5)");
  grad.addColorStop(0.55, "rgba(0,0,0,0.26)");
  grad.addColorStop(1, "rgba(0,0,0,0)");
  g.fillStyle = grad;
  g.fillRect(0, 0, 128, 128);
  return c;
}

function badgeTexture(amber: string, ink: string): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = c.height = 96;
  const g = c.getContext("2d")!;
  g.fillStyle = amber;
  g.beginPath();
  g.arc(48, 48, 44, 0, Math.PI * 2);
  g.fill();
  g.strokeStyle = ink;
  g.lineWidth = 9;
  g.lineCap = "round";
  g.lineJoin = "round";
  g.beginPath();
  g.moveTo(29, 49);
  g.lineTo(42, 62);
  g.lineTo(68, 34);
  g.stroke();
  return c;
}

export default function BurstScene({ compact, progress }: { compact: boolean; progress: () => number }) {
  const host = useRef<HTMLDivElement>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!host.current) return;
    let cancelled = false;
    let stop: (() => void) | undefined;

    // After the first paint: the headline is never waiting on a picture.
    const later = window.requestIdleCallback ? window.requestIdleCallback(run) : window.setTimeout(run, 200);

    function run() {
      const mount = host.current;
      if (cancelled || !mount) return;

      const canvas = document.createElement("canvas");
      canvas.setAttribute("aria-hidden", "true");
      canvas.style.cssText = "width:100%;height:100%;display:block;opacity:0;transition:opacity .9s cubic-bezier(.16,1,.3,1)";
      const ctx = canvas.getContext("webgl2", { alpha: true, antialias: true, premultipliedAlpha: true, powerPreference: "low-power" });
      if (!ctx) { setFailed(true); return; }
      const gl = ctx;
      mount.appendChild(canvas);

      const compile = (type: number, src: string) => {
        const s = gl.createShader(type)!;
        gl.shaderSource(s, src.trim());
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader");
        return s;
      };
      let prog: WebGLProgram;
      try {
        prog = gl.createProgram()!;
        gl.attachShader(prog, compile(gl.VERTEX_SHADER, VERT));
        gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FRAG));
        gl.linkProgram(prog);
        if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog) ?? "link");
      } catch {
        canvas.remove();
        setFailed(true);
        return;
      }
      gl.useProgram(prog);
      const u = (n: string) => gl.getUniformLocation(prog, n);
      const U = {
        proj: u("uProj"), view: u("uView"), model: u("uModel"), size: u("uSize"), bg: u("uBg"), amber: u("uAmber"),
        radius: u("uRadius"), depth: u("uDepth"), kept: u("uKept"), gleam: u("uGleam"), alpha: u("uAlpha"),
        blur: u("uBlur"), flat: u("uFlat"),
        scan: u("uScan"), scanX: u("uScanX"), frameX: u("uFrameX"),
      };

      const quad = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, quad);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, 0.5]), gl.STATIC_DRAW);
      const aPos = gl.getAttribLocation(prog, "aPos");
      gl.enableVertexAttribArray(aPos);
      gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

      // Every colour is read from the page's own tokens, so a theme change moves the scene with it.
      const bg = new Float32Array(3), amberRGB = new Float32Array(3);
      const css = { bg: "#0e0e10", amber: "#e8b45c", ink: "#ededef", panel: "#151517", gleamInk: "#241703" };
      const toRGB = (v: string, out: Float32Array) => {
        if (v.startsWith("#")) {
          const h = v.slice(1);
          const n = parseInt(h.length === 3 ? h.split("").map((c) => c + c).join("") : h, 16);
          out[0] = ((n >> 16) & 255) / 255; out[1] = ((n >> 8) & 255) / 255; out[2] = (n & 255) / 255;
        } else {
          const m = v.match(/[\d.]+/g);
          if (m && m.length >= 3) { out[0] = +m[0] / 255; out[1] = +m[1] / 255; out[2] = +m[2] / 255; }
        }
      };
      const readTheme = () => {
        const cs = getComputedStyle(document.body);
        css.bg = cs.backgroundColor;
        css.amber = cs.getPropertyValue("--gleam").trim() || css.amber;
        css.ink = cs.getPropertyValue("--ink").trim() || css.ink;
        css.panel = cs.getPropertyValue("--surface").trim() || css.panel;
        css.gleamInk = cs.getPropertyValue("--gleam-ink").trim() || css.gleamInk;
        toRGB(css.bg, bg);
        toRGB(css.amber, amberRGB);
      };
      readTheme();

      const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      const fit = compact ? FIT.phone : FIT.wide;
      const fw = fit.w, fh = fw * (TEX_H / TEX_W);

      const upload = (source: TexImageSource) => {
        const tex = gl.createTexture()!;
        gl.bindTexture(gl.TEXTURE_2D, tex);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        return tex;
      };

      const frames = Array.from({ length: TAKES }, (_, i) => {
        const tex = gl.createTexture()!;
        gl.bindTexture(gl.TEXTURE_2D, tex);
        // A warm pixel stands in until the photograph has loaded, so a frame never flashes black.
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([96, 84, 72, 255]));
        const img = new Image();
        img.decoding = "async";
        img.src = `/hero/b${i}.webp`;
        img.onload = () => {
          gl.bindTexture(gl.TEXTURE_2D, tex);
          gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
          gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
          gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
          gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
          gl.generateMipmap(gl.TEXTURE_2D);
          if (reduce) draw(0);
        };
        // The camera drifted between takes, so the frames do not sit perfectly on each other.
        return { i, tex, chip: null as WebGLTexture | null, kept: i === KEEPER, jx: (i % 2 ? 1 : -1) * 0.05, jy: -0.015 * i, jr: (((i * 37) % 7) - 3) * 0.006 };
      });

      const shade = upload(shadowTexture());
      let badge: WebGLTexture | null = null;
      const marks = () => {
        frames.forEach((f) => {
          if (f.chip) gl.deleteTexture(f.chip);
          f.chip = upload(chipTexture(String(SCORES[f.i]), f.kept ? css.gleamInk : css.ink, f.kept ? css.amber : css.panel));
        });
        if (badge) gl.deleteTexture(badge);
        badge = upload(badgeTexture(css.amber, css.gleamInk));
      };
      marks();

      const proj = m4(), view = m4(), tmp = m4(), rot = m4(), scale = m4(), trans = m4(), camT = m4(), model = m4();
      let dpr = 1, camZ = fit.camZ;
      const size = () => {
        const r = mount.getBoundingClientRect();
        if (r.width === 0) return;
        dpr = Math.min(window.devicePixelRatio || 1, 2);
        canvas.width = Math.round(r.width * dpr);
        canvas.height = Math.round(r.height * dpr);
        gl.viewport(0, 0, canvas.width, canvas.height);
        const aspect = r.width / r.height;
        // The stage has its own box, so the camera only steps back when that box turns narrow.
        camZ = fit.camZ + Math.max(0, (1.6 - aspect) * 2.8);
        const f = 1 / Math.tan((34 * Math.PI) / 360);
        proj.fill(0);
        proj[0] = f / aspect; proj[5] = f; proj[10] = 100.1 / -99.9; proj[11] = -1; proj[14] = 20 / -99.9;
        gl.uniformMatrix4fv(U.proj, false, proj);
      };
      size();
      const ro = new ResizeObserver(size);
      ro.observe(mount);

      const theme = new MutationObserver(() => { readTheme(); marks(); if (reduce) draw(0); });
      theme.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

      // Pointer parallax: the whole arc turns towards the reader with weight, and eases back when they leave.
      const want = { x: 0, y: 0 }, at = { x: 0, y: 0 };
      const onMove = (e: PointerEvent) => {
        const r = mount.getBoundingClientRect();
        want.x = ((e.clientX - r.left) / r.width - 0.5) * 0.3;
        want.y = ((e.clientY - r.top) / r.height - 0.5) * 0.2;
      };
      const onLeave = () => { want.x = 0; want.y = 0; };
      if (!reduce) {
        window.addEventListener("pointermove", onMove, { passive: true });
        mount.addEventListener("pointerleave", onLeave);
      }

      // The sequence is scrubbed: scrolling through the section is what reads the frames, so the reader sets the pace.
      let scrolled = 0, head = 0;
      const onScroll = () => { head = progress(); };
      onScroll();
      window.addEventListener("scroll", onScroll, { passive: true });

      /** One quad: the model matrix from position, rotation and size, then the uniforms this piece needs. */
      function piece(tex: WebGLTexture, x: number, y: number, z: number, rz: number, w: number, h: number, alpha: number,
        o: { radius?: number; depth?: number; kept?: number; gleam?: number; scan?: number; scanX?: number; blur?: number; flat?: number } = {}) {
        if (alpha <= 0.002) return;
        const ry = at.x * 0.45, rx = -at.y * 0.35;
        const c1 = Math.cos(ry), s1 = Math.sin(ry), c2 = Math.cos(rx), s2 = Math.sin(rx), c3 = Math.cos(rz), s3 = Math.sin(rz);
        rot.fill(0); rot[15] = 1;
        rot[0] = c1 * c3 + s1 * s2 * s3; rot[1] = c2 * s3; rot[2] = -s1 * c3 + c1 * s2 * s3;
        rot[4] = -c1 * s3 + s1 * s2 * c3; rot[5] = c2 * c3; rot[6] = s1 * s3 + c1 * s2 * c3;
        rot[8] = s1 * c2; rot[9] = -s2; rot[10] = c1 * c2;
        scale.fill(0); scale[0] = w; scale[5] = h; scale[10] = 1; scale[15] = 1;
        trans.fill(0); trans[0] = trans[5] = trans[10] = trans[15] = 1; trans[12] = x; trans[13] = y; trans[14] = z;
        mul(rot, scale, tmp);
        mul(trans, tmp, model);
        gl.uniformMatrix4fv(U.model, false, model);
        gl.uniform2f(U.size, w, h);
        gl.uniform1f(U.radius, o.radius ?? 0.1);
        gl.uniform1f(U.depth, o.depth ?? 0);
        gl.uniform1f(U.kept, o.kept ?? 0);
        gl.uniform1f(U.gleam, o.gleam ?? 0);
        gl.uniform1f(U.alpha, alpha);
        gl.uniform1f(U.scan, o.scan ?? 0);
        gl.uniform1f(U.scanX, o.scanX ?? 0);
        gl.uniform1f(U.frameX, x);
        gl.uniform1f(U.blur, o.blur ?? 0);
        gl.uniform1f(U.flat, o.flat ?? 0);
        gl.bindTexture(gl.TEXTURE_2D, tex);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      }

      const eased = { at: 0 };
      function draw(t: number) {
        at.x += (want.x - at.x) * 0.045;
        at.y += (want.y - at.y) * 0.045;
        // Scrubbed, then eased: a flick of the wheel still lands softly instead of snapping the stack forward.
        eased.at += ((reduce ? 1 : head) * TAKES - eased.at) * 0.085;
        const advance = eased.at;

        const yaw = fit.yaw + at.x * 0.1, pitch = fit.pitch - at.y * 0.08;
        const cy = Math.cos(yaw), sy = Math.sin(yaw), cx = Math.cos(pitch), sx = Math.sin(pitch);
        rot.fill(0); rot[15] = 1;
        rot[0] = cy; rot[2] = -sy; rot[5] = cx; rot[6] = sx; rot[8] = sy * cx; rot[9] = -sx; rot[10] = cy * cx;
        camT.fill(0); camT[0] = camT[5] = camT[10] = camT[15] = 1; camT[12] = -fit.camX; camT[14] = -camZ;
        mul(camT, rot, view);
        gl.uniformMatrix4fv(U.view, false, view);
        gl.uniform3fv(U.bg, bg);
        gl.uniform3fv(U.amber, amberRGB);
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);

        // Deepest first: with blending and no depth buffer, the draw order is the depth test.
        for (let k = frames.length - 1; k >= 0; k--) {
          const f = frames[k];
          // How far this frame has come towards the gate, and how far it has got since being read there.
          const approach = clamp01(advance - f.i + 1);
          const past = clamp01(advance - f.i);
          const sent = f.kept ? 0 : out4(clamp01((past - 0.22) / 0.78));

          // Position along the run. A frame comes forward to the gate and stops there: it is the stack that moves, not the
          // camera, and nothing is allowed past the gate, so the frame being read is always at exactly the same size.
          const zNow = fit.gate - fit.gap * (f.i - Math.min(advance, f.i)) - sent * 2.2;
          const dir = f.i % 2 ? 1 : -1;
          const x = f.jx + sent * 3.4 * dir + at.x * 0.12;
          const y = f.jy + fit.lift + sent * 0.55 + Math.sin(t * 0.3 + f.i) * 0.012;
          const grow = f.kept ? lerp(1, 1.06, out4(past)) : 1;

          // Real defocus: sharp only at the gate, soft everywhere else, read straight from the mip chain.
          const off = Math.abs(zNow - fit.gate);
          const blur = Math.min(4, off * 0.5 + sent * 2.6);
          const alpha = (1 - clamp01((sent - 0.55) / 0.45)) * clamp01(approach * 2.4) * clamp01((5.2 + zNow) / 2);
          if (alpha <= 0.004) continue;

          const w = fw * grow, h = fh * grow;
          const rz = f.jr + sent * 0.22 * dir + at.x * 0.05;
          // A frame at the gate throws its shade onto the stack behind it, which is what gives the pile weight.
          piece(shade, x + 0.06, y - 0.1, zNow - 0.04, rz, w * 1.2, h * 1.45, alpha * 0.8 * clamp01(1.4 - off * 0.5), { radius: 0, flat: 1 });

          // The reading line crosses the frame while it sits at the gate.
          const readK = clamp01((past - 0.02) / 0.3);
          const scanOn = past > 0.02 && past < 0.34 ? Math.sin(Math.PI * readK) : 0;
          const g = f.kept ? clamp01((past - 0.32) / 0.5) : 0;

          piece(f.tex, x, y, zNow, rz, w, h, alpha, {
            blur,
            depth: f.kept ? 0 : sent * 0.35,
            kept: f.kept ? out4(clamp01((past - 0.3) / 0.35)) : 0,
            gleam: g > 0 && g < 1 ? g : 0,
            scan: scanOn, scanX: lerp(x - w * 0.5, x + w * 0.5, readK),
          });

          // The score it was given, under the frame, in the app's own mono figures.
          if (f.chip) {
            const chipA = alpha * clamp01((past - 0.12) / 0.18) * (f.kept ? 1 : 1);
            piece(f.chip, x, y - h * 0.5 - 0.2, zNow, 0, 0.56, 0.308, chipA, { radius: 0, flat: 1 });
          }

          // The keep badge, landing with a little overshoot on the frame that was chosen.
          if (f.kept && badge) {
            const b = clamp01((past - 0.42) / 0.3);
            const pop = b >= 1 ? 1 : Math.sin(b * Math.PI * 0.75) * 1.12;
            piece(badge, x + w * 0.5 - 0.24, y + h * 0.5 - 0.24, zNow + 0.01, 0, 0.38 * pop, 0.38 * pop, alpha * clamp01(b * 3), { radius: 0, flat: 1 });
          }
        }
      }

      let raf = 0, t0 = 0, onView = true;
      const tick = (ms: number) => {
        raf = 0;
        if (!t0) t0 = ms;
        draw((ms - t0) / 1000);
        if (onView && !document.hidden) raf = requestAnimationFrame(tick);
      };
      const wake = () => { if (onView && !document.hidden && !reduce && !raf) raf = requestAnimationFrame(tick); };
      const io = new IntersectionObserver(([e]) => { onView = e.isIntersecting; wake(); }, { threshold: 0 });
      io.observe(mount);
      const onVis = () => { if (document.hidden && raf) { cancelAnimationFrame(raf); raf = 0; } else wake(); };
      document.addEventListener("visibilitychange", onVis);

      // Reduced motion: the verdict is already in. The reader sees the result, not the performance.
      if (reduce) draw(0); else raf = requestAnimationFrame(tick);
      canvas.style.opacity = "1";

      stop = () => {
        if (raf) cancelAnimationFrame(raf);
        ro.disconnect();
        io.disconnect();
        theme.disconnect();
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("scroll", onScroll);
        document.removeEventListener("visibilitychange", onVis);
        mount.removeEventListener("pointerleave", onLeave);
        frames.forEach((f) => { gl.deleteTexture(f.tex); if (f.chip) gl.deleteTexture(f.chip); });
        if (badge) gl.deleteTexture(badge);
        gl.deleteTexture(shade);
        gl.deleteBuffer(quad);
        gl.deleteProgram(prog);
        gl.getExtension("WEBGL_lose_context")?.loseContext();
        canvas.remove();
      };
    }

    return () => {
      cancelled = true;
      if (window.cancelIdleCallback) window.cancelIdleCallback(later as number);
      else clearTimeout(later as number);
      stop?.();
    };
  }, [compact, progress]);

  // No WebGL: the same six frames of one moment, fanned flat, with the keeper marked. The idea survives without the scene.
  if (failed) {
    return (
      <div aria-hidden className="absolute inset-0 overflow-hidden">
        {Array.from({ length: TAKES }, (_, i) => {
          const kept = i === KEEPER;
          return (
            <img key={i} src={`/hero/b${i}.webp`} alt="" width={TEX_W} height={TEX_H} loading="lazy" draggable={false}
              className="absolute w-[30%] rounded-2xl md:w-[24%]"
              style={{
                left: `${kept ? 72 : 58 + i * 5}%`, top: `${kept ? 50 : 42 + Math.abs(i - 2.5) * 4}%`, zIndex: kept ? 10 : i,
                transform: `translate(-50%,-50%) rotate(${(i - 2.5) * 2}deg) scale(${kept ? 1.14 : 1})`,
                opacity: kept ? 1 : 0.42,
                boxShadow: kept ? "0 0 0 1.5px var(--gleam)" : "none",
              }} />
          );
        })}
      </div>
    );
  }

  return <div ref={host} className="absolute inset-0" aria-hidden />;
}
