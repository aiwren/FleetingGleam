/**
 * The hero's sea of light: a nebula. Thousands of motes, one for every photograph of a long shoot, gather in three slow arms
 * around a bright heart, with depth (the near ones larger and brighter, and they shift more as the pointer moves) and a turn so
 * slow it is felt rather than seen, the inside faster than the edge, so the arms keep slowly changing shape.
 *
 * The frames live *on* the arms. Each one is a point on an arm (which arm, how far out) and rides the turn with it; a thread of
 * light runs along the arm from the heart out to it. As the reader scrolls, most of the motes flow into the frames, each taking the
 * colour of the pixel it lands on, and the real photographs develop underneath. Then the choice: a few frames are kept, and the
 * motes of the rest come loose and drift back out into the sea. The story is driven by scroll progress (`ST`), so the reader sets
 * its pace and can run it backwards; the turn, the shimmer, the pointer and the bursts run on the clock.
 *
 * WebGL points, positions stepped on the CPU. Everything here is in CSS pixels of the canvas; the React side says where the heart
 * is, which arm each frame sits on, and reads back where the frames are each tick so the photographs can follow.
 */

export type Rect = { x: number; y: number; w: number; h: number };
/** A frame on the nebula: its arm and radius (px from the heart), its size, the grid of pixel colours (rgb 0..1, row-major) the
 * motes take on arrival, its place in the order the frames form, and whether it is turned down later. */
export type FrameSpec = { arm: number; r: number; w: number; h: number; cols: number; rows: number; colors: Float32Array | null; order: number; out: boolean };
/** Where a frame is right now: its centre (px) and the lean of the arm there (radians). */
export type Pose = { x: number; y: number; rot: number };
export type Palette = {
  mote: [number, number, number]; gleam: [number, number, number]; additive: boolean;
  /** Lifts every mote's alpha; the light theme needs more of it to read at all. */
  lift: number;
};

/** The shape of the nebula's arms: how much they twist over the reach, and how the whole is pressed into an ellipse. */
export const SPIRAL = { twist: 2.7, ex: 1.25, ey: 0.78 } as const;
/** A point on arm `arm` (0..2) at radius `r` around the heart (cx, cy), with the nebula turned by `spin` seconds. */
export function armPoint(cx: number, cy: number, arm: number, r: number, reach: number, spin = 0): [number, number] {
  const a = armAngle(arm, r, reach, spin);
  return [cx + Math.cos(a) * r * SPIRAL.ex, cy + Math.sin(a) * r * SPIRAL.ey];
}
/** The inside turns faster than the edge, so the arms keep slowly winding. */
export function omega(r: number, reach: number) { return 0.028 + 0.05 * (1 - Math.min(1, r / reach)); }
export function armAngle(arm: number, r: number, reach: number, spin: number) {
  return arm * ((Math.PI * 2) / 3) + (r / reach) * SPIRAL.twist + omega(r, reach) * spin;
}

/** Where in the scroll (0 to 1 over the pinned stretch) each part of the story happens. The DOM side keys to the same numbers. */
export const ST = {
  gather: 0.07, // the motes begin to flow into frames
  gatherEnd: 0.46,
  develop: 0.44, // the photographs come into focus and the motes that formed them fade
  developEnd: 0.54,
  decide: 0.57, // a few are kept; the motes of the rest come loose and drift away
  best: 0.75, // of the kept, one is best: the frames leave the arms
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

const THREAD = 64; // motes in the thread from the heart to a frame

// Per-mote data, struct-of-arrays. `kind` 0 = sea, 1 = frame cell, 2 = burst, 3 = thread.
type Store = {
  n: number; kind: Uint8Array; frame: Uint8Array;
  x: Float32Array; y: Float32Array; // where it is
  fx: Float32Array; fy: Float32Array; // its place in the sea (a frame mote starts there and, if turned down, goes back there)
  tx: Float32Array; ty: Float32Array; // its cell in a frame
  vx: Float32Array; vy: Float32Array; // sea drift / burst velocity / thread jitter
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
  private cells = 0;
  private fieldCount: number;
  private burstCount = 64;
  private burstNext = 0;
  private w = 1; private h = 1; private dpr = 1;
  private raf = 0;
  private running = false;
  private began = -1; // performance.now() when the sea first appeared
  private last = 0;
  private spin = 0; // seconds the nebula has been turning
  private target = 0; private P = 0; // scroll progress asked for, and the smoothed value actually shown
  private cx = 0.66; private cy = 0.5; // the nebula's heart, as fractions of the canvas
  private reach = 400; // the radius (px) its arms twist over; the motes run out to about 1.5 times this
  private px = -1e9; private py = -1e9;
  private pvx = 0; private pvy = 0; private pAt = 0; // pointer velocity, for the wake it leaves in the sea
  private parX = 0; private parY = 0; // the sea's slow slide against the pointer
  private palette: Palette;
  private lost = false;
  private poseList: Pose[] = [];
  /** Called once per frame with the smoothed progress and where every frame is, for the DOM side to follow. */
  onTick: ((p: number, poses: Pose[]) => void) | null = null;

  constructor(private canvas: HTMLCanvasElement, palette: Palette, opts: { field: number }) {
    this.palette = palette;
    this.fieldCount = opts.field;
    const gl = canvas.getContext("webgl", { alpha: true, antialias: false, premultipliedAlpha: true, depth: false, stencil: false, powerPreference: "low-power" });
    if (!gl) return;
    const sh = (type: number, src: string) => { const s = gl.createShader(type); if (!s) return null; gl.shaderSource(s, src); gl.compileShader(s); return s; };
    const p = gl.createProgram(), v = sh(gl.VERTEX_SHADER, VS), f = sh(gl.FRAGMENT_SHADER, FS);
    if (!p || !v || !f) return;
    gl.attachShader(p, v); gl.attachShader(p, f); gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) return;
    this.gl = gl; this.prog = p; this.buf = gl.createBuffer();
    this.loc = { pos: gl.getAttribLocation(p, "a_pos"), col: gl.getAttribLocation(p, "a_col"), size: gl.getAttribLocation(p, "a_size"), res: gl.getUniformLocation(p, "u_res"), dpr: gl.getUniformLocation(p, "u_dpr") };
    canvas.addEventListener("webglcontextlost", (e) => { e.preventDefault(); this.lost = true; this.stop(); });
  }

  get ok() { return !!this.gl; }
  get progress() { return this.P; }

  setPalette(p: Palette) { this.palette = p; }
  /** Where the reader has scrolled to, 0 to 1. The shown value follows it with a little lag, so scrubbing never judders. */
  setProgress(p: number) { this.target = clamp01(p); }
  /** Where the nebula's heart is (fractions of the canvas) and its reach in pixels. Set before the frames; the sea is laid out around it. */
  setFocus(cx: number, cy: number, reach: number) { this.cx = cx; this.cy = cy; this.reach = reach; }

  resize(w: number, h: number, dpr: number) {
    const first = this.w === 1 && this.h === 1;
    const sx = w / this.w, sy = h / this.h;
    this.w = w; this.h = h; this.dpr = Math.min(2, dpr);
    this.canvas.width = Math.round(w * this.dpr); this.canvas.height = Math.round(h * this.dpr);
    this.gl?.viewport(0, 0, this.canvas.width, this.canvas.height);
    if (!first) {
      // places in the sea keep their relative position when the canvas changes size
      const s = this.s, k = Math.min(sx, sy);
      for (let i = 0; i < s.n; i++) if (s.kind[i] === 0 || s.kind[i] === 1) { s.fx[i] *= sx; s.fy[i] *= sy; if (s.rad[i] >= 0) s.rad[i] *= k; if (s.kind[i] === 0) { s.x[i] *= sx; s.y[i] *= sy; } }
    }
  }

  /** Lays the motes out: the frames' cells first, then the sea, then the threads. Called when the frames are placed or move. */
  setFrames(frames: FrameSpec[]) {
    this.frames = frames;
    const cells = frames.reduce((a, f) => a + f.cols * f.rows, 0);
    const n = cells + this.fieldCount + frames.length * THREAD + this.burstCount;
    if (this.s.n !== n || this.cells !== cells) {
      this.cells = cells;
      this.s = store(n);
      this.data = new Float32Array(n * 7);
      this.seedSea(cells, cells + this.fieldCount);
      this.seedFrames();
      this.seedThreads();
    }
    this.poseList = frames.map(() => ({ x: 0, y: 0, rot: 0 }));
    this.retarget();
  }

  /** Where every frame is right now (px), in the order they were given. */
  poses(): Pose[] { return this.poseList; }

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
    const reach = this.reach;
    if (Math.random() < 0.22) {
      // the heart: dense, round, bright
      s.rad[i] = Math.abs(this.gauss()) * 0.16 * reach; s.ang[i] = Math.random() * Math.PI * 2;
      this.placeFromPolar(i);
      return;
    }
    // the arms: three, each a tight twist that loosens a little toward the edge
    const r = Math.min(1.5, 0.12 + Math.abs(this.gauss()) * 0.5 + Math.pow(Math.random(), 2.4) * 0.45);
    const arm = Math.floor(Math.random() * 3) * ((Math.PI * 2) / 3);
    const a = arm + r * SPIRAL.twist + this.gauss() * (0.09 + 0.14 * r);
    s.rad[i] = r * reach; s.ang[i] = a;
    this.placeFromPolar(i);
  }

  /** The nebula is an ellipse, wider than it is tall, around the heart. */
  private placeFromPolar(i: number) {
    const s = this.s;
    s.fx[i] = this.cx * this.w + Math.cos(s.ang[i]) * s.rad[i] * SPIRAL.ex;
    s.fy[i] = this.cy * this.h + Math.sin(s.ang[i]) * s.rad[i] * SPIRAL.ey;
  }

  private seedSea(from: number, to: number) {
    const s = this.s;
    for (let i = from; i < to; i++) {
      s.kind[i] = 0;
      this.seaPoint(i);
      s.x[i] = s.fx[i]; s.y[i] = s.fy[i];
      const a = Math.random() * Math.PI * 2, v = 2 + Math.random() * 5;
      s.vx[i] = Math.cos(a) * v; s.vy[i] = Math.sin(a) * v;
      s.size[i] = (1.2 + Math.random() * 2.2) * (0.45 + 1.15 * s.z[i]);
      s.seed[i] = Math.random() * 1000;
      // the heart of the nebula is warmer
      const heart = s.rad[i] >= 0 ? Math.max(0, 1 - s.rad[i] / (this.reach * 0.45)) : 0;
      s.amber[i] = Math.random() < 0.12 + 0.3 * heart ? 1 : 0;
    }
    for (let i = s.n - this.burstCount; i < s.n; i++) { s.kind[i] = 2; s.t0[i] = -1; s.size[i] = 2.2; }
  }

  private seedFrames() {
    const s = this.s;
    let i = 0;
    this.frames.forEach((f, fi) => {
      for (let r = 0; r < f.rows; r++) for (let c = 0; c < f.cols; c++, i++) {
        s.kind[i] = 1; s.frame[i] = fi;
        this.seaPoint(i);
        s.x[i] = s.fx[i]; s.y[i] = s.fy[i];
        s.size[i] = 2.2 + Math.random() * 1.0;
        s.seed[i] = Math.random() * 1000;
        const heart = s.rad[i] >= 0 ? Math.max(0, 1 - s.rad[i] / (this.reach * 0.45)) : 0;
        s.amber[i] = Math.random() < 0.1 + 0.28 * heart ? 1 : 0;
        // the frames form one after another over the gather stretch of the scroll, each condensing from its middle outward
        const dx = (c + 0.5) / f.cols - 0.5, dy = (r + 0.5) / f.rows - 0.5;
        const d = Math.sqrt(dx * dx + dy * dy) / 0.71;
        const span = ST.gatherEnd - ST.gather;
        s.t0[i] = ST.gather + span * (f.order * 0.075 + d * 0.22 + Math.random() * 0.08);
        s.dur[i] = span * (0.3 + Math.random() * 0.12);
      }
    });
  }

  /** The thread from the heart out to each frame: a line of small motes along its arm. */
  private seedThreads() {
    const s = this.s;
    let i = this.cells + this.fieldCount;
    this.frames.forEach((_, fi) => {
      for (let k = 0; k < THREAD; k++, i++) {
        s.kind[i] = 3; s.frame[i] = fi;
        s.rad[i] = k / (THREAD - 1); // how far along the thread, 0 heart .. 1 frame
        s.vx[i] = (Math.random() - 0.5) * 3.2; // a little to one side, so it is a thread and not a ruled line
        s.size[i] = 1.3 + Math.random() * 0.6 + s.rad[i] * 0.9;
        s.seed[i] = Math.random() * 1000;
        s.amber[i] = Math.random() < 0.55 ? 1 : 0;
      }
    });
  }

  /** Where each frame is now, from its arm and the turn so far; and the cells of its motes, turned with it. */
  private retarget() {
    const s = this.s, reach = this.reach, hx = this.cx * this.w, hy = this.cy * this.h;
    let i = 0;
    this.frames.forEach((f, fi) => {
      const [mx, my] = armPoint(hx, hy, f.arm, f.r, reach, this.spin);
      const [nx, ny] = armPoint(hx, hy, f.arm, f.r + 1, reach, this.spin);
      let rot = Math.atan2(ny - my, nx - mx);
      if (rot > Math.PI / 2) rot -= Math.PI; else if (rot < -Math.PI / 2) rot += Math.PI;
      rot = Math.max(-0.25, Math.min(0.25, rot));
      const pose = this.poseList[fi];
      if (pose) { pose.x = mx; pose.y = my; pose.rot = rot; }
      const cos = Math.cos(rot), sin = Math.sin(rot);
      for (let r = 0; r < f.rows; r++) for (let c = 0; c < f.cols; c++, i++) {
        const ox = ((c + 0.5) / f.cols - 0.5) * f.w, oy = ((r + 0.5) / f.rows - 0.5) * f.h;
        s.tx[i] = mx + ox * cos - oy * sin;
        s.ty[i] = my + ox * sin + oy * cos;
        const k = (r * f.cols + c) * 3;
        if (f.colors) { s.r[i] = f.colors[k]; s.g[i] = f.colors[k + 1]; s.b[i] = f.colors[k + 2]; }
        else { s.r[i] = s.g[i] = s.b[i] = -1; }
      }
    });
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
    // the nebula keeps turning, and the frames and their cells turn with it, until the frames leave the arms
    const leaving = smooth(ST.best - 0.02, ST.best + 0.08, P);
    this.spin += dt * (1 - leaving);
    this.retarget();
    this.onTick?.(P, this.poseList);
    const seaIn = smooth(0.05, 0.95, t);
    // once the frames have formed the sea steps back a little, so the photographs are what the eye lands on
    const seaDim = 1 - 0.28 * smooth(ST.gather + 0.12, ST.developEnd, P);
    const [mr, mg, mb] = p.mote, [ar, ag, ab] = p.gleam;
    const d = this.data;
    const W = this.w, H = this.h;
    const px = this.px, py = this.py, R = 190, R2 = R * R;
    // the whole sea slides a little against the pointer, the near motes more than the far: depth
    const hasPointer = px > -1e8;
    const pnx = hasPointer ? (px / W - 0.5) * 2 : 0, pny = hasPointer ? (py / H - 0.5) * 2 : 0;
    this.parX += ((hasPointer ? -pnx * 26 : 0) - this.parX) * Math.min(1, 3 * dt);
    this.parY += ((hasPointer ? -pny * 18 : 0) - this.parY) * Math.min(1, 3 * dt);
    const reach = this.reach, hx0 = this.cx * W, hy0 = this.cy * H;
    const span = ST.gatherEnd - ST.gather;
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
          s.ang[i] += omega(s.rad[i], reach) * dt * (1 - leaving);
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
        if (s.rad[i] >= 0) { s.ang[i] += omega(s.rad[i], reach) * dt * (1 - leaving); this.placeFromPolar(i); }
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
        // the thread from the heart to a frame, along its arm; lit as the frame begins to form, gone when the frame is turned down
        // or when the kept frames leave the arms
        const fr = this.frames[s.frame[i]];
        if (!fr) { a = 0; }
        else {
          const along = s.rad[i];
          const rr = fr.r * (0.05 + 0.95 * along);
          const ang = armAngle(fr.arm, rr, reach, this.spin);
          x = hx0 + Math.cos(ang) * rr * SPIRAL.ex - Math.sin(ang) * s.vx[i];
          y = hy0 + Math.sin(ang) * rr * SPIRAL.ey + Math.cos(ang) * s.vx[i];
          s.x[i] = x; s.y[i] = y;
          const begins = ST.gather + span * fr.order * 0.075;
          // it is drawn out from the heart toward the frame
          const drawn = smooth(begins - 0.02, begins + 0.1, P);
          const reached = along <= drawn ? 1 : 0;
          const end = fr.out ? smooth(ST.decide + fr.order * 0.01, ST.decide + fr.order * 0.01 + 0.08, P) : smooth(ST.best - 0.03, ST.best + 0.05, P);
          const flicker = 0.85 + 0.15 * Math.sin(t * 2.2 + s.seed[i]);
          a = seaIn * reached * (0.28 + 0.62 * along) * flicker * (1 - end);
          if (amber) { r = ar; g = ag; b = ab; } else { r = mr; g = mg; b = mb; }
        }
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
      d[j++] = x; d[j++] = y; d[j++] = r; d[j++] = g; d[j++] = b; d[j++] = Math.min(1, a * p.lift); d[j++] = size;
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
    ? { mote: [0.14, 0.15, 0.2], gleam, additive: false, lift: 1.6 }
    : { mote: [0.93, 0.93, 0.96], gleam, additive: true, lift: 1.05 };
}
