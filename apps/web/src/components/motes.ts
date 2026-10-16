/**
 * The hero's sea of light: a nebula. Thousands of motes, one for every photograph of a long shoot, gather in three slow arms
 * around a bright heart, with depth (the near ones larger and brighter, and they shift more as the pointer moves) and a turn so
 * slow it is felt rather than seen, the inside faster than the edge, so the arms keep slowly changing shape. As the reader
 * scrolls, most of them flow together into a handful of frames, each mote taking the colour of the pixel it lands on, and the
 * real photographs develop underneath. Then the choice: a few frames are kept, and the motes of the rest come loose again and
 * drift back out into the sea they came from. The story is driven by scroll progress (`ST`), so the reader sets its pace and can
 * run it backwards; only the sea's own shimmer, the pointer and the bursts run on the clock.
 *
 * WebGL points, positions stepped on the CPU: a few thousand points is nothing for either side. Everything here is in CSS pixels
 * of the canvas; the React side tells it where the frames are and which ones are kept.
 */

export type Rect = { x: number; y: number; w: number; h: number };
/** A frame the motes form: its rectangle, the grid of pixel colours (rgb 0..1, row-major) the motes take on arrival, its place
 * in the order the frames appear, and whether it is turned down later (its motes then come loose and leave). */
export type FrameSpec = { rect: Rect; cols: number; rows: number; colors: Float32Array | null; order: number; out: boolean };
export type Palette = {
  mote: [number, number, number]; gleam: [number, number, number]; additive: boolean;
  /** The nebula's gas: large, faint, soft discs along the arms. */
  haze: [number, number, number]; hazeAlpha: number;
  /** Lifts every mote's alpha; the light theme needs more of it to read at all. */
  lift: number;
};

/** Where in the scroll (0 to 1 over the pinned stretch) each part of the story happens. The DOM side keys to the same numbers. */
export const ST = {
  gather: 0.07, // the motes begin to flow into frames
  gatherEnd: 0.46,
  develop: 0.44, // the photographs come into focus and the motes that formed them fade
  developEnd: 0.54,
  decide: 0.57, // a few are kept; the motes of the rest come loose and drift away
  best: 0.75, // of the kept, one is best
  explain: 0.88, // the reasons
} as const;

const VS = `
attribute vec2 a_pos; attribute vec4 a_col; attribute float a_size;
uniform vec2 u_res; uniform float u_dpr; varying vec4 v_col;
void main() {
  vec2 c = a_pos / u_res * 2.0 - 1.0;
  gl_Position = vec4(c.x, -c.y, 0.0, 1.0);
  gl_PointSize = a_size * u_dpr;
  v_col = a_col;
}`;
const FS = `
precision mediump float; varying vec4 v_col;
void main() {
  float d = length(gl_PointCoord - 0.5);
  float a = smoothstep(0.5, 0.18, d) * v_col.a;
  gl_FragColor = vec4(v_col.rgb * a, a);
}`;

const expo = (t: number) => (t >= 1 ? 1 : 1 - Math.pow(2, -10 * t));
const clamp01 = (t: number) => (t < 0 ? 0 : t > 1 ? 1 : t);
const smooth = (a: number, b: number, t: number) => { const x = clamp01((t - a) / (b - a)); return x * x * (3 - 2 * x); };

// Per-mote data, struct-of-arrays. `kind` 0 = sea, 1 = frame, 2 = burst.
type Store = {
  n: number; kind: Uint8Array; frame: Uint8Array;
  x: Float32Array; y: Float32Array; // where it is
  fx: Float32Array; fy: Float32Array; // its place in the sea (a frame mote starts there and, if turned down, goes back there)
  tx: Float32Array; ty: Float32Array; // its cell in a frame
  vx: Float32Array; vy: Float32Array; // sea drift / burst velocity
  r: Float32Array; g: Float32Array; b: Float32Array; // the pixel it takes on
  t0: Float32Array; dur: Float32Array; // its own leg of the gather (in progress units), or birth time for a burst
  size: Float32Array; seed: Float32Array; amber: Uint8Array;
  rad: Float32Array; ang: Float32Array; // place in the nebula, polar around its heart (rad < 0: a background mote that only drifts)
  z: Float32Array; // depth, 0 far .. 1 near
};

function store(n: number): Store {
  const f = () => new Float32Array(n);
  return { n, kind: new Uint8Array(n), frame: new Uint8Array(n), x: f(), y: f(), fx: f(), fy: f(), tx: f(), ty: f(), vx: f(), vy: f(), r: f(), g: f(), b: f(), t0: f(), dur: f(), size: f(), seed: f(), amber: new Uint8Array(n), rad: f(), ang: f(), z: f() };
}

export class Motes {
  private gl: WebGLRenderingContext | null = null;
  private prog: WebGLProgram | null = null;
  private buf: WebGLBuffer | null = null;
  private loc = { pos: 0, col: 0, size: 0, res: null as WebGLUniformLocation | null, dpr: null as WebGLUniformLocation | null };
  private data = new Float32Array(0);
  private s: Store = store(0);
  private frames: FrameSpec[] = [];
  private fieldCount: number;
  private hazeCount: number;
  private burstCount = 64;
  private maxPoint = 64; // the largest point this GPU draws, in device pixels
  private burstNext = 0;
  private w = 1; private h = 1; private dpr = 1;
  private raf = 0;
  private running = false;
  private began = -1; // performance.now() when the sea first appeared
  private last = 0;
  private target = 0; private P = 0; // scroll progress asked for, and the smoothed value actually shown
  private cx = 0.66; private cy = 0.5; private R = 0.5; // the nebula's heart and reach, as fractions of the canvas
  private px = -1e9; private py = -1e9;
  private pvx = 0; private pvy = 0; private pAt = 0; // pointer velocity, for the wake it leaves in the sea
  private parX = 0; private parY = 0; // the sea's slow slide against the pointer
  private palette: Palette;
  private lost = false;
  /** Called once per frame with the smoothed progress, for the DOM side to follow. */
  onTick: ((p: number) => void) | null = null;

  constructor(private canvas: HTMLCanvasElement, palette: Palette, opts: { field: number; haze: number }) {
    this.palette = palette;
    this.fieldCount = opts.field;
    this.hazeCount = opts.haze;
    const gl = canvas.getContext("webgl", { alpha: true, antialias: false, premultipliedAlpha: true, depth: false, stencil: false, powerPreference: "low-power" });
    if (!gl) return;
    const sh = (type: number, src: string) => { const s = gl.createShader(type); if (!s) return null; gl.shaderSource(s, src); gl.compileShader(s); return s; };
    const p = gl.createProgram(), v = sh(gl.VERTEX_SHADER, VS), f = sh(gl.FRAGMENT_SHADER, FS);
    if (!p || !v || !f) return;
    gl.attachShader(p, v); gl.attachShader(p, f); gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) return;
    this.gl = gl; this.prog = p; this.buf = gl.createBuffer();
    const range = gl.getParameter(gl.ALIASED_POINT_SIZE_RANGE) as Float32Array | null;
    if (range && range[1] > 0) this.maxPoint = range[1];
    this.loc = { pos: gl.getAttribLocation(p, "a_pos"), col: gl.getAttribLocation(p, "a_col"), size: gl.getAttribLocation(p, "a_size"), res: gl.getUniformLocation(p, "u_res"), dpr: gl.getUniformLocation(p, "u_dpr") };
    canvas.addEventListener("webglcontextlost", (e) => { e.preventDefault(); this.lost = true; this.stop(); });
  }

  get ok() { return !!this.gl; }
  get progress() { return this.P; }

  setPalette(p: Palette) { this.palette = p; }
  /** Where the nebula's heart is (fractions of the canvas) and how far it reaches. Set before the frames; the sea is laid out around it. */
  setFocus(cx: number, cy: number, reach: number) { this.cx = cx; this.cy = cy; this.R = reach; }
  /** Where the reader has scrolled to, 0 to 1. The shown value follows it with a little lag, so scrubbing never judders. */
  setProgress(p: number) { this.target = clamp01(p); }

  resize(w: number, h: number, dpr: number) {
    const first = this.w === 1 && this.h === 1;
    const sx = w / this.w, sy = h / this.h;
    this.w = w; this.h = h; this.dpr = Math.min(2, dpr);
    this.canvas.width = Math.round(w * this.dpr); this.canvas.height = Math.round(h * this.dpr);
    this.gl?.viewport(0, 0, this.canvas.width, this.canvas.height);
    if (!first) {
      // places in the sea keep their relative position when the canvas changes size
      const s = this.s, k = Math.min(sx, sy);
      for (let i = 0; i < s.n; i++) if (s.kind[i] !== 2) { s.fx[i] *= sx; s.fy[i] *= sy; if (s.rad[i] >= 0) s.rad[i] *= k; if (s.kind[i] === 0) { s.x[i] *= sx; s.y[i] *= sy; } }
    }
  }

  /** Lays the motes out: the frames' cells first, then the sea. Called when the frames are placed or move. */
  setFrames(frames: FrameSpec[]) {
    this.frames = frames;
    const cells = frames.reduce((a, f) => a + f.cols * f.rows, 0);
    const n = this.hazeCount + cells + this.fieldCount + this.burstCount;
    if (this.s.n !== n) {
      this.s = store(n);
      this.data = new Float32Array(n * 7);
      this.seedHaze();
      this.seedSea(this.hazeCount + cells);
      this.seedFrames();
    } else {
      this.retarget();
    }
  }

  private rand(seed: number) { const x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453; return x - Math.floor(x); }

  private gauss() { let u = 0, v = 0; while (u === 0) u = Math.random(); while (v === 0) v = Math.random(); return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v); }

  /** A place in the sea: most motes in the nebula's three arms around its heart, a few scattered over the whole screen as background. */
  private seaPoint(i: number) {
    const s = this.s;
    s.z[i] = 0.25 + 0.75 * Math.pow(Math.random(), 1.6);
    if (Math.random() < 0.26) {
      s.rad[i] = -1;
      s.fx[i] = Math.random() * this.w; s.fy[i] = Math.random() * this.h;
      return;
    }
    const reach = this.R * Math.min(this.w, this.h) * 1.15;
    if (Math.random() < 0.22) {
      // the heart: dense, round, bright
      s.rad[i] = Math.abs(this.gauss()) * 0.16 * reach; s.ang[i] = Math.random() * Math.PI * 2;
      this.placeFromPolar(i);
      return;
    }
    // the arms: three, each a tight twist that loosens a little toward the edge
    const r = Math.min(1.5, 0.12 + Math.abs(this.gauss()) * 0.5 + Math.pow(Math.random(), 2.4) * 0.45);
    const arm = Math.floor(Math.random() * 3) * ((Math.PI * 2) / 3);
    const a = arm + r * 2.7 + this.gauss() * (0.09 + 0.14 * r);
    s.rad[i] = r * reach; s.ang[i] = a;
    this.placeFromPolar(i);
  }

  /** The nebula is an ellipse, wider than it is tall, around the heart. */
  private placeFromPolar(i: number) {
    const s = this.s;
    s.fx[i] = this.cx * this.w + Math.cos(s.ang[i]) * s.rad[i] * 1.25;
    s.fy[i] = this.cy * this.h + Math.sin(s.ang[i]) * s.rad[i] * 0.78;
  }

  /** The gas of the nebula: a few hundred large, very faint discs laid along the arms and the heart, turning with them. */
  private seedHaze() {
    const s = this.s;
    const cap = this.maxPoint / this.dpr - 1;
    for (let i = 0; i < this.hazeCount; i++) {
      s.kind[i] = 3;
      this.seaPoint(i);
      if (s.rad[i] < 0) { s.rad[i] = Math.random() * 0.6 * this.R * Math.min(this.w, this.h); s.ang[i] = Math.random() * Math.PI * 2; this.placeFromPolar(i); }
      s.x[i] = s.fx[i]; s.y[i] = s.fy[i];
      s.z[i] = 0.2 + Math.random() * 0.5;
      s.size[i] = Math.min(cap, 60 + Math.random() * 70);
      s.seed[i] = Math.random() * 1000;
      s.amber[i] = Math.random() < 0.7 ? 1 : 0;
    }
  }

  private seedSea(from: number) {
    const s = this.s;
    for (let i = from; i < s.n - this.burstCount; i++) {
      s.kind[i] = 0;
      this.seaPoint(i);
      s.x[i] = s.fx[i]; s.y[i] = s.fy[i];
      const a = Math.random() * Math.PI * 2, v = 2 + Math.random() * 5;
      s.vx[i] = Math.cos(a) * v; s.vy[i] = Math.sin(a) * v;
      s.size[i] = (1.2 + Math.random() * 2.2) * (0.45 + 1.15 * s.z[i]);
      s.seed[i] = Math.random() * 1000;
      // the heart of the nebula is warmer
      const heart = s.rad[i] >= 0 ? Math.max(0, 1 - s.rad[i] / (this.R * Math.min(this.w, this.h) * 0.5)) : 0;
      s.amber[i] = Math.random() < 0.12 + 0.3 * heart ? 1 : 0;
    }
    for (let i = s.n - this.burstCount; i < s.n; i++) { s.kind[i] = 2; s.t0[i] = -1; s.size[i] = 2.2; }
  }

  private seedFrames() {
    const s = this.s;
    let i = this.hazeCount;
    this.frames.forEach((f, fi) => {
      for (let r = 0; r < f.rows; r++) for (let c = 0; c < f.cols; c++, i++) {
        s.kind[i] = 1; s.frame[i] = fi;
        this.seaPoint(i);
        s.x[i] = s.fx[i]; s.y[i] = s.fy[i];
        s.size[i] = 2.2 + Math.random() * 1.0;
        s.seed[i] = Math.random() * 1000;
        const heart = s.rad[i] >= 0 ? Math.max(0, 1 - s.rad[i] / (this.R * Math.min(this.w, this.h) * 0.5)) : 0;
        s.amber[i] = Math.random() < 0.1 + 0.28 * heart ? 1 : 0;
        // the frames form one after another over the gather stretch of the scroll, each condensing from its middle outward
        const dx = (c + 0.5) / f.cols - 0.5, dy = (r + 0.5) / f.rows - 0.5;
        const d = Math.sqrt(dx * dx + dy * dy) / 0.71;
        const span = ST.gatherEnd - ST.gather;
        s.t0[i] = ST.gather + span * (f.order * 0.075 + d * 0.22 + Math.random() * 0.08);
        s.dur[i] = span * (0.3 + Math.random() * 0.12);
      }
    });
    this.retarget();
  }

  /** Updates only where the frame motes are going and what colour they take, for a relayout or once pixels are known. */
  private retarget() {
    const s = this.s;
    let i = this.hazeCount;
    for (const f of this.frames) {
      for (let r = 0; r < f.rows; r++) for (let c = 0; c < f.cols; c++, i++) {
        s.tx[i] = f.rect.x + ((c + 0.5) / f.cols) * f.rect.w;
        s.ty[i] = f.rect.y + ((r + 0.5) / f.rows) * f.rect.h;
        const k = (r * f.cols + c) * 3;
        if (f.colors) { s.r[i] = f.colors[k]; s.g[i] = f.colors[k + 1]; s.b[i] = f.colors[k + 2]; }
        else { s.r[i] = s.g[i] = s.b[i] = -1; }
      }
    }
  }

  pointer(x: number, y: number) {
    const now = performance.now();
    if (this.px > -1e8 && this.pAt > 0) {
      const dt = Math.max(8, now - this.pAt) / 1000;
      // smoothed, so one jittery event does not whip the sea
      this.pvx = this.pvx * 0.6 + ((x - this.px) / dt) * 0.4;
      this.pvy = this.pvy * 0.6 + ((y - this.py) / dt) * 0.4;
    }
    this.px = x; this.py = y; this.pAt = now;
  }
  pointerOut() { this.px = this.py = -1e9; this.pvx = this.pvy = 0; }

  /** A handful of amber motes leave a point and fade: the app's own small celebration when a photo is kept. */
  burst(x: number, y: number, count = 26) {
    const s = this.s, now = this.clock();
    const base = s.n - this.burstCount;
    if (base < 0) return;
    for (let k = 0; k < count; k++) {
      const i = base + (this.burstNext++ % this.burstCount);
      const a = Math.random() * Math.PI * 2, v = 40 + Math.random() * 110;
      s.x[i] = x; s.y[i] = y; s.vx[i] = Math.cos(a) * v; s.vy[i] = Math.sin(a) * v - 20;
      s.t0[i] = now; s.dur[i] = 0.8 + Math.random() * 0.5; s.size[i] = 1.8 + Math.random() * 1.8;
    }
  }

  /** Seconds since the sea appeared. */
  clock() { return this.began < 0 ? 0 : (performance.now() - this.began) / 1000; }

  start() {
    if (!this.gl || this.lost) return;
    if (this.began < 0) this.began = performance.now();
    this.resume();
  }
  pause() { if (!this.running) return; this.running = false; cancelAnimationFrame(this.raf); }
  resume() {
    if (this.running || !this.gl || this.lost) return;
    this.running = true; this.last = performance.now();
    const loop = () => { if (!this.running) return; this.step(); this.raf = requestAnimationFrame(loop); };
    this.raf = requestAnimationFrame(loop);
  }
  stop() { this.running = false; cancelAnimationFrame(this.raf); }
  dispose() { this.stop(); this.gl?.getExtension("WEBGL_lose_context")?.loseContext(); this.gl = null; }

  private step() {
    const gl = this.gl!, s = this.s, p = this.palette;
    const now = performance.now();
    const dt = Math.min(0.05, (now - this.last) / 1000);
    this.last = now;
    const t = this.clock();
    // the shown progress eases toward the scrolled one
    this.P += (this.target - this.P) * Math.min(1, 7 * dt);
    if (Math.abs(this.target - this.P) < 0.0005) this.P = this.target;
    const P = this.P;
    this.onTick?.(P);
    const seaIn = smooth(0.05, 0.95, t);
    // once the frames have formed the sea steps back a little, so the photographs are what the eye lands on
    const seaDim = 1 - 0.5 * smooth(ST.gather + 0.12, ST.developEnd, P);
    const [mr, mg, mb] = p.mote, [ar, ag, ab] = p.gleam;
    const d = this.data;
    const W = this.w, H = this.h;
    const px = this.px, py = this.py, R = 190, R2 = R * R;
    // the whole sea slides a little against the pointer, the near motes more than the far: depth
    const hasPointer = px > -1e8;
    const pnx = hasPointer ? (px / W - 0.5) * 2 : 0, pny = hasPointer ? (py / H - 0.5) * 2 : 0;
    this.parX += ((hasPointer ? -pnx * 26 : 0) - this.parX) * Math.min(1, 3 * dt);
    this.parY += ((hasPointer ? -pny * 18 : 0) - this.parY) * Math.min(1, 3 * dt);
    const reach = this.R * Math.min(W, H) * 1.15;
    let j = 0;
    for (let i = 0; i < s.n; i++) {
      const kind = s.kind[i];
      let x = s.x[i], y = s.y[i], a = 0, r = mr, g = mg, b = mb, size = s.size[i];
      const amber = s.amber[i] === 1;
      if (amber) { r = ar; g = ag; b = ab; }
      const z = s.z[i];
      if (kind === 0) {
        // the sea. Each mote has a place: in the nebula it turns slowly about the heart (faster near it), in the background it
        // drifts and wraps at the edges. The mote itself is pulled back to its place, so whatever the pointer does is undone later.
        let hx: number, hy: number;
        if (s.rad[i] >= 0) {
          s.ang[i] += (0.028 + 0.05 * (1 - Math.min(1, s.rad[i] / reach))) * dt;
          this.placeFromPolar(i);
          hx = s.fx[i] + Math.sin(t * 0.7 + s.seed[i]) * 0.6; hy = s.fy[i] + Math.cos(t * 0.5 + s.seed[i]) * 0.6;
        } else {
          hx = s.fx[i] + s.vx[i] * dt; hy = s.fy[i] + s.vy[i] * dt;
          if (hx < -4) { hx += W + 8; x += W + 8; } else if (hx > W + 4) { hx -= W + 8; x -= W + 8; }
          if (hy < -4) { hy += H + 8; y += H + 8; } else if (hy > H + 4) { hy -= H + 8; y -= H + 8; }
          s.fx[i] = hx; s.fy[i] = hy;
        }
        hx += this.parX * (z - 0.4); hy += this.parY * (z - 0.4);
        let ox = x - hx, oy = y - hy;
        const dx = x - px, dy = y - py, dd = dx * dx + dy * dy;
        let lit = 0;
        if (dd < R2 && dd > 0.01) {
          // near the pointer: pushed aside, carried a little along the way the pointer is going, and lit by it
          const dist = Math.sqrt(dd), near = 1 - dist / R;
          lit = near * near;
          const f = lit * 420 * dt, inv = 1 / dist;
          ox += dx * inv * f + this.pvx * dt * 0.28 * near;
          oy += dy * inv * f + this.pvy * dt * 0.28 * near;
        }
        // the spring home
        const k = 1 - Math.min(1, 2.6 * dt);
        ox *= k; oy *= k;
        x = hx + ox; y = hy + oy;
        s.x[i] = x; s.y[i] = y;
        // a slow shimmer, each mote on its own time, so the sea is never quite still
        const twinkle = 0.82 + 0.18 * Math.sin(t * (0.5 + this.rand(s.seed[i] + 3) * 1.1) + s.seed[i]);
        a = seaIn * seaDim * (amber ? 0.95 : 0.8) * (0.3 + 0.7 * z) * (0.6 + 0.4 * this.rand(s.seed[i])) * twinkle * (1 + 1.4 * lit);
        size = s.size[i] * (1 + 1.1 * lit);
        if (lit > 0.001 && !amber) { r += (ar - r) * lit * 0.6; g += (ag - g) * lit * 0.6; b += (ab - b) * lit * 0.6; }
      } else if (kind === 1) {
        const fr = this.frames[s.frame[i]];
        const loose = fr?.out ? ST.decide + fr.order * 0.01 : Infinity;
        if (s.rad[i] >= 0) { s.ang[i] += (0.028 + 0.05 * (1 - Math.min(1, s.rad[i] / reach))) * dt; this.placeFromPolar(i); }
        if (P < loose) {
          // on its way to its cell, on an arc rather than a rail; it takes on its pixel as it settles, then fades as the photo develops
          const k = clamp01((P - s.t0[i]) / s.dur[i]);
          const e = expo(k);
          const dx = s.tx[i] - s.fx[i], dy = s.ty[i] - s.fy[i];
          const len = Math.sqrt(dx * dx + dy * dy) || 1;
          const bow = Math.sin(e * Math.PI) * Math.min(80, len * 0.2) * (s.seed[i] % 2 < 1 ? 1 : -1);
          const wob = (1 - e) * 5;
          x = s.fx[i] + dx * e + (-dy / len) * bow + Math.sin(t * 1.3 + s.seed[i]) * wob + this.parX * (z - 0.4) * (1 - e);
          y = s.fy[i] + dy * e + (dx / len) * bow + Math.cos(t * 1.1 + s.seed[i]) * wob + this.parY * (z - 0.4) * (1 - e);
          const settled = smooth(0.55, 1, k);
          if (s.r[i] >= 0) { r += (s.r[i] - r) * settled; g += (s.g[i] - g) * settled; b += (s.b[i] - b) * settled; }
          const gone = smooth(ST.develop, ST.developEnd, P);
          // before it sets off it is simply part of the sea
          const idle = 1 - smooth(0, 0.08, k);
          const twinkle = 0.82 + 0.18 * Math.sin(t * (0.5 + this.rand(s.seed[i] + 3) * 1.1) + s.seed[i]);
          a = seaIn * (idle * seaDim * 0.75 * (0.3 + 0.7 * z) * twinkle + (1 - idle) * (0.6 + 0.4 * settled)) * (1 - gone);
          size = s.size[i] * (idle * (0.55 + 0.9 * z) + (1 - idle) * (1 + 0.3 * settled));
        } else {
          // turned down: the motes come loose from the photograph and drift back out to where they were in the sea
          const k = clamp01((P - loose) / 0.15);
          const e = expo(k);
          const dx = s.fx[i] - s.tx[i], dy = s.fy[i] - s.ty[i];
          const len = Math.sqrt(dx * dx + dy * dy) || 1;
          const bow = Math.sin(e * Math.PI) * Math.min(60, len * 0.15) * (s.seed[i] % 2 < 1 ? 1 : -1);
          x = s.tx[i] + dx * e + (-dy / len) * bow;
          y = s.ty[i] + dy * e + (dx / len) * bow;
          if (s.r[i] >= 0) { const keepPix = 1 - e; r += (s.r[i] - r) * keepPix; g += (s.g[i] - g) * keepPix; b += (s.b[i] - b) * keepPix; }
          // once home it is a sea mote again, dim and shimmering
          const twinkle = 0.82 + 0.18 * Math.sin(t * (0.5 + this.rand(s.seed[i] + 3) * 1.1) + s.seed[i]);
          a = (k < 0.1 ? k / 0.1 : 1) * (0.85 - (0.85 - seaDim * 0.6 * twinkle) * smooth(0.3, 1, k));
          size = s.size[i] * (1.2 - 0.5 * e);
        }
        s.x[i] = x; s.y[i] = y;
      } else if (kind === 3) {
        // the gas: turns with the arms, breathes very slowly, steps well back once the photographs are there
        if (s.rad[i] >= 0) { s.ang[i] += (0.028 + 0.05 * (1 - Math.min(1, s.rad[i] / reach))) * dt; this.placeFromPolar(i); }
        x = s.fx[i] + this.parX * (z - 0.4) * 0.6; y = s.fy[i] + this.parY * (z - 0.4) * 0.6;
        s.x[i] = x; s.y[i] = y;
        const breathe = 0.75 + 0.25 * Math.sin(t * (0.15 + this.rand(s.seed[i] + 7) * 0.2) + s.seed[i]);
        const [hr, hg, hb] = p.haze;
        r = amber ? hr : mr; g = amber ? hg : mg; b = amber ? hb : mb;
        a = seaIn * (1 - 0.7 * smooth(ST.gather + 0.12, ST.developEnd, P)) * p.hazeAlpha * breathe;
      } else {
        // burst motes live for a moment after `burst()`
        if (s.t0[i] < 0) { a = 0; }
        else {
          const k = clamp01((t - s.t0[i]) / s.dur[i]);
          if (k >= 1) { a = 0; s.t0[i] = -1; }
          else {
            x += s.vx[i] * dt; y += s.vy[i] * dt; s.vx[i] *= 1 - 2.2 * dt; s.vy[i] *= 1 - 2.2 * dt;
            s.x[i] = x; s.y[i] = y;
            r = ar; g = ag; b = ab; a = (1 - k) * (1 - k); size = s.size[i] * (1 - 0.4 * k);
          }
        }
      }
      d[j++] = x; d[j++] = y; d[j++] = r; d[j++] = g; d[j++] = b; d[j++] = Math.min(1, a * (kind === 3 ? 1 : p.lift)); d[j++] = size;
    }
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.enable(gl.BLEND);
    if (p.additive) gl.blendFunc(gl.ONE, gl.ONE); else gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.useProgram(this.prog);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buf);
    gl.bufferData(gl.ARRAY_BUFFER, d, gl.DYNAMIC_DRAW);
    const stride = 7 * 4;
    gl.enableVertexAttribArray(this.loc.pos); gl.vertexAttribPointer(this.loc.pos, 2, gl.FLOAT, false, stride, 0);
    gl.enableVertexAttribArray(this.loc.col); gl.vertexAttribPointer(this.loc.col, 4, gl.FLOAT, false, stride, 8);
    gl.enableVertexAttribArray(this.loc.size); gl.vertexAttribPointer(this.loc.size, 1, gl.FLOAT, false, stride, 24);
    gl.uniform2f(this.loc.res, W, H);
    gl.uniform1f(this.loc.dpr, this.dpr);
    gl.drawArrays(gl.POINTS, 0, s.n);
  }
}

/** The rgb (0..1) of an image sampled on a cols × rows grid, for the motes to take on. */
export function sampleImage(img: HTMLImageElement, cols: number, rows: number): Float32Array | null {
  const c = document.createElement("canvas");
  c.width = cols; c.height = rows;
  const ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  // cover-fit, like the frame shows it
  const s = Math.max(cols / img.naturalWidth, rows / img.naturalHeight);
  const w = img.naturalWidth * s, h = img.naturalHeight * s;
  ctx.drawImage(img, (cols - w) / 2, (rows - h) / 2, w, h);
  const px = ctx.getImageData(0, 0, cols, rows).data;
  const out = new Float32Array(cols * rows * 3);
  for (let i = 0, k = 0; i < px.length; i += 4, k += 3) {
    // lifted a touch: a mote is a point of light, and the darkest pixels would simply vanish
    out[k] = 0.1 + (px[i] / 255) * 0.95; out[k + 1] = 0.1 + (px[i + 1] / 255) * 0.95; out[k + 2] = 0.1 + (px[i + 2] / 255) * 0.95;
  }
  return out;
}

/** The mote colours for the current theme, read from the page's own tokens. */
export function paletteFromTheme(): Palette {
  const css = getComputedStyle(document.documentElement);
  const light = css.getPropertyValue("color-scheme").trim() === "light";
  const hex = (v: string, fallback: [number, number, number]): [number, number, number] => {
    const m = v.trim().match(/^#([0-9a-f]{6})$/i);
    if (!m) return fallback;
    const n = parseInt(m[1], 16);
    return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
  };
  const gleam = hex(css.getPropertyValue("--gleam"), light ? [0.71, 0.46, 0.11] : [0.91, 0.71, 0.36]);
  return light
    ? { mote: [0.16, 0.17, 0.22], gleam, additive: false, haze: [0.78, 0.6, 0.36], hazeAlpha: 0.075, lift: 1.35 }
    : { mote: [0.93, 0.93, 0.96], gleam, additive: true, haze: [0.95, 0.72, 0.4], hazeAlpha: 0.05, lift: 1 };
}
