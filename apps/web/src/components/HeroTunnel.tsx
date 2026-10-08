import { useEffect, useRef } from "react";
import { useReducedMotion } from "motion/react";

/** Only photographs that hold up at any size; they are cut into a 4×3 atlas and each card picks a slot. */
const PHOTOS = [
  "/wall/01.webp", "/wall/02.webp", "/wall/03.webp", "/wall/06.webp", "/wall/11.webp", "/wall/13.webp",
  "/wall/15.webp", "/photos/village.webp", "/photos/st0.webp", "/photos/st1.webp", "/photos/boat.webp",
];
const COLS = 4, ROWS = 3, SLOT_W = 512, SLOT_H = 1024 / 3;

const RINGS = 22, PER = 11, SPACING = 2.9, ZNEAR = 3, DEPTH = RINGS * SPACING;
const RX = 11.5, RY = 6.4;
const COUNT = RINGS * PER;

const VERT = `
attribute vec2 aSlot; attribute vec4 aState;
varying vec2 vUv; varying vec2 vSlot; varying vec4 vState;
void main() {
  vUv = uv; vSlot = aSlot; vState = aState;
  gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0);
}`;
const FRAG = `
precision highp float;
uniform sampler2D uAtlas; uniform vec3 uBg; uniform vec3 uGold; uniform float uIntro; uniform vec2 uRes; uniform vec4 uZone;
varying vec2 vUv; varying vec2 vSlot; varying vec4 vState;
void main() {
  vec2 p = (vUv - 0.5) * vec2(1.5, 1.0);
  vec2 q = abs(p) - vec2(0.75, 0.5) + 0.07;
  float sd = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - 0.07;
  float aa = fwidth(sd) * 1.2;
  float mask = 1.0 - smoothstep(-aa, aa, sd);
  vec2 uv = vec2((vSlot.x + vUv.x) / ${COLS}.0, (${ROWS - 1}.0 - vSlot.y + vUv.y) / ${ROWS}.0);
  vec3 col = texture2D(uAtlas, uv).rgb;
  col = mix(uBg, col, clamp(vState.x, 0.0, 1.0));
  float rim = (1.0 - smoothstep(0.0, 0.03, -sd)) * vState.z;
  col = mix(col, uGold, rim * 0.95);
  vec2 sp = (vUv - vec2(0.9, 0.85)) * vec2(1.5, 1.0) / 0.075;
  float d = pow(abs(sp.x), 0.5) + pow(abs(sp.y), 0.5);
  float star = (1.0 - smoothstep(0.9, 1.1, d)) * vState.z;
  col = mix(col, vec3(1.0, 0.95, 0.78), star);
  col += uGold * exp(-d * 1.3) * vState.z * 0.5;
  // Photographs keep clear of the headline: wherever the text sits, whatever passes behind it steps right back.
  vec2 zs = (gl_FragCoord.xy / uRes - uZone.xy) / uZone.zw;
  float clear = mix(0.1, 1.0, smoothstep(0.8, 1.55, length(zs)));
  gl_FragColor = vec4(col, mask * vState.y * uIntro * clear);
}`;

const hexToRgb = (v: string, fallback: [number, number, number]): [number, number, number] => {
  const m = v.trim().match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i);
  if (!m) return fallback;
  let h = m[1];
  if (h.length === 3) h = h.split("").map((c) => c + c).join("");
  return [parseInt(h.slice(0, 2), 16) / 255, parseInt(h.slice(2, 4), 16) / 255, parseInt(h.slice(4), 16) / 255];
};
const smooth = (a: number, b: number, x: number) => { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); };

/**
 * The headline, in space: thousands of photographs stream toward you out of the dark, nearly all of them dim and passing by,
 * and the few the app would keep catch the light, take a gold rim and its mark. One instanced draw call for the whole tunnel.
 * The camera pushes in on arrival and leans toward the pointer; scrolling carries you faster and fades the scene out.
 * three.js is loaded on demand, so the headline and the page never wait for it.
 */
export default function HeroTunnel() {
  const reduce = useReducedMotion();
  const host = useRef<HTMLDivElement>(null);
  const cv = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let dead = false;
    let dispose = () => {};

    (async () => {
      const THREE = await import("three");
      if (dead) return;
      const canvas = cv.current!, box = host.current!;
      let renderer: InstanceType<typeof THREE.WebGLRenderer>;
      try { renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: "high-performance" }); }
      catch { return; }
      renderer.setClearColor(0x000000, 0);
      renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));

      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(52, 1.6, 0.1, 140);

      const atlas = document.createElement("canvas");
      atlas.width = 2048; atlas.height = 1024;
      const actx = atlas.getContext("2d")!;
      const texture = new THREE.CanvasTexture(atlas);
      texture.anisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());
      let loaded = 0;
      PHOTOS.forEach((src, i) => {
        const img = new Image();
        const done = () => { if (++loaded === PHOTOS.length) texture.needsUpdate = true; };
        img.onload = () => {
          const dw = SLOT_W, dh = SLOT_H, dx = (i % COLS) * SLOT_W, dy = Math.floor(i / COLS) * SLOT_H;
          const s = Math.max(dw / img.width, dh / img.height);
          const sw = dw / s, sh = dh / s;
          actx.drawImage(img, (img.width - sw) / 2, (img.height - sh) / 2, sw, sh, dx, dy, dw, dh);
          texture.needsUpdate = true; done();
        };
        img.onerror = done;
        img.src = src;
      });

      const geo = new THREE.PlaneGeometry(1, 1);
      const slotAttr = new THREE.InstancedBufferAttribute(new Float32Array(COUNT * 2), 2);
      const stateAttr = new THREE.InstancedBufferAttribute(new Float32Array(COUNT * 4), 4);
      geo.setAttribute("aSlot", slotAttr);
      geo.setAttribute("aState", stateAttr);
      const uniforms = {
        uAtlas: { value: texture },
        uBg: { value: new THREE.Vector3(0.06, 0.06, 0.07) },
        uGold: { value: new THREE.Vector3(0.91, 0.71, 0.36) },
        uIntro: { value: 0 },
        uRes: { value: new THREE.Vector2(1, 1) },
        uZone: { value: new THREE.Vector4(0.4, 0.5, 0.4, 0.3) },
      };
      const mat = new THREE.ShaderMaterial({ vertexShader: VERT, fragmentShader: FRAG, uniforms, transparent: true, depthWrite: true, side: THREE.DoubleSide });
      const mesh = new THREE.InstancedMesh(geo, mat, COUNT);
      mesh.frustumCulled = false;
      scene.add(mesh);

      const readTheme = () => {
        const cs = getComputedStyle(document.documentElement);
        const bg = hexToRgb(cs.getPropertyValue("--canvas"), [0.06, 0.06, 0.07]);
        const gold = hexToRgb(cs.getPropertyValue("--gleam"), [0.91, 0.71, 0.36]);
        uniforms.uBg.value.set(...bg); uniforms.uGold.value.set(...gold);
      };
      readTheme();
      const mo = new MutationObserver(readTheme);
      mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

      // Each card keeps its own place on its ring; a ring that passes the camera goes to the back and is dealt again.
      const ringZ = new Float32Array(RINGS), ringPhase = new Float32Array(RINGS);
      for (let k = 0; k < RINGS; k++) { ringZ[k] = ZNEAR - k * SPACING; ringPhase[k] = k * 0.41; }
      const slot = new Float32Array(COUNT), keep = new Float32Array(COUNT), jitter = new Float32Array(COUNT);
      const deal = (k: number) => {
        for (let j = 0; j < PER; j++) {
          const i = k * PER + j;
          slot[i] = Math.floor(Math.random() * PHOTOS.length);
          keep[i] = Math.random() < 0.075 ? 1 : 0;
          jitter[i] = (Math.random() - 0.5) * 0.5;
          slotAttr.setXY(i, slot[i] % COLS, Math.floor(slot[i] / COLS));
        }
        slotAttr.needsUpdate = true;
      };
      for (let k = 0; k < RINGS; k++) deal(k);

      const dummy = new THREE.Object3D();
      let mx = 0, my = 0, cx = 0, cy = 0, scrollK = 0, visible = true, raf = 0;
      const t0 = performance.now();
      let last = t0;

      const resize = () => {
        const r = box.getBoundingClientRect();
        const w = Math.max(1, Math.round(r.width)), h = Math.max(1, Math.round(r.height));
        renderer.setSize(w, h, false);
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        renderer.getDrawingBufferSize(uniforms.uRes.value);
        const text = document.querySelector("[data-hero-text]");
        if (text) {
          const tr = text.getBoundingClientRect();
          uniforms.uZone.value.set((tr.left + tr.width / 2 - r.left) / w, 1 - (tr.top + tr.height / 2 - r.top) / h, (tr.width / w) * 0.62, (tr.height / h) * 0.62);
        }
      };
      resize();
      const ro = new ResizeObserver(resize);
      ro.observe(box);
      document.fonts?.ready.then(resize);

      const frame = (now: number, still: boolean) => {
        const dt = still ? 0 : Math.min(0.05, (now - last) / 1000);
        last = now;
        const age = (now - t0) / 1000;
        const speed = (1.5 + scrollK * 5) * (still ? 0 : 1);
        // The camera pushes in on arrival, then leans toward the pointer.
        const push = 1 - Math.pow(1 - Math.min(1, age / 2.6), 3);
        cx += (mx - cx) * Math.min(1, dt * 3); cy += (my - cy) * Math.min(1, dt * 3);
        camera.position.set(cx * 1.5, -cy * 0.9, 9 - push * 3);
        camera.lookAt(cx * 0.5, -cy * 0.3, -20);
        uniforms.uIntro.value = still ? 1 : smooth(0.05, 1.3, age);

        for (let k = 0; k < RINGS; k++) {
          ringZ[k] += speed * dt;
          if (ringZ[k] > ZNEAR) { ringZ[k] -= DEPTH; deal(k); }
          const z = ringZ[k];
          const far = smooth(-70, -14, z), alphaFar = smooth(-62, -36, z), alphaNear = 1 - smooth(-3, 2.4, z);
          for (let j = 0; j < PER; j++) {
            const i = k * PER + j;
            const th = ringPhase[k] + (j / PER) * Math.PI * 2 + age * 0.018 + jitter[i] * 0.2;
            const kept = keep[i];
            dummy.position.set(Math.cos(th) * RX, Math.sin(th) * RY, z);
            dummy.rotation.set(Math.sin(th) * 0.34, -Math.cos(th) * 0.42, jitter[i] * 0.12);
            const s = 1 + kept * 0.2;
            dummy.scale.set(3.2 * s, (3.2 / 1.5) * s, 1);
            dummy.updateMatrix();
            mesh.setMatrixAt(i, dummy.matrix);
            stateAttr.setXYZW(i, kept ? 1 : 0.5 + 0.28 * far, alphaFar * alphaNear, kept, 0);
          }
        }
        mesh.instanceMatrix.needsUpdate = true;
        stateAttr.needsUpdate = true;
        renderer.render(scene, camera);
      };

      const loop = (now: number) => {
        raf = 0;
        if (dead || !visible) return;
        frame(now, false);
        raf = requestAnimationFrame(loop);
      };

      const onMove = (e: PointerEvent) => { mx = (e.clientX / window.innerWidth) * 2 - 1; my = (e.clientY / window.innerHeight) * 2 - 1; };
      const onScroll = () => {
        const r = box.getBoundingClientRect();
        scrollK = Math.min(1, Math.max(0, -r.top / Math.max(1, r.height)));
        canvas.style.opacity = String(1 - scrollK * 0.85);
      };
      const io = new IntersectionObserver(([e]) => {
        visible = e.isIntersecting;
        if (visible && !raf && !reduce) { last = performance.now(); raf = requestAnimationFrame(loop); }
      });
      io.observe(box);
      window.addEventListener("pointermove", onMove, { passive: true });
      window.addEventListener("scroll", onScroll, { passive: true });

      if (reduce) {
        // The finished state, once: the tunnel already open, no flight.
        const settle = () => frame(performance.now() + 4000, true);
        setTimeout(settle, 400); setTimeout(settle, 1200);
      } else {
        raf = requestAnimationFrame(loop);
      }

      dispose = () => {
        if (raf) cancelAnimationFrame(raf);
        io.disconnect(); ro.disconnect(); mo.disconnect();
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("scroll", onScroll);
        geo.dispose(); mat.dispose(); texture.dispose(); renderer.dispose();
      };
    })();

    return () => { dead = true; dispose(); };
  }, [reduce]);

  return (
    <div ref={host} aria-hidden className="pointer-events-none absolute inset-y-0 left-1/2 -z-10 w-screen -translate-x-1/2 overflow-hidden">
      <canvas ref={cv} className="h-full w-full" />
      {/* A soft scrim of the page's own colour keeps the headline readable over whatever is passing behind it. */}
      <div className="absolute inset-0" style={{ background: "radial-gradient(56% 52% at 36% 50%, color-mix(in srgb, var(--canvas) 86%, transparent), transparent 72%)" }} />
    </div>
  );
}
