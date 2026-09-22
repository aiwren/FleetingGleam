/**
 * Dev-only harness: runs the UI in a plain browser (`/?mock`) with procedurally painted "photos"
 * and a simulated analysis, so motion and layout can be checked without Tauri. Never imported in production.
 */
import type { Photo } from "../types";

type Fn = (...a: unknown[]) => unknown;
const cbs = new Map<number, Fn>();
const listeners = new Map<string, number[]>();
let nextId = 1;
const images = new Map<string, string>();

function rng(seed: number) {
  let s = seed * 9301 + 49297;
  return () => ((s = (s * 9301 + 49297) % 233280) / 233280);
}

const PALETTES = [
  ["#ffb36b", "#e0566f", "#3a2358"], ["#9fd8ff", "#4f86d6", "#132a52"], ["#ffd98a", "#f08a5d", "#5b2a4e"],
  ["#bfe9d8", "#4fa39a", "#12384a"], ["#f6c1d4", "#b667a8", "#2c2350"], ["#ffe6a7", "#e9a23b", "#5a3a1e"],
  ["#d2e1ff", "#7b8fe0", "#221f4d"], ["#ffcfa3", "#d9645a", "#39203f"],
];

function paint(seed: number, w: number, h: number, shift = 0): string {
  const r = rng(seed);
  const c = document.createElement("canvas");
  c.width = w; c.height = h;
  const g = c.getContext("2d")!;
  const [a, b, d] = PALETTES[seed % PALETTES.length];
  const sky = g.createLinearGradient(0, 0, 0, h);
  sky.addColorStop(0, d); sky.addColorStop(0.55, b); sky.addColorStop(1, a);
  g.fillStyle = sky; g.fillRect(0, 0, w, h);
  const sx = w * (0.3 + r() * 0.4) + shift, sy = h * (0.42 + r() * 0.12);
  const glow = g.createRadialGradient(sx, sy, 0, sx, sy, w * 0.5);
  glow.addColorStop(0, "rgba(255,240,210,.95)"); glow.addColorStop(0.15, "rgba(255,210,150,.5)"); glow.addColorStop(1, "rgba(255,180,120,0)");
  g.fillStyle = glow; g.fillRect(0, 0, w, h);
  for (let layer = 0; layer < 3; layer++) {
    const base = h * (0.62 + layer * 0.1), amp = h * (0.1 - layer * 0.02), ph = r() * 6, fr = 1.5 + r() * 2;
    g.beginPath(); g.moveTo(0, h);
    for (let x = 0; x <= w; x += 6) g.lineTo(x, base - Math.sin(x / w * fr * Math.PI + ph + shift * 0.01) * amp - Math.sin(x / w * 9 + ph) * amp * 0.25);
    g.lineTo(w, h); g.closePath();
    g.fillStyle = `rgba(${18 - layer * 4},${14 - layer * 3},${34 - layer * 6},${0.55 + layer * 0.2})`; g.fill();
  }
  for (let i = 0; i < 26; i++) {
    const x = r() * w, y = r() * h * 0.7, rad = 4 + r() * 22;
    g.beginPath(); g.arc(x, y, rad, 0, Math.PI * 2);
    g.fillStyle = `rgba(255,${200 + r() * 55 | 0},${140 + r() * 80 | 0},${0.05 + r() * 0.12})`; g.fill();
  }
  return c.toDataURL("image/jpeg", 0.86);
}

const SIZES: [number, number][] = [[960, 640], [640, 960], [1024, 576], [800, 800], [960, 640], [1024, 576]];
const FOLDERS: Record<string, { count: number; seedOffset: number }> = {
  "/mock/Lisbon": { count: 30, seedOffset: 0 },
  "/mock/Porto": { count: 22, seedOffset: 5 },
  "/mock/Big": { count: 300, seedOffset: 2 }, // for performance checks; pre-analysed so it opens instantly
  "/mock/Huge": { count: 3000, seedOffset: 3 }, // 3000 photos: windowing and lazy-loading checks
};
type Src = { path: string; seed: number; shift: number; w: number; h: number };
const SRC: Src[] = [];
for (const [dir, { count, seedOffset }] of Object.entries(FOLDERS)) {
  for (let i = 0; i < count; i++) {
    const seed = (Math.floor(i / 2.6) + seedOffset) % 11;
    const [w, h] = SIZES[seed % SIZES.length];
    SRC.push({ path: `${dir}/IMG_${String(4100 + i)}.jpg`, seed, shift: (i % 3) * 14, w, h });
  }
}
const inDirs = (dirs: string[]) => SRC.filter((s) => dirs.some((d) => s.path.startsWith(d + "/")));
const cached = new Set<string>(SRC.filter((s) => s.path.startsWith("/mock/Big/") || s.path.startsWith("/mock/Huge/")).map((s) => s.path));

function makePhotos(list: Src[]): Photo[] {
  const rand = rng(7);
  const photos: Photo[] = list.map((s, i) => {
    const sharp = 0.35 + rand() * 0.65, expo = 0.6 + rand() * 0.4, ai = 0.4 + rand() * 0.5;
    const flags: string[] = [];
    if (sharp < 0.42) flags.push("blur");
    if (i === 17) flags.push("utility");
    const score = Math.max(0, Math.min(1, 0.45 * ai + 0.35 * sharp + 0.2 * expo)) * (flags.includes("utility") ? 0.3 : flags.includes("blur") ? 0.6 : 1);
    return {
      path: s.path, name: s.path.split("/").pop()!, thumb: s.path.replace(".jpg", ".thumb"), width: s.w, height: s.h,
      sharpness: sharp * 200, brightness: 0.45, clipped_high: 0.02, clipped_low: 0.01,
      sharp_score: sharp, expo_score: expo, tech_score: 0.65 * sharp + 0.35 * expo,
      clip_score: ai, sys_score: ai - 0.2, ai_score: ai, pref_score: null, face_count: i % 4 === 0 ? 2 : 0, face_min: i % 4 === 0 ? 0.3 : null,
      utility: flags.includes("utility"), flags, eyes_open: null, kind: i % 3 === 0 ? "portrait" : "people", taken_at: 1_780_000_000 + i * 300, score, group: Math.floor(i / 2.6), scene: Math.floor(i / 5), group_best: false,
    };
  });
  const best = new Map<number, Photo>();
  photos.forEach((p) => { const b = best.get(p.group); if (!b || p.score > b.score) best.set(p.group, p); });
  best.forEach((p) => { p.group_best = true; });
  return photos;
}

document.documentElement.dataset.mock = "1"; // the browser has no native vibrancy, so give the sidebar a base colour
const w = window as unknown as Record<string, unknown>;
const packState: Record<"standard" | "high" | "ultra" | "search" | "eyes", boolean> = { standard: true, high: false, ultra: false, search: false, eyes: false };
const emit = (event: string, payload: unknown) => (listeners.get(event) ?? []).forEach((h) => cbs.get(h)?.({ event, id: 0, payload }));
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function scan(dirs: string[], quiet: boolean): Promise<Photo[]> {
  const list = inDirs(dirs);
  const photos = makePhotos(list);
  const todo = photos.filter((p) => !cached.has(p.path));
  if (quiet || todo.length === 0) { list.forEach((s) => cached.add(s.path)); return photos; }
  const total = photos.length;
  let done = total - todo.length;
  emit("scan-info", { cpu_only: false, clip: "coreml-fp16", model: "ViT-L/14", vision: true, cores: 10 });
  emit("scan-progress", { done, total });
  for (const p of photos) if (cached.has(p.path)) emit("scan-photo", p);
  const order = todo.map((p) => photos.indexOf(p));
  const worker = async () => {
    while (order.length) {
      const i = order.shift()!;
      const p = photos[i];
      for (const [stage, pct, ms] of [["decode", 8, 260], ["quality", 30, 320], ["ai", 50, 520], ["system", 70, 420], ["thumb", 92, 200]] as const) {
        emit("scan-stage", stage === "quality"
          ? { path: p.path, stage, pct, thumb: p.thumb, width: p.width, height: p.height }
          : { path: p.path, stage, pct, thumb: null, width: null, height: null });
        await sleep(ms + Math.random() * 180);
      }
      done++;
      cached.add(p.path);
      emit("scan-photo", p);
      emit("scan-progress", { done, total });
    }
  };
  await Promise.all([worker(), worker(), worker(), worker()]);
  return photos;
}

let dialogCalls = 0;
let labels = 0, keeps = 0;
const prefsInfo = () => ({ labels, keeps, active: labels >= 6 && keeps >= 3 && labels - keeps >= 3 });

w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
w.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
  transformCallback(cb: Fn, once?: boolean) {
    const id = nextId++;
    cbs.set(id, (...a) => { if (once) cbs.delete(id); return cb(...a); });
    return id;
  },
  unregisterCallback: (id: number) => cbs.delete(id),
  convertFileSrc: (p: string) => images.get(p) ?? p,
  async invoke(cmd: string, args: Record<string, unknown> = {}) {
    switch (cmd) {
      case "plugin:event|listen": {
        const ev = args.event as string;
        listeners.set(ev, [...(listeners.get(ev) ?? []), args.handler as number]);
        return args.handler;
      }
      case "plugin:event|unlisten": {
        const ev = args.event as string;
        listeners.set(ev, (listeners.get(ev) ?? []).filter((h) => h !== args.eventId));
        return null;
      }
      case "plugin:dialog|open": { const f = new URLSearchParams(location.search).get("folder"); return f ? `/mock/${f}` : ["/mock/Lisbon", "/mock/Porto"][dialogCalls++ % 2]; }
      case "probe_folder": {
        const list = inDirs(args.dirs as string[]);
        await sleep(250);
        const files = list.map((s) => ({ path: s.path, width: s.w, height: s.h, cached: cached.has(s.path) }));
        return { files, bytes: files.filter((f) => !f.cached).length * 9_000_000, uncached: files.filter((f) => !f.cached).length };
      }
      case "scan_project": return scan(args.dirs as string[], !!args.quiet);
      case "record_label": {
        const cur = args.label;
        if (cur !== null) { labels++; if (cur) keeps++; }
        return prefsInfo();
      }
      case "export_photos": {
        const paths = args.paths as string[];
        for (let i = 0; i < paths.length; i++) { emit("export-progress", { done: i + 1, total: paths.length }); await sleep(60); }
        return { copied: paths.length, skipped: 0, dest: args.dest };
      }
      case "trash_photos": {
        const paths = args.paths as string[];
        for (const p of paths) { const i = SRC.findIndex((s) => s.path === p); if (i >= 0) SRC.splice(i, 1); cached.delete(p); }
        await sleep(300);
        return { moved: paths.length, failed: [] };
      }
      case "reveal": return null;
      case "large_preview": return (args.path as string);
      case "photo_info": return { format: "JPEG", bytes: 9_400_000, color_space: "sRGB", bit_depth: "8, 8, 8", camera: "FUJIFILM X-T5", lens: "XF35mmF1.4 R", focal: "35 mm", aperture: "f/2", shutter: "1/250 s", iso: "400", software: null, taken: "2026-04-17 18:22:31" };
      case "packs_status": return (["standard", "high", "ultra", "search", "eyes"] as const).map((id) => ({ id, installed: packState[id], bytes: 0 }));
      case "pack_size": return { standard: 266_000_000, high: 609_000_000, ultra: 870_000_000, search: 139_000_000, eyes: 7_000_000 }[String(args.id) as "standard"];
      case "pack_download": {
        for (let i = 1; i <= 10; i++) { await sleep(150); emit("model-progress", { pack: args.id, file: "x", done: i * 60_900_000, total: 609_000_000 }); }
        packState[String(args.id) as "standard"] = true;
        return null;
      }
      case "pack_cancel": return null;
      case "pack_remove": packState[String(args.id) as "standard"] = false; return null;
      case "styles_list": return [{ id: "default", name: "", is_default: true, info: { labels: 0, keeps: 0, refs: 0, refs_from: "", active: false } }];
      case "style_create": case "style_rename": case "style_delete": case "style_set_default":
        return [{ id: "default", name: "", is_default: true, info: { labels: 0, keeps: 0, refs: 0, refs_from: "", active: false } }, { id: "s1", name: String(args.name ?? "Street"), is_default: false, info: { labels: 0, keeps: 0, refs: 0, refs_from: "", active: false } }];
      case "learn_from_folder": return { labels: 0, keeps: 0, refs: 24, refs_from: String(args.dir), active: true };
      case "search_status": return { total: inDirs(args.dirs as string[]).length, ready: inDirs(args.dirs as string[]).length, available: true };
      case "search_index": return 0;
      case "search_photos": {
        const words = String(args.query).toLowerCase();
        return inDirs(args.dirs as string[]).filter((_, i) => (i + words.length) % 3 === 0).map((s, i) => ({ path: s.path, name: s.path.split("/").pop(), thumb: s.path.replace(".jpg", ".thumb"), width: s.w, height: s.h, score: 0.3 - i * 0.001 }));
      }
      case "preview_thumbs": return (args.paths as string[]).map((p) => p.replace(".jpg", ".thumb"));
      case "prefs_info": return prefsInfo();
      case "prefs_reset": labels = 0; keeps = 0; return prefsInfo();
      case "set_locale": return null;
      case "cache_size": return 48_300_000;
      case "clear_cache": return null;
      case "plugin:app|version": return "0.1.0";
      default: return null;
    }
  },
};

/** Real photographs, when `public/dev-photos/<seed>.jpg` exists (git-ignored; used for screenshots). A burst is the same picture nudged a little. */
async function photo(seed: number, w: number, h: number, shift: number): Promise<string | null> {
  try {
    const img = new Image();
    img.src = `/dev-photos/${seed}.jpg`;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = w; c.height = h;
    const g = c.getContext("2d")!;
    const zoom = 1 + shift * 0.004, dx = shift * 0.6, dy = shift * 0.25;
    g.drawImage(img, -(w * (zoom - 1)) / 2 + dx, -(h * (zoom - 1)) / 2 + dy, w * zoom, h * zoom);
    return c.toDataURL("image/jpeg", 0.88);
  } catch { return null; }
}

// Paint everything up front so images are ready the instant the UI asks for them.
const painted = new Map<string, string>();
for (const s of SRC) {
  const key = `${s.seed}|${s.w}|${s.h}|${s.shift}`;
  const url = painted.get(key) ?? (await photo(s.seed, s.w, s.h, s.shift)) ?? paint(s.seed, s.w, s.h, s.shift);
  painted.set(key, url);
  images.set(s.path, url);
  images.set(s.path.replace(".jpg", ".thumb"), url);
}
