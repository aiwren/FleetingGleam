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
      face_boxes: i % 4 === 0 ? [[0.3, 0.25, 0.14, 0.2], [0.55, 0.3, 0.13, 0.19]] : i % 4 === 1 ? [[0.42, 0.28, 0.16, 0.22]] : [],
      people: i % 4 === 0 ? [0, 1] : i % 4 === 1 ? [i % 8 === 1 ? 0 : 1] : [],
      hdr: i % 3 === 0, motion: i % 7 === 0,
    };
  });
  const best = new Map<number, Photo>();
  photos.forEach((p) => { const b = best.get(p.group); if (!b || p.score > b.score) best.set(p.group, p); });
  best.forEach((p) => { p.group_best = true; });
  return photos;
}

document.documentElement.dataset.mock = "1"; // the browser has no native vibrancy, so give the sidebar a base colour
const w = window as unknown as Record<string, unknown>;
const packState: Record<"standard" | "high" | "ultra" | "search" | "eyes" | "faces" | "scenetext", boolean> = { standard: true, high: false, ultra: false, search: false, eyes: false, faces: true, scenetext: true };
const emit = (event: string, payload: unknown) => (listeners.get(event) ?? []).forEach((h) => cbs.get(h)?.({ event, id: 0, payload }));
// lets a test play the part of the native layer (e.g. `__mockEmit("hdr-view", {...})`)
(window as unknown as { __mockEmit?: typeof emit }).__mockEmit = emit;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

let lastPhotos: Photo[] = [];

async function scan(dirs: string[], quiet: boolean): Promise<Photo[]> {
  const list = inDirs(dirs);
  const photos = makePhotos(list);
  lastPhotos = photos;
  const todo = photos.filter((p) => !cached.has(p.path));
  if (quiet || todo.length === 0) { list.forEach((s) => cached.add(s.path)); return photos; }
  const total = photos.length;
  let done = total - todo.length;
  emit("scan-info", { cpu_only: false, clip: "coreml-fp16", model: "ViT-L/14", vision: true, cores: 10 });
  emit("scan-progress", { done, total });
  { const known = photos.filter((p) => cached.has(p.path)); for (let i = 0; i < known.length; i += 250) emit("scan-photos", known.slice(i, i + 250)); }
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
/** The same shape prefs.rs sends: counts, whether the taste is in use, and the marks on the way to its full say. */
const prefsOf = (labels: number, keeps: number, refs = 0, refs_from = "") => {
  const active = keeps + refs >= 3 && labels - keeps >= 3;
  const trained = active ? labels + refs * 2 : 0;
  return { labels, keeps, rejects: labels - keeps, refs, refs_from, imported: 0, active, trained, strength: active ? Math.min(0.08 + trained / 300, 0.35) : 0, max_strength: 0.35, min_per_class: 3, half_at: 29, full_at: 81 };
};
const prefsInfo = () => prefsOf(labels, keeps);

type MockStyle = { id: string; name: string; is_default: boolean; info: ReturnType<typeof prefsOf> };
const now = Date.now();
const mockShares = [{ id: "n1", name: "Home NAS", host: "nas.local", user: "me", enabled: true, local: "/mock/nas/n1", connected: true }];
type MockTarget = { id: string; name: string; kind: "r2"; account: string; bucket: string; access_key: string; has_secret: boolean; last_ok: number; last_error: string | null };
const mockBackup = { configured: false, enabled: false, keep: 30, targets: [] as MockTarget[] };
const bkSync = () => { mockBackup.configured = mockBackup.targets.length > 0; return { ...mockBackup, targets: [...mockBackup.targets] }; };
const mockStyles: MockStyle[] = [
  { id: "default", name: "", is_default: true, info: prefsOf(34, 19) },
  { id: `s${now - 6 * 86400_000}`, name: "Street", is_default: false, info: prefsOf(12, 7, 40, "/Users/me/Photos/Street") },
  { id: `s${now - 1 * 86400_000}`, name: "Weddings", is_default: false, info: prefsOf(2, 1) },
];

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
      case "scan_cancel": return null;
      case "projects_load": return localStorage.getItem("fg.projects");
      case "projects_save": localStorage.setItem("fg.projects", String(args.json)); return null;
      case "projects_quarantine": localStorage.removeItem("fg.projects"); return "fg.projects.corrupt";
      case "nas_list": return [...mockShares];
      case "nas_add": {
        await sleep(600);
        if (String(args.url).includes("bad")) throw "the server cannot be reached (65)";
        mockShares.push({ id: `n${Date.now()}`, name: String(args.name || "NAS"), host: String(args.host), user: String(args.user ?? ""), enabled: true, local: "/mock/nas/new", connected: true });
        return [...mockShares];
      }
      case "nas_update": { await sleep(500); const s = mockShares.find((x) => x.id === args.id); if (s) Object.assign(s, { name: String(args.name || args.host), host: String(args.host), user: String(args.user ?? ""), connected: true }); return [...mockShares]; }
      case "nas_set_enabled": { const s = mockShares.find((x) => x.id === args.id); if (s) { s.enabled = !!args.enabled; s.connected = s.enabled; } return [...mockShares]; }
      case "nas_remove": { const i = mockShares.findIndex((s) => s.id === args.id); if (i >= 0) mockShares.splice(i, 1); return [...mockShares]; }
      case "nas_connect": { await sleep(500); const s = mockShares.find((x) => x.id === args.id); if (s) s.connected = true; return [...mockShares]; }
      case "nas_disconnect": { const s = mockShares.find((x) => x.id === args.id); if (s) s.connected = false; return [...mockShares]; }
      case "nas_ensure": return [];
      case "backup_status": return bkSync();
      case "backup_save_target": {
        await sleep(700);
        if (String(args.bucket).includes("bad")) throw "no such bucket in that account";
        const id = (args.id as string | null) || `t${Date.now()}`;
        const t: MockTarget = { id, name: String(args.name || `Cloudflare R2 · ${args.bucket}`), kind: "r2", account: String(args.account), bucket: String(args.bucket), access_key: String(args.accessKey), has_secret: true, last_ok: mockBackup.targets.find((x) => x.id === id)?.last_ok ?? 0, last_error: null };
        const i = mockBackup.targets.findIndex((x) => x.id === id); if (i >= 0) mockBackup.targets[i] = t; else mockBackup.targets.push(t);
        return bkSync();
      }
      case "backup_remove_target": mockBackup.targets = mockBackup.targets.filter((x) => x.id !== args.id); return bkSync();
      case "backup_set_options": { if (args.enabled != null) mockBackup.enabled = !!args.enabled; if (args.keep != null) mockBackup.keep = Number(args.keep); return bkSync(); }
      case "backup_now": await sleep(900); mockBackup.targets.forEach((x) => { x.last_ok = Math.floor(Date.now() / 1000); x.last_error = null; }); return bkSync();
      case "backup_restore": await sleep(900); return [{ styles: 2, examples: 40, refs: 10, skipped: 0 }, { theme: "system" }];
      case "nas_probe": { await sleep(600); if (String(args.host).includes("bad")) throw "the server cannot be reached"; return ["photos", "backup", "media"]; }
      case "nas_browse": {
        // 假的共享：根下两个年份文件夹，每个里面有几张照片
        await sleep(300);
        const path = String(args.path ?? "");
        // 根是共享列表；往下是文件夹和照片
        const tree: Record<string, { name: string; dir: boolean; size: number }[]> = {
          "": [{ name: "photos", dir: true, size: 0 }, { name: "backup", dir: true, size: 0 }, { name: "media", dir: true, size: 0 }],
          "photos": [{ name: "2024", dir: true, size: 0 }, { name: "2025", dir: true, size: 0 }, { name: "IMG_0001.jpg", dir: false, size: 4_200_000 }],
          "photos/2024": [{ name: "Lisbon", dir: true, size: 0 }, { name: "IMG_0100.jpg", dir: false, size: 5_100_000 }, { name: "IMG_0101.jpg", dir: false, size: 4_900_000 }],
          "photos/2025": [{ name: "Porto", dir: true, size: 0 }],
          "photos/2024/Lisbon": Array.from({ length: 12 }, (_, i) => ({ name: `IMG_${200 + i}.jpg`, dir: false, size: 6_000_000 })),
          "photos/2025/Porto": Array.from({ length: 7 }, (_, i) => ({ name: `IMG_${300 + i}.jpg`, dir: false, size: 5_500_000 })),
          "backup": Array.from({ length: 30 }, (_, i) => ({ name: `folder-${i + 1}`, dir: true, size: 0 })),
        };
        return { path, local: path ? `/mock/nas/${args.id}/${path}` : "", entries: tree[path] ?? [] };
      }
      case "taste_export": await sleep(300); return { styles: mockStyles.length, examples: 46, refs: 40, skipped: 2 };
      case "taste_import": await sleep(400); labels += 20; keeps += 11; return [{ styles: 1, examples: 20, refs: 0, skipped: 0 }, { theme: "system" }];
      // The native HDR layer exists only in the app; here the calls are just recorded (window.__hdrCalls) so the page's side can be checked.
      case "hdr_overlay_supported": return true;
      case "hdr_overlay_show": case "hdr_overlay_move": case "hdr_overlay_hide": {
        ((window as unknown as { __hdrCalls?: unknown[] }).__hdrCalls ??= []).push({ cmd, args: JSON.parse(JSON.stringify(args)) });
        return cmd === "hdr_overlay_show" ? true : null;
      }
      case "scene_text": {
        const pool = ["A green lawn with trees in the background", "A river with buildings along the shore", "A large white church with a wide staircase", "A cafe with tables and chairs", "A field of pink flowers", "A street with tall buildings"];
        const zh = !!args.zh, zhPool = ["绿色的草地，背景有树", "河边，沿岸有建筑", "白色的教堂和宽阔的台阶", "有桌椅的咖啡馆", "一片粉红色的花田", "有高楼的街道"];
        const reqs = args.scenes as { id: number }[];
        // most scenes are named at once (as the real vocabulary step does); the rest follow one by one
        const named = Math.floor(reqs.length * 0.8);
        setTimeout(() => emit("scene-texts", reqs.slice(0, named).map((r, i) => ({ id: r.id, text: (zh ? zhPool : pool)[i % pool.length] }))), 300);
        reqs.slice(named).forEach((r, i) => setTimeout(() => { emit("scene-text", { id: r.id, text: (zh ? zhPool : pool)[i % pool.length] }); emit("scene-text-progress", { done: i + 1, total: reqs.length - named }); }, 600 + i * 80));
        return null;
      }
      // A rough stand-in for the real (Rust-side) keep/review/reject logic: just enough to exercise the UI in a
      // plain browser. The actual ranking algorithm lives only in src-tauri/src/decide.rs.
      case "decide_photos": {
        const keepRatio = Number(args.keepRatio ?? 1);
        const overrides = (args.overrides ?? {}) as Record<string, string>;
        const sorted = [...lastPhotos].sort((a, b) => b.score - a.score);
        const keepN = Math.max(1, Math.round(sorted.length * keepRatio));
        const reviewN = Math.max(1, Math.round(sorted.length * 0.15));
        const out: Record<string, { state: string; manual: boolean; reasons: { k: string; p?: Record<string, number> }[] }> = {};
        sorted.forEach((p, i) => {
          const good = p.score >= 0.7;
          const auto = good && i < keepN ? "keep" : good || i < keepN + reviewN ? "review" : "reject";
          const manual = Object.prototype.hasOwnProperty.call(overrides, p.path);
          const state = manual ? overrides[p.path] : auto;
          out[p.path] = { state, manual, reasons: [{ k: state === "keep" ? "reason.keepSolo" : state === "review" ? "reason.nearBest" : "reason.belowLine", p: { score: Math.round(p.score * 100) } }] };
        });
        return out;
      }
      case "record_label": {
        const cur = args.label;
        if (cur !== null) { labels++; if (cur) keeps++; }
        return prefsInfo();
      }
      case "export_photos": {
        const paths = args.paths as string[];
        for (let i = 0; i < paths.length; i++) { emit("export-progress", { done: i + 1, total: paths.length }); await sleep(60); }
        // 落在 NAS 镜像里的：假装传到了 NAS 上
        const nas = String(args.dest).match(/\/nas\/[^/]+\/(.+)$/);
        return nas ? { copied: paths.length, skipped: 0, dest: `smb://nas.local/${nas[1]}`, remote: true } : { copied: paths.length, skipped: 0, dest: args.dest };
      }
      case "trash_photos": {
        const paths = args.paths as string[];
        for (const p of paths) { const i = SRC.findIndex((s) => s.path === p); if (i >= 0) SRC.splice(i, 1); cached.delete(p); }
        await sleep(300);
        return { moved: paths.length, failed: [] };
      }
      case "reveal": return null;
      case "large_preview": return (args.path as string);
      case "set_rotation": return null;
      case "backup_peek": return null;
      case "photo_info": return { format: "JPEG", bytes: 9_400_000, color_space: "sRGB", bit_depth: "8, 8, 8", camera: "FUJIFILM X-T5", lens: "XF35mmF1.4 R", focal: "35 mm", aperture: "f/2", shutter: "1/250 s", iso: "400", software: null, taken: "2026-04-17 18:22:31" };
      case "machine_info": return { memory_gb: 32, cores: 10 };
      case "packs_status": return (["standard", "high", "ultra", "search", "eyes", "faces", "scenetext"] as const).map((id) => ({ id, installed: packState[id], bytes: 0 }));
      case "pack_size": return { standard: 266_000_000, high: 609_000_000, ultra: 870_000_000, search: 139_000_000, eyes: 7_000_000, faces: 39_000_000, scenetext: 400_000_000 }[String(args.id) as "standard"];
      case "pack_download": {
        for (let i = 1; i <= 10; i++) { await sleep(150); emit("model-progress", { pack: args.id, file: "x", done: i * 60_900_000, total: 609_000_000 }); }
        packState[String(args.id) as "standard"] = true;
        return null;
      }
      case "pack_cancel": return null;
      case "pack_remove": packState[String(args.id) as "standard"] = false; return null;
      // Real Tauri IPC always hands back a freshly JSON-deserialized array, so `useSyncExternalStore`'s
      // reference check sees a change on every call; mutating mockStyles in place and returning it directly
      // would not, so every mutator here returns a shallow copy to match that behavior.
      case "styles_list": return [...mockStyles];
      case "style_create": {
        mockStyles.push({ id: `s${Date.now()}`, name: String(args.name ?? ""), is_default: false, info: prefsOf(0, 0) });
        return [...mockStyles];
      }
      case "style_rename": {
        const s = mockStyles.find((x) => x.id === args.id);
        if (s) s.name = String(args.name ?? "");
        return [...mockStyles];
      }
      case "style_delete": {
        const i = mockStyles.findIndex((x) => x.id === args.id);
        if (i >= 0) mockStyles.splice(i, 1);
        return [...mockStyles];
      }
      case "style_set_default": {
        for (const s of mockStyles) s.is_default = s.id === args.id;
        return [...mockStyles];
      }
      case "learn_from_folder": {
        const s = mockStyles.find((x) => x.id === args.style) ?? mockStyles.find((x) => x.is_default);
        if (s) { s.info.refs = 24; s.info.refs_from = String(args.dir); s.info.active = true; }
        return prefsInfo();
      }
      case "search_status": return { total: inDirs(args.dirs as string[]).length, ready: inDirs(args.dirs as string[]).length, available: packState.search && packState.standard };
      case "search_index": return 0;
      case "search_photos": {
        const words = String(args.query).toLowerCase();
        return inDirs(args.dirs as string[]).filter((_, i) => (i + words.length) % 3 === 0).map((s, i) => ({ path: s.path, name: s.path.split("/").pop(), thumb: s.path.replace(".jpg", ".thumb"), width: s.w, height: s.h, score: 0.3 - i * 0.001 }));
      }
      case "preview_thumbs": return (args.paths as string[]).map((p) => p.replace(".jpg", ".thumb"));
      case "prefs_info": return prefsInfo();
      case "prefs_reset": {
        const s = mockStyles.find((x) => x.id === args.style);
        if (s) { s.info = prefsOf(0, 0); }
        else { labels = 0; keeps = 0; }
        return prefsInfo();
      }
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
