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
  "/mock/Samples": { count: 10, seedOffset: 7 }, // 起始屏的"试试示例照片"（samples_folder）
  "/mock/nas/n1/photos/Lisbon": { count: 12, seedOffset: 1 }, // NAS「Home NAS」上的项目（本机镜像），离线时照样打开
};
type Src = { path: string; seed: number; shift: number; w: number; h: number };
const SRC: Src[] = [];
/** 移到废纸篓的模拟照片，撤销时放回 SRC。 */
const trashed = new Map<string, (typeof SRC)[number]>();
for (const [dir, { count, seedOffset }] of Object.entries(FOLDERS)) {
  for (let i = 0; i < count; i++) {
    const seed = (Math.floor(i / 2.6) + seedOffset) % 11;
    const [w, h] = SIZES[seed % SIZES.length];
    SRC.push({ path: `${dir}/IMG_${String(4100 + i)}.jpg`, seed, shift: (i % 3) * 14, w, h });
  }
}
const inDirs = (dirs: string[]) => SRC.filter((s) => dirs.some((d) => s.path.startsWith(d + "/")));
const cached = new Set<string>(SRC.filter((s) => s.path.startsWith("/mock/Big/") || s.path.startsWith("/mock/Huge/") || s.path.startsWith("/mock/nas/")).map((s) => s.path));

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
// `?fresh`：一台刚装好的电脑，一个模型都没下（看第一次分析的询问框）
const fresh = new URLSearchParams(location.search).has("fresh");
const packState: Record<"standard" | "high" | "ultra" | "search" | "eyes" | "faces" | "scenetext", boolean> = { standard: !fresh, high: false, ultra: false, search: false, eyes: false, faces: !fresh, scenetext: !fresh };
const emit = (event: string, payload: unknown) => (listeners.get(event) ?? []).forEach((h) => cbs.get(h)?.({ event, id: 0, payload }));
// lets a test play the part of the native layer (e.g. `__mockEmit("hdr-view", {...})`)
(window as unknown as { __mockEmit?: typeof emit }).__mockEmit = emit;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

let lastPhotos: Photo[] = [];
/** decide_photos 的紧凑格式：上次的路径表、版本和整张表（理由从这里取）。 */
let decideRev = 0;
let decidePaths: string[] = [];
let decideTable: Record<string, MockDecision> = {};

type MockDecision = { state: string; manual: boolean; reasons: { k: string; p?: Record<string, number> }[] };
/** 和 "decide_photos" 同一套粗糙的取舍（真正的算法只在 decide.rs）：分数排名，前面的保留，中间的待复查，其余淘汰。 */
function judge(photos: Photo[], keepRatio: number, overrides: Record<string, string>): Record<string, MockDecision> {
  const sorted = [...photos].sort((a, b) => b.score - a.score);
  const keepN = Math.max(1, Math.round(sorted.length * keepRatio));
  const reviewN = Math.max(1, Math.round(sorted.length * 0.15));
  const out: Record<string, MockDecision> = {};
  sorted.forEach((p, i) => {
    const good = p.score >= 0.7;
    const auto = good && i < keepN ? "keep" : good || i < keepN + reviewN ? "review" : "reject";
    const manual = Object.prototype.hasOwnProperty.call(overrides, p.path);
    const state = manual ? overrides[p.path] : auto;
    out[p.path] = { state, manual, reasons: [{ k: state === "keep" ? "reason.keepSolo" : state === "review" ? "reason.nearBest" : "reason.belowLine", p: { score: Math.round(p.score * 100) } }] };
  });
  return out;
}

/** 和 lib.rs 的 scan_project 一样：真正的分析不再整包回照片，只回事件送过之后又变了的字段（这里就是分组、打分那几样）。 */
const slim = (photos: Photo[]) => photos.map((p) => ({ path: p.path, score: p.score, group: p.group, scene: p.scene, group_best: p.group_best }));

async function scan(dirs: string[], quiet: boolean, live?: { keep_ratio?: number; overrides?: Record<string, string> } | null, force = false): Promise<Photo[] | ReturnType<typeof slim>> {
  const list = inDirs(dirs);
  // "重新分析"：当作都没分析过
  if (force) list.forEach((s) => cached.delete(s.path));
  const photos = makePhotos(list);
  lastPhotos = photos;
  const todo = photos.filter((p) => !cached.has(p.path));
  if (quiet) { list.forEach((s) => cached.add(s.path)); return photos; }
  const total = photos.length;
  let done = total - todo.length;
  emit("scan-info", { cpu_only: false, clip: "coreml-fp16", model: "ViT-L/14", vision: true, cores: 10 });
  emit("scan-progress", { done, total });
  { const known = photos.filter((p) => cached.has(p.path)); for (let i = 0; i < known.length; i += 250) emit("scan-photos", known.slice(i, i + 250)); }
  if (todo.length === 0) return slim(photos);
  const order = todo.map((p) => photos.indexOf(p));
  // 边分析边判（引擎的判定线程）：每隔一会儿把已出结果的照片整体判一遍，只发变化的
  const judged = new Map<string, string>();
  let dirty = true;
  const judgeNow = () => {
    if (!live || !dirty) return;
    dirty = false;
    const done = photos.filter((p) => cached.has(p.path));
    const d = judge(done, live.keep_ratio ?? 1, live.overrides ?? {});
    const changed = Object.entries(d).filter(([path, dec]) => judged.get(path) !== dec.state).map(([path, decision]) => { judged.set(path, decision.state); return { path, decision }; });
    if (changed.length) emit("scan-decide", changed);
  };
  const judgeTimer = setInterval(judgeNow, 500);
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
      dirty = true;
    }
  };
  await Promise.all([worker(), worker(), worker(), worker()]);
  clearInterval(judgeTimer);
  judgeNow();
  return slim(photos);
}

let dialogCalls = 0;
let dlCancel = false;
const mirrorBytes: Record<string, number> = {};
let exportCancel = false;
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
        const offline = !!(window as unknown as { __mockNasDown?: string }).__mockNasDown && (args.dirs as string[]).some((d) => d.startsWith("/mock/nas/"));
        return { files, bytes: files.filter((f) => !f.cached).length * 9_000_000, uncached: files.filter((f) => !f.cached).length, offline };
      }
      case "scan_project": return scan(args.dirs as string[], !!args.quiet, args.live as { keep_ratio?: number; overrides?: Record<string, string> } | null, !!args.force);
      case "scan_cancel": return null;
      case "projects_load": return localStorage.getItem("fg.projects");
      case "projects_save": localStorage.setItem("fg.projects", String(args.json)); return null;
      case "projects_quarantine": localStorage.removeItem("fg.projects"); return "fg.projects.corrupt";
      case "nas_list": { const down = (window as unknown as { __mockNasDown?: string }).__mockNasDown; return mockShares.map((s) => ({ ...s, connected: s.connected && !down, error: down ?? null })); }
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
      // `window.__mockNasDown = "auth:…"`（或 net: / denied:）：Home NAS 连不上，原因按约定的前缀给
      case "nas_ensure": {
        const down = (window as unknown as { __mockNasDown?: string }).__mockNasDown;
        return mockShares.filter((s) => (args.dirs as string[]).some((d) => d.startsWith(s.local + "/"))).map((s) => ({ ...s, connected: s.connected && !down, error: down ?? null }));
      }
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
      // NAS 副本（第 31 条）：Home NAS 的镜像占 12.4 GB，清掉就是 0
      case "nas_mirror_usage": return mockShares.map((s) => ({ id: s.id, name: s.name, bytes: mirrorBytes[s.id] ?? 12_400_000_000 }));
      case "nas_mirror_clear": { const freed = mirrorBytes[String(args.id)] ?? 12_400_000_000; mirrorBytes[String(args.id)] = 0; await sleep(400); return freed; }
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
      // 和 lib.rs 一样回紧凑格式：路径表变了才发，状态一张一个字符，理由另取（decide_reasons）
      case "decide_photos": {
        const tbl = judge(lastPhotos, Number(args.keepRatio ?? 1), (args.overrides ?? {}) as Record<string, string>);
        const paths = Object.keys(tbl).sort();
        if (paths.join("\n") !== decidePaths.join("\n")) { decideRev++; decidePaths = paths; }
        decideTable = tbl;
        return { rev: decideRev, paths: args.knownRev === decideRev ? undefined : paths, states: paths.map((p) => ({ keep: "k", review: "r", reject: "x" } as Record<string, string>)[tbl[p].state]).join(""), manual: paths.flatMap((p, i) => (tbl[p].manual ? [i] : [])) };
      }
      case "decide_reasons": return decideTable[String(args.path)]?.reasons ?? null;
      case "record_label": {
        const cur = args.label;
        if (cur !== null) { labels++; if (cur) keeps++; }
        return prefsInfo();
      }
      case "export_photos": {
        // 每张 300 ms，可以中途停（export_cancel）；第 2 张"目标里已经有了"，第 4、6 张按约定的错误码失败
        const paths = args.paths as string[];
        exportCancel = false;
        let copied = 0, skipped = 0, cancelled = false;
        const failed: { path: string; error: string }[] = [];
        for (let i = 0; i < paths.length; i++) {
          if (exportCancel) { cancelled = true; break; }
          await sleep(300);
          if (i === 1) skipped++;
          else if (i === 3) failed.push({ path: paths[i], error: "space:9400000:1200000" });
          else if (i === 5) failed.push({ path: paths[i], error: "denied:Operation not permitted (os error 1)" });
          else copied++;
          emit("export-progress", { done: i + 1, total: paths.length });
        }
        // 落在 NAS 镜像里的：假装传到了 NAS 上
        const nas = String(args.dest).match(/\/nas\/[^/]+\/(.+)$/);
        return nas ? { copied, skipped, failed, cancelled, dest: `smb://nas.local/${nas[1]}`, remote: true } : { copied, skipped, failed, cancelled, dest: args.dest };
      }
      case "export_cancel": exportCancel = true; return null;
      case "trash_photos": {
        const paths = args.paths as string[];
        const undo: { path: string; at: string }[] = [];
        for (const p of paths) {
          const i = SRC.findIndex((s) => s.path === p);
          if (i >= 0) { trashed.set(p, SRC[i]); SRC.splice(i, 1); undo.push({ path: p, at: `/Users/me/.Trash/${p.split("/").pop()}` }); }
          cached.delete(p);
        }
        await sleep(300);
        return { moved: paths.length, failed: [], undo };
      }
      // 撤销移到废纸篓：放回原来的位置（模拟的照片表里加回去）
      case "untrash_photos": {
        const items = args.items as { path: string }[];
        const restored: string[] = [], failed: string[] = [];
        for (const it of items) { const s = trashed.get(it.path); if (s) { SRC.push(s); trashed.delete(it.path); restored.push(it.path); } else failed.push(it.path); }
        await sleep(300);
        return { restored, failed };
      }
      case "reveal": return null;
      case "large_preview": return (args.path as string);
      // 转一张：角度记下、宽高按转过的对调（模拟的缩略图本身不转）
      case "set_rotation": {
        const p = lastPhotos.find((x) => x.path === args.path);
        if (!p) return null;
        const deg = Number(args.deg), odd = Math.abs(deg - (p.rotation ?? 0)) % 180 === 90;
        const q = { ...p, rotation: deg, width: odd ? p.height : p.width, height: odd ? p.width : p.height };
        lastPhotos = lastPhotos.map((x) => (x.path === q.path ? q : x));
        return q;
      }
      case "backup_peek": return null;
      case "photo_info": return { format: "JPEG", bytes: 9_400_000, color_space: "sRGB", bit_depth: "8, 8, 8", camera: "FUJIFILM X-T5", lens: "XF35mmF1.4 R", focal: "35 mm", focal_35: "52 mm", aperture: "f/2", shutter: "1/250 s", iso: "400", software: null, taken: "2026-04-17 18:22",
        exposure_bias: "+0.3 EV", white_balance: "Auto", flash: "Off", metering: "Multi", exposure_program: "A", files: [{ path: String(args.path), bytes: 9_400_000 }, { path: String(args.path).replace(/\.\w+$/, ".RAF"), bytes: 31_200_000 }] };
      case "machine_info": return { memory_gb: 32, cores: 10 };
      case "packs_status": return (["standard", "high", "ultra", "search", "eyes", "faces", "scenetext"] as const).map((id) => ({ id, installed: packState[id], bytes: 0 }));
      case "pack_size": return { standard: 266_000_000, high: 609_000_000, ultra: 870_000_000, search: 139_000_000, eyes: 7_000_000, faces: 39_000_000, scenetext: 400_000_000 }[String(args.id) as "standard"];
      case "pack_download": {
        // 约 8 秒下完，可以取消；`window.__mockDlErr = "space:300000000:120000000"`（或 verify: / net:）让它在半路按约定的错误码失败
        dlCancel = false;
        for (let i = 1; i <= 40; i++) {
          await sleep(200);
          if (dlCancel) throw "cancelled";
          const err = (window as unknown as { __mockDlErr?: string }).__mockDlErr;
          if (err && i === (err.startsWith("space:") ? 1 : 20)) throw err;
          emit("model-progress", { pack: args.id, file: "x", done: i * 15_225_000, total: 609_000_000 });
        }
        packState[String(args.id) as "standard"] = true;
        return null;
      }
      case "pack_cancel": dlCancel = true; return null;
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
      // 网络代理（proxy.rs）：系统的写死一个，手填的记在 localStorage；认不出来的照 proxy.rs 的错误码
      case "proxy_info": return { system: "http://127.0.0.1:7890", manual: localStorage.getItem("fg.mock.proxy"), active: localStorage.getItem("fg.mock.proxy") ?? "http://127.0.0.1:7890", exceptions: ["*.local", "169.254/16"], exclude_simple: true };
      case "proxy_set": {
        const raw = String(args.manual ?? "").trim();
        if (!raw) localStorage.removeItem("fg.mock.proxy");
        else {
          const m = raw.match(/^(?:(\w+):\/\/)?([^/:\s]+):(\d+)\/?$/);
          if (m?.[1] && !/^https?$/i.test(m[1])) throw `unsupported:${m[1]}`;
          if (!m) throw raw.includes(":") ? "host" : "port";
          localStorage.setItem("fg.mock.proxy", `http://${m[2]}:${m[3]}`);
        }
        return { system: "http://127.0.0.1:7890", manual: localStorage.getItem("fg.mock.proxy"), active: localStorage.getItem("fg.mock.proxy") ?? "http://127.0.0.1:7890" };
      }
      case "samples_folder": return "/mock/Samples";
      // 拖进窗口（lib.rs 的 dropped_folders）：模拟的文件夹本身，或模拟照片所在的文件夹
      case "dropped_folders": return [...new Set((args.paths as string[]).map((p) => (p in FOLDERS ? p : p.slice(0, p.lastIndexOf("/")))).filter((d) => d in FOLDERS))];
      case "plugin:dialog|save": return "/Users/me/Desktop/fleetinggleam-diagnostics.txt";
      case "diagnostics_export": await sleep(500); return String(args.out);
      case "diagnostics_dir": return "/Users/me/Library/Application Support/com.fleetinggleam.app/logs";
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
