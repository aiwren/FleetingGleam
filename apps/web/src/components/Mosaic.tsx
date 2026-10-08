import { useEffect, useRef } from "react";
import { useReducedMotion } from "motion/react";

/** Only photographs that hold up at any size; they are cut into a 4×3 atlas and the shader picks one per cell. */
const PHOTOS = [
  "/wall/01.webp", "/wall/02.webp", "/wall/03.webp", "/wall/06.webp", "/wall/11.webp", "/wall/13.webp",
  "/wall/15.webp", "/photos/village.webp", "/photos/st0.webp", "/photos/st1.webp", "/photos/boat.webp",
];
const COLS = 4, ROWS = 3, SLOT = 256, TRAIL = 32;

const VERT = "attribute vec2 p; void main(){ gl_Position = vec4(p, 0.0, 1.0); }";
const FRAG = `
precision highp float;
uniform vec2 uRes; uniform float uCell; uniform sampler2D uAtlas; uniform vec4 uTrail[${TRAIL}];
float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
void main(){
  vec2 px = vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y);
  vec2 cell = floor(px / uCell);
  vec2 local = fract(px / uCell) - 0.5;
  vec2 centre = (cell + 0.5) * uCell;
  float b = 0.0;
  for (int i = 0; i < ${TRAIL}; i++) {
    vec4 t = uTrail[i];
    float d = distance(centre, t.xy) / (uCell * 4.2);
    b = max(b, t.z * exp(-d * d * 2.4));
  }
  float size = mix(0.3, 0.95, b);
  vec2 q = abs(local) - vec2(size * 0.5) + 0.07;
  float sd = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - 0.07;
  float aa = 1.4 / uCell;
  float mask = 1.0 - smoothstep(-aa, aa, sd);
  float id = floor(hash(cell) * ${PHOTOS.length}.0);
  vec2 slot = vec2(mod(id, ${COLS}.0), floor(id / ${COLS}.0));
  vec2 uv = clamp(local / size + 0.5, 0.004, 0.996);
  vec3 col = texture2D(uAtlas, (slot + uv) / vec2(${COLS}.0, ${ROWS}.0)).rgb;
  float a = mix(0.46, 1.0, b) * mask;
  col *= mix(0.78, 1.0, b);
  gl_FragColor = vec4(col * a, a);
}`;

/**
 * A wall of thumbnails, every one a real photograph, too small to read until the pointer passes: the ones it touches swell to
 * full size and brightness and then settle back into the crowd. One fragment shader draws the whole grid, so the cost does
 * not grow with the number of photographs. Nothing is greyed out: what is not chosen simply steps back.
 */
export default function Mosaic() {
  const reduce = useReducedMotion();
  const box = useRef<HTMLDivElement>(null);
  const cv = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const host = box.current!, canvas = cv.current!;
    const gl = canvas.getContext("webgl", { premultipliedAlpha: true, alpha: true, antialias: false });
    if (!gl) return;

    const compile = (type: number, src: string) => {
      const s = gl.createShader(type)!;
      gl.shaderSource(s, src); gl.compileShader(s);
      return s;
    };
    const prog = gl.createProgram()!;
    gl.attachShader(prog, compile(gl.VERTEX_SHADER, VERT));
    gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FRAG));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return;
    gl.useProgram(prog);

    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(prog, "p");
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    const u = (n: string) => gl.getUniformLocation(prog, n);
    const uRes = u("uRes"), uCell = u("uCell"), uTrail = u("uTrail");
    gl.uniform1i(u("uAtlas"), 0);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

    let live = true, raf = 0, visible = true;
    let dpr = 1, cell = 56, w = 0, h = 0;
    const pts: { x: number; y: number; s: number; t: number }[] = [];
    let lastX = -1e4, lastY = -1e4, lastInput = 0;
    const startedAt = performance.now();
    const data = new Float32Array(TRAIL * 4);

    const resize = () => {
      dpr = Math.min(2, window.devicePixelRatio || 1);
      const r = host.getBoundingClientRect();
      cell = (r.width < 640 ? 38 : 56) * dpr;
      w = Math.max(1, Math.round(r.width * dpr)); h = Math.max(1, Math.round(r.height * dpr));
      if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
      gl.viewport(0, 0, w, h);
    };

    const add = (x: number, y: number, s = 1) => {
      const dx = x - lastX, dy = y - lastY, dist = Math.hypot(dx, dy);
      if (dist < cell * 0.35) return;
      const steps = Math.min(6, Math.floor(dist / (cell * 0.6)));
      for (let k = 1; k <= steps; k++) pts.push({ x: lastX + (dx * k) / (steps + 1), y: lastY + (dy * k) / (steps + 1), s, t: performance.now() });
      pts.push({ x, y, s, t: performance.now() });
      lastX = x; lastY = y;
      while (pts.length > TRAIL) pts.shift();
    };

    const draw = (now: number, still = false) => {
      for (let i = 0; i < TRAIL; i++) {
        const p = pts[i];
        const k = p ? (still ? p.s : Math.pow(Math.max(0, 1 - (now - p.t) / 1500), 1.6) * p.s) : 0;
        data[i * 4] = p ? p.x : 0; data[i * 4 + 1] = p ? p.y : 0; data[i * 4 + 2] = k; data[i * 4 + 3] = 0;
      }
      gl.uniform2f(uRes, w, h); gl.uniform1f(uCell, cell); gl.uniform4fv(uTrail, data);
      gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    };

    const tick = (now: number) => {
      raf = 0;
      if (!live || !visible) return;
      resize();
      const age = now - startedAt;
      // On arrival one sweep crosses the wall, and whenever the pointer is idle a slow path keeps it from sitting dead.
      if (age > 500 && age < 2900) {
        const k = (age - 500) / 2400;
        add(w * (-0.05 + 1.1 * (1 - Math.pow(1 - k, 2.2))), h * (0.5 + 0.18 * Math.sin(k * 6)), 1);
      } else if (age >= 2900 && now - lastInput > 2600) {
        add(w * (0.5 + 0.38 * Math.sin(now * 0.00042)), h * (0.5 + 0.22 * Math.sin(now * 0.00071 + 1.3)), 0.85);
      }
      draw(now);
      raf = requestAnimationFrame(tick);
    };

    const onMove = (e: PointerEvent) => {
      const r = host.getBoundingClientRect();
      lastInput = performance.now();
      add((e.clientX - r.left) * dpr, (e.clientY - r.top) * dpr, 1);
    };

    const atlas = document.createElement("canvas");
    atlas.width = COLS * SLOT; atlas.height = ROWS * SLOT;
    const ctx = atlas.getContext("2d")!;
    let loaded = 0;
    const onAll = () => {
      const tex = gl.createTexture();
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, atlas);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      resize();
      if (reduce) {
        // The finished state: a few photographs already lit, no performance.
        for (const [fx, fy] of [[0.28, 0.42], [0.5, 0.55], [0.72, 0.4]]) { lastX = -1e4; add(w * fx, h * fy, 1); }
        draw(0, true);
        return;
      }
      raf = requestAnimationFrame(tick);
    };
    PHOTOS.forEach((src, i) => {
      const img = new Image();
      img.onload = () => {
        const sx = (img.width - Math.min(img.width, img.height)) / 2, sy = (img.height - Math.min(img.width, img.height)) / 2;
        const side = Math.min(img.width, img.height);
        ctx.drawImage(img, sx, sy, side, side, (i % COLS) * SLOT, Math.floor(i / COLS) * SLOT, SLOT, SLOT);
        if (++loaded === PHOTOS.length && live) onAll();
      };
      img.onerror = () => { if (++loaded === PHOTOS.length && live) onAll(); };
      img.src = src;
    });

    const io = new IntersectionObserver(([e]) => {
      visible = e.isIntersecting;
      if (visible && !raf && loaded === PHOTOS.length && !reduce) raf = requestAnimationFrame(tick);
    });
    io.observe(host);
    if (!reduce) host.addEventListener("pointermove", onMove);
    window.addEventListener("resize", resize);

    return () => {
      live = false;
      if (raf) cancelAnimationFrame(raf);
      io.disconnect();
      host.removeEventListener("pointermove", onMove);
      window.removeEventListener("resize", resize);
    };
  }, [reduce]);

  return (
    <div ref={box} aria-hidden className="relative left-1/2 w-screen -translate-x-1/2 touch-pan-y"
      style={{ height: "clamp(240px, 44vh, 460px)", maskImage: "linear-gradient(to bottom, transparent, #000 24%, #000 76%, transparent)", WebkitMaskImage: "linear-gradient(to bottom, transparent, #000 24%, #000 76%, transparent)" }}>
      <canvas ref={cv} className="absolute inset-0 h-full w-full" />
    </div>
  );
}
