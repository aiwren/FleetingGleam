import Modal, { isModalOpen } from "../components/Modal";
import Flights, { FlightsHandle } from "../components/Flights";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import InfoPop from "../components/InfoPop";
import Wall, { WallHandle } from "../components/Wall";
import Single from "./Single";
import ViewMenu from "../components/ViewMenu";
import Resizer, { usePanelWidth } from "../components/Resizer";
import SceneGallery from "../components/SceneGallery";
import FilterPanel, { PersonItem } from "../components/FilterPanel";
import SearchBox from "../components/SearchBox";
import { usePrefsRevision } from "../styles";
import { ensurePack, ensurePacks, refreshPacks, usePacks } from "../models";
import { analysisPacks, currentTier, estimateSeconds, eyeMethodNow, getMachine, predictTier, resolveTier } from "../strategy";
import { Hit, searchPhotos, setPendingSelect, takePendingSelect, useSearchIndex } from "../search";
import PhotoVerdict from "../components/PhotoVerdict";
import { FileSection } from "../components/FileFacts";
import Accordion from "../components/Accordion";
import { forgetLarge, prefetchLarge } from "../largeSrc";
import Compare from "./Compare";
import Ticker from "../components/Ticker";
import Tile, { StageInfo } from "../components/Tile";
import { DUR, EASE_OUT, SPRING } from "../motion";
import { newProjectFromDialog, pickFolder } from "../projectActions";
import { Project, setActive, trashProject, updateProject, useProjects, useProjectsStatus } from "../projects";
import { getSettings, ScanInfo, setScanInfo, useSettings, useCacheRev } from "../settings";
import { Decision, Photo, Probe, ProbeFile, Reason, State, pct, thumbSrc } from "../types";
import { NasProgress } from "../nas";
import Organize from "./Organize";
import ReadyStage from "./ReadyStage";
import ProjectSettings from "./ProjectSettings";
import ProjectHome from "./ProjectHome";
import Viewer from "./Viewer";

type ViewMode = "all" | State;
type Phase = "idle" | "probing" | "ready" | "scanning" | "loading" | "done";
type SortBy = "group" | "order" | "time" | "score";
type Item = { path: string; ratio: number };

function EditableTitle({ value, tip, onCommit }: { value: string; tip: string; onCommit: (v: string) => void }) {
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  if (editing) {
    const done = () => { setEditing(false); const v = text.trim(); if (v && v !== value) onCommit(v); else setText(value); };
    return <input className="title-input" autoFocus value={text} onChange={(e) => setText(e.target.value)} onBlur={done}
      onKeyDown={(e) => { if (e.key === "Enter") done(); if (e.key === "Escape") { setText(value); setEditing(false); } }} />;
  }
  return <b className="title-edit" title={tip} onDoubleClick={() => setEditing(true)}>{value}</b>;
}

type ViewStyle = "justified" | "grid" | "single" | "compare";
const LAYOUTS: { id: ViewStyle; icon: ReactNode }[] = [
  { id: "justified", icon: <><rect x="1.5" y="2" width="6" height="5" rx="1" /><rect x="9" y="2" width="5.5" height="5" rx="1" /><rect x="1.5" y="9" width="4" height="5" rx="1" /><rect x="7" y="9" width="7.5" height="5" rx="1" /></> },
  { id: "grid", icon: <><rect x="1.5" y="1.5" width="5" height="5" rx="1" /><rect x="9.5" y="1.5" width="5" height="5" rx="1" /><rect x="1.5" y="9.5" width="5" height="5" rx="1" /><rect x="9.5" y="9.5" width="5" height="5" rx="1" /></> },
  { id: "single", icon: <><rect x="1.5" y="2" width="13" height="8" rx="1.5" /><path d="M4 13.5h1.5M7.3 13.5h1.5M10.5 13.5H12" /></> },
  { id: "compare", icon: <><rect x="1.5" y="2.5" width="5.5" height="11" rx="1.2" /><rect x="9" y="2.5" width="5.5" height="11" rx="1.2" /></> },
];

export default function Culling({ active, inspectorHidden, onToggleInspector, onOpenStyles }: {
  active: boolean;
  inspectorHidden: boolean;
  onToggleInspector: () => void;
  /** Called when the user selects a photo, so the shell can slide the inspector open. */
  onOpenStyles: () => void;
}) {
  const { t, i18n } = useTranslation();
  const store = useProjects();
  const { saveError, loadError } = useProjectsStatus();
  const project: Project | null = store.projects.find((p) => p.id === store.activeId && !p.trashed) ?? null;
  const settings = useSettings();

  const [phase, setPhase] = useState<Phase>("idle");
  const [files, setFiles] = useState<ProbeFile[]>([]);
  const [bytes, setBytes] = useState(0);
  const [uncached, setUncached] = useState(0);
  const [photos, setPhotos] = useState<Photo[]>([]);
  const [progress, setProgress] = useState({ done: 0, total: 0 });
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<ViewMode>("all");
  const [sceneFilter, setSceneFilter] = useState<number | null>(null);
  const [personFilter, setPersonFilter] = useState<number | null>(null);
  const filterW = usePanelWidth("fg.w.filter", 236, 190, 460);
  const inspectorW = usePanelWidth("fg.w.inspector", 300, 240, 560);
  const [resizing, setResizing] = useState(false);
  /** How much width the wall is about to gain (positive) or lose (negative) as a side panel closes or opens, for the length of that animation. */
  const [wallDelta, setWallDelta] = useState<number | null>(null);
  const [sceneView, setSceneView] = useState(false);
  /** One sentence per scene ("a large white building with a steeple"), written on this machine once the project is done. */
  const [sceneText, setSceneText] = useState<Map<number, string>>(new Map());
  const [describing, setDescribing] = useState<{ done: number; total: number } | null>(null);
  const scenePack = usePacks().find((p) => p.id === "scenetext")?.installed ?? false;
  // Looking at faces of photos analysed before people recognition existed takes a while; say so instead of looking stuck.
  const [peopleProg, setPeopleProg] = useState<{ done: number; total: number } | null>(null);
  // The same for HDR and motion: photos analysed earlier get their HDR thumbnails made on first opening.
  const [mediaProg, setMediaProg] = useState<{ done: number; total: number } | null>(null);
  // 项目在 NAS 上时，探测和分析前都先把新照片拉进本地镜像；第一次打开可能要等一会儿，所以把进度说出来
  const [nasProg, setNasProg] = useState<NasProgress | null>(null);
  useEffect(() => {
    const a = listen<{ done: number; total: number }>("people-progress", (e) => setPeopleProg(e.payload));
    const b = listen<{ done: number; total: number }>("media-progress", (e) => setMediaProg(e.payload));
    const c = listen<NasProgress>("nas-progress", (e) => setNasProg(e.payload.done >= e.payload.total ? null : e.payload));
    return () => { void a.then((f) => f()); void b.then((f) => f()); void c.then((f) => f()); };
  }, []);
  const [panelOpen, setPanelOpen] = useState(() => { try { return localStorage.getItem("fg.filterpanel") !== "0"; } catch { return true; } });
  const togglePanel = () => setPanelOpen((v) => { const n = !v; try { localStorage.setItem("fg.filterpanel", n ? "1" : "0"); } catch { /* storage unavailable */ } return n; });
  const [selected, setSelected] = useState<string | null>(null);
  const [zoomed, setZoomed] = useState(false);
  const [celebrate, setCelebrate] = useState(false);
  const [dirty, setDirty] = useState(0);
  // a taste was changed elsewhere (a folder of picks, another style): offer to re-evaluate
  const prefsRev = usePrefsRevision();
  const seenRev = useRef(prefsRev);
  useEffect(() => { if (seenRev.current !== prefsRev) { seenRev.current = prefsRev; setDirty((d) => d + 1); } }, [prefsRev]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const wallRef = useRef<WallHandle>(null);
  const [query, setQuery] = useState("");
  const [rawHits, setHits] = useState<Hit[] | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [layout, setLayout] = useState<ViewStyle>(() => {
    try { const v = localStorage.getItem("fg.layout"); if (v === "justified" || v === "grid" || v === "single" || v === "compare") return v; } catch { /* storage unavailable */ }
    return "justified";
  });
  const pickLayout = (v: ViewStyle) => { setLayout(v); try { localStorage.setItem("fg.layout", v); } catch { /* storage unavailable */ } };
  const [from, setFrom] = useState<{ x: number; y: number; width: number; height: number } | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const cacheRev = useCacheRev();
  const [nasOff, setNasOff] = useState("");
  const [etaSeconds, setEtaSeconds] = useState<number | null>(null);
  const [searchRev, setSearchRev] = useState(0);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const undoStack = useRef<{ path: string; prev: State | undefined }[]>([]);
  const [hintUses, setHintUses] = useState(() => { try { return Number(localStorage.getItem("fg.hint.dock") ?? 0); } catch { return 0; } });
  const bumpHint = (n: number) => { setHintUses((u) => { const v = Math.max(u, n === 99 ? 99 : u + 1); try { localStorage.setItem("fg.hint.dock", String(v)); } catch { /* ignore */ } return v; }); };

  const keepRatio = project?.keepCap ?? 1;
  const onePerGroup = project?.onePerGroup ?? false;
  const pickMode = project?.pickMode ?? "diverse";
  const sortBy: SortBy = project?.sort ?? "group";
  const setSortBy = (v: SortBy) => { if (project) updateProject(project.id, { sort: v }); };
  const overrides = useMemo(() => new Map(Object.entries(project?.overrides ?? {})) as Map<string, State>, [project?.overrides]);

  // Live scan data lives in refs and is flushed once per frame, so thousands of events stay cheap.
  const results = useRef(new Map<string, Photo>());
  const stages = useRef(new Map<string, StageInfo>());
  /** 边分析边判：引擎隔一会儿发来的判定（分析完换成正式的 decisions）。 */
  const live = useRef(new Map<string, Decision>());
  const lastStage = useRef<{ path: string; stage: string } | null>(null);
  const startedAt = useRef(0);
  const raf = useRef(0);
  const token = useRef(0);
  const [tick, setTick] = useState(0);
  /** 正在分析的项目（同一时间只有一个，作者 2026-10-23：PC 也要后台分析）：离开它照跑，回来接上；首页卡片显示它的进度。 */
  const running = useRef<{ id: string; name: string; folders: string[]; files: ProbeFile[] } | null>(null);
  const [runningId, setRunningId] = useState<string | null>(null);
  /** 分析中这一屏正显示着它（离开项目时为假：事件照收，界面不动）。 */
  const attached = useRef(false);
  const activeId = useRef<string | null>(null);
  activeId.current = project?.id ?? null;
  const filesRef = useRef<ProbeFile[]>([]);
  filesRef.current = files;
  /** 上一次进的项目（离开时只取消它自己的读取 / 分析）。 */
  const enteredId = useRef<string | null>(null);
  /** 在后台分析完的项目：首页给一句"分析完成 · 打开"。 */
  const [bgDone, setBgDone] = useState<{ id: string; name: string } | null>(null);
  /** 要分析这个项目，可另一个还在分析：问一句。 */
  const [conflict, setConflict] = useState<{ id: string; name: string } | null>(null);
  const pendingStart = useRef<(() => void) | null>(null);
  /** 分析中照片判出来时飘向页签的光点。 */
  const flights = useRef<FlightsHandle>(null);
  const flush = useCallback(() => {
    if (!raf.current) raf.current = requestAnimationFrame(() => { raf.current = 0; setTick((n) => n + 1); });
  }, []);

  /**
   * `quiet` = 只是从缓存打开（没有进度事件）；否则是一次真正的分析：它属于 `proj`，离开那个项目也照跑，
   * 跑完时如果页面还开着它就把结果交给页面，不然只更新项目封面、首页提一句"分析完成"。
   */
  const runScan = useCallback(async (proj: Project, total: number, quiet: boolean, tok: number, force = false) => {
    if (quiet) setPhase("loading");
    else {
      results.current.clear(); stages.current.clear(); live.current.clear(); lastStage.current = null;
      startedAt.current = performance.now();
      setProgress({ done: 0, total });
      running.current = { id: proj.id, name: proj.name, folders: proj.folders, files: filesRef.current };
      setRunningId(proj.id);
      attached.current = true;
      setPhase("scanning");
    }
    // 事件只认正在跑的这次分析（离开项目也照收，回来时照片墙是全的）
    const own = () => running.current?.id === proj.id;
    // 结果交不交给页面：安静的读取看 token；分析看页面是不是还开着它
    const mine = () => (quiet ? tok === token.current : attached.current && activeId.current === proj.id);
    const offs = quiet ? [] : await Promise.all([
      listen<{ done: number; total: number }>("scan-progress", (e) => { if (own()) setProgress(e.payload); }),
      listen<ScanInfo>("scan-info", (e) => setScanInfo(e.payload)),
      listen<{ path: string; stage: string; pct: number; thumb?: string | null; width?: number | null; height?: number | null }>("scan-stage", (e) => {
        if (!own()) return;
        const p = e.payload, old = stages.current.get(p.path);
        stages.current.set(p.path, { stage: p.stage, pct: p.pct, thumb: p.thumb ?? old?.thumb, width: p.width ?? old?.width, height: p.height ?? old?.height });
        lastStage.current = { path: p.path, stage: p.stage };
        flush();
      }),
      listen<Photo>("scan-photo", (e) => {
        if (!own()) return;
        results.current.set(e.payload.path, e.payload);
        stages.current.delete(e.payload.path);
        flush();
      }),
      // 边分析边判：只有判定变了的那些会发来；页面开着时让它们飞进页签
      listen<{ path: string; decision: Decision }[]>("scan-decide", (e) => {
        if (!own()) return;
        for (const j of e.payload) live.current.set(j.path, j.decision);
        flush();
        if (attached.current) requestAnimationFrame(() => flights.current?.judged(e.payload));
      }),
      // the photos that were already analysed arrive in batches
      listen<Photo[]>("scan-photos", (e) => {
        if (!own()) return;
        for (const p of e.payload) { results.current.set(p.path, p); stages.current.delete(p.path); }
        flush();
      }),
    ]);
    try {
      // 边分析边判用和分析完一样的参数
      const liveArgs = quiet ? null : { keep_ratio: proj.keepCap ?? 1, pick_mode: proj.pickMode ?? "diverse", one_per_group: proj.onePerGroup ?? false, overrides: proj.overrides ?? {} };
      const res = await invoke<Photo[]>("scan_project", { key: proj.id, dirs: proj.folders, compute: "auto", quality: currentTier(), style: proj.style, learn: getSettings().learn && proj.learn !== false, quiet, force, eyeMethod: eyeMethodNow(), live: liveArgs });
      if (!quiet) { running.current = null; setRunningId(null); }
      const cover = [...res].sort((a, b) => b.score - a.score).slice(0, 4).map((p) => p.thumb);
      if (!mine()) {
        // 在后台分析完：封面与张数更新，首页提一句
        if (!quiet) { updateProject(proj.id, { cover, count: res.length }); setBgDone({ id: proj.id, name: proj.name }); }
        return;
      }
      setPhotos(res);
      setPhase("done");
      setDirty(0);
      updateProject(proj.id, { cover });
      if (!quiet) { setCelebrate(true); setTimeout(() => setCelebrate(false), 3200); }
    } catch (e) {
      if (!quiet) { running.current = null; setRunningId(null); }
      // a cancelled scan is not an error: the project simply goes back to "ready to analyse"
      if (mine()) { if (String(e) !== "cancelled") setError(String(e)); setPhase("ready"); }
    } finally {
      offs.forEach((f) => f());
      // 问过"停下它、分析这个"的：那个停下了，这个开始
      const next = pendingStart.current;
      pendingStart.current = null;
      next?.();
    }
  }, [flush]);

  // Entering a project (or changing its folders): read sizes first, then either wait for the user's go-ahead
  // (new photos to analyse) or open instantly from the cache.
  const folderKey = project ? `${project.id}|${project.folders.join("|")}` : "";
  useEffect(() => {
    const tok = ++token.current;
    const run = running.current;
    const left = enteredId.current;
    enteredId.current = project?.id ?? null;
    undoStack.current = []; // the marks that could be undone belonged to the project just left
    setSelected(null); setQuery(""); setZoomed(false); setError(null); setMode("all"); setSceneFilter(null); setPersonFilter(null); setSceneView(false); setCelebrate(false); setDirty(0); setNasProg(null);
    setBgDone((d) => (d && project && d.id === project.id ? null : d));
    // 回到正在分析的那个项目（文件夹没变）：接上，照片墙和进度都还在，不重来
    if (project && run && run.id === project.id && run.folders.join("|") === project.folders.join("|")) {
      attached.current = true;
      setFiles(run.files);
      setPhase("scanning");
      flush();
      return;
    }
    // 离开的那个项目：只停它自己的读取 / 分析；正在分析的那个转到后台接着跑
    attached.current = false;
    if (left && left !== run?.id) invoke("scan_cancel", { key: left }).catch(() => { /* not in Tauri */ });
    if (!project || project.folders.length === 0) { setPhase("idle"); setPhotos([]); setFiles([]); return; }
    setPhase("probing");
    (async () => {
      try {
        // a project on a network share: connect the share first, and say so when it cannot be reached
        const shares = await invoke<{ name: string; enabled: boolean; connected: boolean }[]>("nas_ensure", { dirs: project.folders }).catch(() => []);
        if (tok !== token.current) return;
        const disabled = shares.filter((s) => !s.enabled);
        if (disabled.length > 0) { setPhotos([]); setFiles([]); setPhase("idle"); setNasOff(disabled.map((s) => s.name).join(", ")); setError("cull.nasDisabled"); return; }
        const off = shares.filter((s) => !s.connected);
        if (off.length > 0) { setPhotos([]); setFiles([]); setPhase("idle"); setNasOff(off.map((s) => s.name).join(", ")); setError("cull.nasOffline"); return; }
        const probe = await invoke<Probe>("probe_folder", { dirs: project.folders, compute: "auto", quality: currentTier() });
        if (tok !== token.current) return;
        setFiles(probe.files); setBytes(probe.bytes); setUncached(probe.uncached);
        if (probe.files.length === 0) { setPhotos([]); setPhase("idle"); setError("cull.emptyFolder"); return; }
        if (project.count !== probe.files.length) updateProject(project.id, { count: probe.files.length });
        if (probe.uncached > 0) setPhase("ready");
        else await runScan(project, probe.files.length, true, tok);
      } catch (e) {
        if (tok === token.current) { setError(String(e)); setPhase("idle"); }
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folderKey, reloadKey, settings.tierRev, cacheRev]);

  // how long the analysis will take, said before it starts (the model the app will use, the cores this machine has)
  useEffect(() => {
    if (phase !== "ready") return;
    let live = true;
    Promise.all([predictTier(), getMachine()]).then(([tier, m]) => { if (live) setEtaSeconds(estimateSeconds(tier, uncached, m.cores)); }).catch(() => { /* not in the app */ });
    return () => { live = false; };
  }, [phase, uncached]);

  // Quick 640px previews for the photos shown on the confirmation screen, so the UI never decodes originals.
  useEffect(() => {
    if (phase !== "ready") return;
    const paths = files.filter((f) => !f.cached).slice(0, 8).map((f) => f.path);
    if (!paths.length) return;
    invoke<(string | null)[]>("preview_thumbs", { paths })
      .then((list) => setPreviews((prev) => { const next = { ...prev }; list.forEach((th, i) => { if (th) next[paths[i]] = th; }); return next; }))
      .catch(() => { /* fall back to the originals */ });
  }, [phase, files]);

  const startScan = async () => {
    if (!project) return;
    // 同一时间只分析一个项目：另一个还在分析时先问一句
    const run = running.current;
    if (run && run.id !== project.id) { setConflict({ id: run.id, name: run.name }); return; }
    // The app picks the model (strategy.ts); what it needs, the eye model and people recognition included, is fetched under one question the first time.
    const tier = await resolveTier();
    if ((await ensurePacks(analysisPacks(tier))) === "cancelled") return;
    runScan(project, files.length, false, token.current);
  };
  /** 停下正在跑的那次分析（不管页面开着哪个项目）。 */
  const stopScan = () => { invoke("scan_cancel", { key: running.current?.id }).catch(() => { /* not in Tauri */ }); };
  /** 问过了：先停下另一个项目的分析（分析过的都在缓存里，回到它时接着来），再分析这个。 */
  const stopOtherAndStart = () => {
    setConflict(null);
    pendingStart.current = () => { void startScan(); };
    stopScan();
  };
  const reevaluate = () => { if (project) runScan(project, files.length, true, token.current); };
  const forceRescan = () => { if (project) runScan(project, files.length, false, token.current, true); };

  const addFolder = async () => {
    const dir = await pickFolder();
    if (dir && project && !project.folders.includes(dir)) updateProject(project.id, { folders: [...project.folders, dir] });
  };
  const removeFolder = (dir: string) => {
    if (project) updateProject(project.id, { folders: project.folders.filter((f) => f !== dir) });
  };
  // Keep/review/reject is decided on the backend (see src-tauri/src/decide.rs): only the keep ratio and any manual
  // overrides cross the IPC boundary, the photos themselves stay cached there from the last scan.
  const [decisions, setDecisions] = useState<Map<string, Decision>>(new Map());
  const decideToken = useRef(0);
  // A photo marked by hand changes only that photo (what is kept otherwise does not depend on anyone's override unless a ceiling or "one per
  // group" is on), so it is patched here, at once, and the whole decision table is not asked for again and sent over for every click.
  const localPatch = useRef<{ path: string; state: State; photos: Photo[] } | null>(null);
  useEffect(() => {
    if (photos.length === 0) { setDecisions(new Map()); return; }
    const lp = localPatch.current;
    localPatch.current = null;
    if (lp && lp.photos === photos && overrides.get(lp.path) === lp.state) {
      decideToken.current++;   // an answer still on its way is out of date now
      setDecisions((m) => {
        const cur = m.get(lp.path);
        if (!cur) return m;
        const n = new Map(m);
        n.set(lp.path, { state: lp.state, manual: true, reasons: [{ k: "reason.manual", p: { stateKey: `state.${lp.state}` } }, ...(cur.manual ? cur.reasons.slice(1) : cur.reasons)] });
        return n;
      });
      return;
    }
    const tok = ++decideToken.current;
    invoke<Record<string, Decision>>("decide_photos", { key: project?.id ?? "", keepRatio, pickMode, onePerGroup, overrides: Object.fromEntries(overrides) })
      .then((res) => { if (tok === decideToken.current) setDecisions(new Map(Object.entries(res))); })
      // "stale": the backend holds another project's photos (a scan of this one is on its way); the next scan result asks again
      .catch((e) => { if (tok === decideToken.current) { if (String(e) !== "stale") console.error("decide_photos:", e); setDecisions(new Map()); } });
  }, [photos, keepRatio, pickMode, onePerGroup, overrides, project?.id]);
  const counts = useMemo(() => {
    const c = { all: photos.length, keep: 0, review: 0, reject: 0 };
    decisions.forEach((d) => { c[d.state]++; });
    return c;
  }, [decisions, photos.length]);
  // 分析中横幅上的保留 / 待复查 / 淘汰数（跟着每帧的 tick 走）
  const liveCounts = useMemo(() => {
    if (phase !== "scanning" || live.current.size === 0) return null;
    const c = { keep: 0, review: 0, reject: 0 };
    live.current.forEach((d) => { c[d.state]++; });
    return c;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, tick]);

  // Scenes are the same loose, look-alike clusters the backend already groups for scene-diverse picks (see decide.rs);
  // this just surfaces them as a filter, in the order they first appear, each with its highest-scoring photo as the cover.
  const scenes = useMemo(() => {
    const by = new Map<number, { scene: number; cover: Photo; total: number }>();
    for (const p of photos) {
      const s = by.get(p.scene);
      if (!s) by.set(p.scene, { scene: p.scene, cover: p, total: 1 });
      else { s.total++; if (p.score > s.cover.score) s.cover = p; }
    }
    // Biggest first; a stable sort keeps the order of first appearance among equals.
    return [...by.values()].sort((x, y) => y.total - x.total);
  }, [photos]);

  // People are the recurring faces the backend recognised (see faces.rs): each one's avatar is their biggest face.
  const people = useMemo(() => {
    const by = new Map<number, PersonItem & { area: number; paths: Set<string> }>();
    for (const p of photos) {
      p.people.forEach((id, fi) => {
        if (id < 0 || !p.face_boxes[fi]) return;
        const box = p.face_boxes[fi];
        const area = box[2] * box[3];
        const e = by.get(id);
        if (!e) by.set(id, { person: id, photo: p, box, total: 1, count: 1, area, paths: new Set([p.path]) });
        else {
          if (!e.paths.has(p.path)) { e.paths.add(p.path); e.total++; e.count++; }
          if (area > e.area) { e.area = area; e.photo = p; e.box = box; }
        }
      });
    }
    // Most photographed first: those are the people you came for.
    return [...by.values()].sort((a, b) => b.total - a.total || a.person - b.person);
  }, [photos]);

  // Every number in the filter panel is what the wall would show if that entry were picked now, given the other two filters:
  // picking a person makes each scene count only that person's photos in it, and so on. An entry that would show nothing is greyed out.
  const facets = useMemo(() => {
    const st = (p: Photo) => decisions.get(p.path)?.state;
    const inScene = (p: Photo) => sceneFilter === null || p.scene === sceneFilter;
    const inPerson = (p: Photo) => personFilter === null || p.people.includes(personFilter);
    const inMode = (p: Photo) => mode === "all" || st(p) === mode;
    const status = { all: 0, keep: 0, review: 0, reject: 0 };
    const sceneN = new Map<number, number>();
    const personN = new Map<number, number>();
    for (const p of photos) {
      if (inScene(p) && inPerson(p)) { status.all++; const s = st(p); if (s) status[s]++; }
      if (inMode(p) && inPerson(p)) sceneN.set(p.scene, (sceneN.get(p.scene) ?? 0) + 1);
      if (inMode(p) && inScene(p)) new Set(p.people.filter((id) => id >= 0)).forEach((id) => personN.set(id, (personN.get(id) ?? 0) + 1));
    }
    return { status, sceneN, personN };
  }, [photos, decisions, mode, sceneFilter, personFilter]);
  // Ask the backend to name each scene (biggest first) once the photos are final: from a large vocabulary of places straight away, and for
  // the scenes it cannot name, in a sentence from the captioning model when that is installed. The names arrive one at a time. Scene numbers change whenever photos are regrouped, so it starts over then; sentences already written are remembered.
  const describeScenes = useCallback((caption: boolean) => {
    const by = new Map<number, Photo[]>();
    for (const p of photos) { const l = by.get(p.scene); if (l) l.push(p); else by.set(p.scene, [p]); }
    const reqs = [...by.entries()].sort((a, b) => b[1].length - a[1].length)
      .map(([id, ps]) => ({ id, paths: [...ps].sort((a, b) => b.score - a.score).map((p) => p.path).slice(0, 6) }));
    setDescribing({ done: 0, total: reqs.length });
    invoke("scene_text", { scenes: reqs, zh: i18n.language.startsWith("zh"), caption }).catch(() => setDescribing(null));
  }, [photos, i18n.language]);
  useEffect(() => {
    let un: (() => void)[] = [];
    let live = true;
    // The names arrive in a burst (every scene the vocabulary can name at once): they are gathered and applied together, a few times a second,
    // instead of redrawing the whole page for each one.
    let buf = new Map<number, string>();
    let timer = 0;
    const flushNames = () => {
      timer = 0;
      const b = buf;
      buf = new Map();
      setSceneText((m) => { const n = new Map(m); b.forEach((v, k) => n.set(k, v)); return n; });
    };
    Promise.all([
      listen<{ id: number; text: string }>("scene-text", (e) => { buf.set(e.payload.id, e.payload.text); if (!timer) timer = window.setTimeout(flushNames, 250); }),
      listen<{ id: number; text: string }[]>("scene-texts", (e) => { e.payload.forEach((x) => buf.set(x.id, x.text)); if (!timer) timer = window.setTimeout(flushNames, 50); }),
      listen<{ done: number; total: number }>("scene-text-progress", (e) => setDescribing(e.payload.done >= e.payload.total ? null : e.payload)),
    ]).then((f) => { if (live) un = f; else f.forEach((x) => x()); }).catch(() => { /* not in Tauri */ });
    return () => { live = false; window.clearTimeout(timer); un.forEach((x) => x()); };
  }, []);
  useEffect(() => { void refreshPacks(); }, []);
  useEffect(() => {
    setSceneText(new Map());
    setDescribing(null);
    if (phase === "done" && photos.length > 0) describeScenes(scenePack);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, photos, scenePack, i18n.language]);
  const startDescribing = async () => { if ((await ensurePack("scenetext")) !== "cancelled") describeScenes(true); };

  // The scene gallery: each scene with the photos that match the status and person filters, best shot first.
  const sceneGroups = useMemo(() => {
    if (!sceneView) return [];   // only the scene page needs these, and building them for every photo is not free
    const by = new Map<number, Photo[]>();
    for (const p of photos) {
      if ((mode !== "all" && decisions.get(p.path)?.state !== mode) || (personFilter !== null && !p.people.includes(personFilter))) continue;
      const l = by.get(p.scene);
      if (l) l.push(p); else by.set(p.scene, [p]);
    }
    return [...by.entries()].map(([scene, ps]) => ({ scene, photos: ps.sort((a, b) => b.score - a.score), text: sceneText.get(scene) ?? "" })).sort((a, b) => b.photos.length - a.photos.length);
  }, [photos, decisions, mode, personFilter, sceneText, sceneView]);
  // Compare works inside the filters: with a person picked only their photos are put side by side, and with a scene picked the scene as a whole is compared, not just one burst.
  const compareAll = useMemo(() => (personFilter === null ? photos : photos.filter((p) => p.people.includes(personFilter))), [photos, personFilter]);
  const sceneItems = useMemo(() => scenes.map((s) => ({ ...s, count: facets.sceneN.get(s.scene) ?? 0, text: sceneText.get(s.scene) ?? "" })), [scenes, facets, sceneText]);
  const peopleItems = useMemo(() => people.map((p) => ({ person: p.person, photo: p.photo, box: p.box, total: p.total, count: facets.personN.get(p.person) ?? 0 })), [people, facets]);

  // The selected photo stays visible until you move on, even if the change you just made means it no longer matches this view.
  const matchesView = useCallback((p: Photo) => (mode === "all" || decisions.get(p.path)?.state === mode) && (sceneFilter === null || p.scene === sceneFilter) && (personFilter === null || p.people.includes(personFilter)), [mode, decisions, sceneFilter, personFilter]);
  const byPath = useMemo(() => new Map(photos.map((p) => [p.path, p])), [photos]);
  // Only when the selected photo really no longer matches is it carried along. In every other case this is null, so selecting a photo leaves the
  // list of photos in view (and with it every tile's position) exactly as it was.
  const carried = useMemo(() => {
    const p = selected ? byPath.get(selected) : undefined;
    return p && !matchesView(p) ? p.path : null;
  }, [selected, byPath, matchesView]);
  const matches = useCallback((p: Photo) => matchesView(p) || p.path === carried, [matchesView, carried]);
  // One stable order for the whole project. Switching the filter only hides tiles; it never rebuilds them.
  const ordered: Photo[] = useMemo(() => {
    if (phase !== "done") return [];
    let list = photos;
    if (sortBy === "score") list = [...list].sort((a, b) => b.score - a.score);
    else if (sortBy === "time") list = [...list].sort((a, b) => (a.taken_at ?? 0) - (b.taken_at ?? 0));
    else if (sortBy === "group") {
      // Similar shots side by side: groups keep their place in the folder, the best of each group comes first.
      const first = new Map<number, number>();
      list.forEach((p, i) => { if (!first.has(p.group)) first.set(p.group, i); });
      list = [...list].sort((a, b) => (first.get(a.group)! - first.get(b.group)!) || b.score - a.score);
    }
    return list;
  }, [phase, photos, sortBy]);
  // Scene sentences are searchable too: "花田" or "building" finds every photo of a scene described that way, ahead of what the text model ranks.
  const hits = useMemo<Hit[] | null>(() => {
    const q = query.trim().toLowerCase();
    if (!project || !q || !rawHits || sceneText.size === 0 || (q.length < 3 && !/[^\x00-\x7f]/.test(q))) return rawHits;
    const tagged = photos.filter((p) => sceneText.get(p.scene)?.toLowerCase().includes(q));
    if (tagged.length === 0) return rawHits;
    const tagSet = new Set(tagged.map((p) => p.path));
    const rank = new Map(rawHits.map((h, i) => [h.path, i]));
    const asHit = (p: Photo): Hit => ({ path: p.path, name: p.name, thumb: p.thumb, width: p.width, height: p.height, score: p.score });
    const first = rawHits.filter((h) => tagSet.has(h.path));
    const extra = tagged.filter((p) => !rank.has(p.path)).sort((a, b) => b.score - a.score).map(asHit);
    return [...first, ...extra, ...rawHits.filter((h) => !tagSet.has(h.path))];
  }, [rawHits, query, project, photos, sceneText]);

  const shownRaw: Photo[] = useMemo(() => {
    const base = ordered.filter(matches);
    if (!hits) return base;
    // While searching, the wall shows the matches, best first, whatever the sort order.
    const rank = new Map(hits.map((h, i) => [h.path, i]));
    return base.filter((p) => rank.has(p.path)).sort((a, b) => rank.get(a.path)! - rank.get(b.path)!);
  }, [ordered, matches, hits]);
  // The same photos in the same order are the same list: marking a photo makes a new decision table, but when it changes nothing about what is
  // in view, nothing downstream (the wall layout, every tile's position) is redone.
  const shownRef = useRef<Photo[]>(shownRaw);
  if (shownRef.current !== shownRaw && (shownRef.current.length !== shownRaw.length || shownRef.current.some((p, k) => p !== shownRaw[k]))) shownRef.current = shownRaw;
  const shown = shownRef.current;

  const groupEnds = useMemo(() => {
    const ends = new Set<string>();
    if (sortBy !== "group") return ends;
    const size = new Map<number, number>();
    shown.forEach((p) => size.set(p.group, (size.get(p.group) ?? 0) + 1));
    shown.forEach((p, i) => { if ((size.get(p.group) ?? 0) > 1 && shown[i + 1]?.group !== p.group) ends.add(p.path); });
    return ends;
  }, [shown, sortBy]);

  // 分析中按眼下的判定筛（"全部"是整个文件夹）。判定每帧都可能变，可筛出来的还是同一批文件时沿用上一份，墙就不重排
  const scanRaw = phase !== "scanning" ? null : mode === "all" ? files : files.filter((f) => live.current.get(f.path)?.state === mode);
  const scanRef = useRef<ProbeFile[]>([]);
  if (scanRaw && scanRef.current !== scanRaw && (scanRef.current.length !== scanRaw.length || scanRef.current.some((f, k) => f !== scanRaw[k]))) scanRef.current = scanRaw;
  const scanList = scanRaw ? scanRef.current : null;
  // 墙的静态部分：哪些格子、宽高比（来自文件头，分析前就有，格子从一开始就是最终大小）、组末间隙，只随文件集与筛选结果变。
  // 动态部分（照片、进度）在渲染格子时从 ref 读，分析中每帧的 tick 只重画变了的格子，不再每帧重跑整面墙的排版（审查第 14 条）
  const items: Item[] = useMemo(() => (scanList
    ? scanList.map((f) => ({ path: f.path, ratio: f.width / f.height }))
    : shown.map((p) => ({ path: p.path, ratio: p.width / p.height }))), [scanList, shown]);
  // 相似组的末尾多留 6px（M5.8：以前只加了类名没有样式）
  const wallItems = useMemo(() => items.map((it) => ({ key: it.path, ratio: it.ratio, gapAfter: groupEnds.has(it.path) })), [items, groupEnds]);
  const openViewer = useCallback((path: string) => {
    const r = document.getElementById(`t-${path}`)?.getBoundingClientRect();
    setFrom(r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null);
    setZoomed(true);
  }, []);

  const selIndex = selected ? shown.findIndex((p) => p.path === selected) : -1;
  const sel = selIndex >= 0 ? shown[selIndex] : null;
  /** The inspector is there when a photo is selected and not otherwise (Compare has no selection). */
  // The inspector is there while a photo is selected (and the person has not hidden it). Nothing selected, nothing to show: no empty panel.
  // When it appears the wall is laid out at its final width at once and glides there (see `wallDelta`), so the photos do not hop between rows.
  const showInspector = phase === "done" && !!sel && !inspectorHidden && !!project && layout !== "compare";
  const panelShown = (phase === "done" || phase === "scanning") && panelOpen;
  const lastPanels = useRef({ inspector: showInspector, filter: panelShown });
  useLayoutEffect(() => {
    const was = lastPanels.current;
    lastPanels.current = { inspector: showInspector, filter: panelShown };
    let d = 0;
    if (was.inspector !== showInspector) d += showInspector ? -inspectorW.width : inspectorW.width;
    if (was.filter !== panelShown) d += panelShown ? -filterW.width : filterW.width;
    if (!d) return;
    setWallDelta(d);
    // The inspector slides; the filter panel does not. The inspector clears this itself when it stops moving (below); this is the fallback.
    const id = window.setTimeout(() => setWallDelta(null), was.inspector !== showInspector ? 2500 : 300);
    return () => window.clearTimeout(id);
  }, [showInspector, panelShown]); // eslint-disable-line react-hooks/exhaustive-deps
  const groupMates = useMemo(() => (sel ? photos.filter((p) => p.group === sel.group) : []), [sel, photos]);

  const select = useCallback((path: string) => setSelected(path), []);
  // What the tiles get: functions whose identity never changes, so a tile is redrawn only when its own photo, mark or position changed.
  const openSceneView = useCallback(() => setSceneView(true), []);
  const tileSelect = useCallback((path: string) => { clickedPath.current = path; setSelected(path); }, []);
  /** 把一张照片顺时针（dir = 1）或逆时针转 90°：核心库换缩略图与显示尺寸（原文件不动），这里只替换手里那张。 */
  const rotate = useCallback((path: string, dir: 1 | -1) => {
    const cur = photos.find((p) => p.path === path);
    if (!cur) return;
    const deg = ((((cur.rotation ?? 0) + 90 * dir) % 360) + 360) % 360;
    invoke<Photo | null>("set_rotation", { path, deg })
      .then((p) => {
        if (!p) return;
        forgetLarge(path);
        setPhotos((ps) => ps.map((x) => (x.path === p.path ? { ...x, thumb: p.thumb, width: p.width, height: p.height, rotation: p.rotation } : x)));
      })
      .catch(() => { /* not analysed yet, or not in Tauri */ });
  }, [photos]);
  const decide = useCallback((path: string, st: State | null, fromUndo = false) => {
    // only photos of the project in view: a stale key press or undo must not write another project's path into this one
    if (!project || !photos.some((p) => p.path === path)) return;
    const prev = project.overrides[path];
    const next = { ...project.overrides };
    if (st === null) delete next[path]; else next[path] = st;
    localPatch.current = st !== null && project.keepCap === undefined && !project.onePerGroup ? { path, state: st, photos } : null;
    updateProject(project.id, { overrides: next });
    // Only explicit keep / reject teach the taste model; "review" and "cleared" mean "not sure". With learning off, nothing is recorded at all.
    if (getSettings().learn && project.learn !== false) {
      invoke("record_label", { path, label: st === null || st === "review" ? null : st === "keep", style: project?.style }).catch(() => { /* not in Tauri */ });
    }
    setDirty((d) => d + 1);
    if (!fromUndo) {
      undoStack.current.push({ path, prev });
      if (undoStack.current.length > 50) undoStack.current.shift();
      bumpHint(1);
    }
    // In a filtered view a decision can move the photo elsewhere. Move on to a neighbour instead of losing the selection.
    if (st && mode !== "all" && st !== mode) {
      const i = shown.findIndex((p) => p.path === path);
      const nb = shown[i + 1] ?? shown[i - 1];
      setSelected(nb && nb.path !== path ? nb.path : null);
    }
  }, [project, mode, shown, photos]);
  const decideRef = useRef(decide);
  decideRef.current = decide;
  const tileDecide = useCallback((path: string, st: State | null) => decideRef.current(path, st), []);
  const undoLast = useCallback(() => {
    const e = undoStack.current.pop();
    if (e) decide(e.path, e.prev ?? null, true);
  }, [decide]);
  const move = useCallback((d: number) => {
    if (!shown.length) return;
    const i = Math.min(shown.length - 1, Math.max(0, (selIndex < 0 ? (d > 0 ? -1 : 0) : selIndex) + d));
    select(shown[i].path);
  }, [shown, selIndex, select]);

  useEffect(() => {
    if (!active || zoomed || phase !== "done") return;
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA" || isModalOpen()) return;
      const k = e.key.toLowerCase();
      if ((e.metaKey || e.ctrlKey) && k === "z") { e.preventDefault(); undoLast(); return; }
      if ((e.metaKey || e.ctrlKey) && (e.key === "]" || e.key === "[")) { if (sel) { e.preventDefault(); rotate(sel.path, e.key === "]" ? 1 : -1); } return; }
      if (layout === "compare" && ["arrowright", "arrowleft", "p", "r", "x"].includes(k)) return; // Compare handles these itself (groups; the photo under the pointer)
      if (k === "arrowright") { e.preventDefault(); move(1); }
      else if (k === "arrowleft") { e.preventDefault(); move(-1); }
      else if (k === " " && sel) { e.preventDefault(); openViewer(sel.path); }
      else if (k === "p" && sel) decide(sel.path, "keep");
      else if (k === "r" && sel) decide(sel.path, "review");
      else if (k === "x" && sel) decide(sel.path, "reject");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active, zoomed, phase, layout, move, sel, decide, rotate, undoLast, openViewer]);

  // Single and Compare always show a photo: fall back to the first one when nothing (or something filtered out) is selected.
  useEffect(() => {
    if (phase === "done" && (layout === "single" || layout === "compare") && shown.length && !shown.some((p) => p.path === selected)) setSelected(shown[0].path);
  }, [phase, layout, shown, selected]);

  // While a full-size view is open, the next few photos are prepared in the background so stepping is instant.
  useEffect(() => {
    if (phase !== "done" || !(zoomed || layout === "single") || selIndex < 0) return;
    const id = setTimeout(() => prefetchLarge([1, -1, 2, -2, 3].map((d) => shown[selIndex + d]?.path).filter((p): p is string => !!p)), 120);
    return () => clearTimeout(id);
  }, [phase, zoomed, layout, selIndex, shown]);

  // Scroll the selected photo into view, unless it was picked by clicking it: it is right under the pointer already, and scrolling the whole
  // wall by a few pixels at that moment reads as the cards jumping.
  const clickedPath = useRef<string | null>(null);
  useEffect(() => {
    const byClick = clickedPath.current === selected;
    clickedPath.current = null;
    if (selected && !byClick) wallRef.current?.reveal(selected);
  }, [selected]);

  useEffect(() => { setQuery(""); }, [project?.id]);
  // ---- search: the index is built in the background, the query runs shortly after typing stops
  // search is on for every project (a project can still have it switched off from before); its models are fetched when someone first goes to search
  const searchOf = (p: Project) => p.search !== false;
  const allFolders = useMemo(() => store.projects.filter((p) => !p.trashed && p.search !== false).flatMap((p) => p.folders), [store.projects]);
  const projectSearch = !!project && searchOf(project);
  const searchDirs = project ? (projectSearch ? project.folders : []) : allFolders;
  const searchKey = searchDirs.join("\n");
  const index = useSearchIndex(project ? (phase === "done" && projectSearch ? project.folders : []) : allFolders, searchRev);
  // the search models are fetched the first time someone goes to search (one question, then the index builds itself in the background)
  const ensureSearch = async () => {
    if (index.available) return;
    if ((await ensurePacks(["search", "standard"])) !== "cancelled") setSearchRev((r) => r + 1);
  };
  useEffect(() => {
    const q = query.trim();
    if (!q) { setHits(null); return; }
    let live = true;
    const id = setTimeout(() => {
      searchPhotos(searchDirs, q).then((h) => { if (live) setHits(h); }).catch(() => { if (live) setHits([]); });
    }, 250);
    return () => { live = false; clearTimeout(id); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, searchKey]);
  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f" && (!project || phase === "done")) { e.preventDefault(); searchRef.current?.focus(); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active, project, phase]);
  // a photo picked in the global search: select it once its project has opened
  useEffect(() => {
    if (phase !== "done") return;
    const want = takePendingSelect();
    if (want && photos.some((p) => p.path === want)) select(want);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, photos]);

  const fmtBytes = (b: number) => {
    const gb = b >= 1e9;
    return new Intl.NumberFormat(i18n.language, { style: "unit", unit: gb ? "gigabyte" : "megabyte", maximumFractionDigits: gb ? 1 : 0 }).format(gb ? b / 1e9 : Math.max(1, b / 1e6));
  };
  // 文案缺参数时回退到不带数字的句子，不会露出 {{diff}}（M8.5）
  const reasonText = (r: Reason) => {
    if (r.k === "reason.nearBest" && (r.p?.diff === undefined || r.p?.diff === null)) return t("reason.nearBestNoDiff");
    return t(r.k, { ...r.p, state: r.p?.stateKey ? t(String(r.p.stateKey)) : undefined });
  };

  const scanning = phase === "scanning";
  const showWall = scanning || phase === "done";
  const elapsed = (performance.now() - startedAt.current) / 1000;
  const eta = progress.done >= 4 ? Math.max(1, Math.round((elapsed / progress.done) * (progress.total - progress.done))) : null;
  // 当前阶段那一行最多 600 ms 更新一次，不显示文件名（M4.2 / A5.2）；"可以离开"只在第一次分析时显示 6 秒
  const [stageText, setStageText] = useState("");
  const [showLeave, setShowLeave] = useState(false);
  useEffect(() => {
    if (!scanning) return;
    const tick = () => { const l = lastStage.current; setStageText(l ? t(`cull.stage.${l.stage}`, { defaultValue: l.stage }) : t("cull.loadingModel")); };
    tick();
    const id = window.setInterval(tick, 600);
    let seen = false;
    try { seen = localStorage.getItem("fg.hint.leave") === "1"; } catch { /* ignore */ }
    let hide = 0;
    if (!seen) { setShowLeave(true); hide = window.setTimeout(() => { setShowLeave(false); try { localStorage.setItem("fg.hint.leave", "1"); } catch { /* ignore */ } }, 6000); }
    return () => { window.clearInterval(id); window.clearTimeout(hide); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scanning]);
  const showBanner = scanning || celebrate;
  const decision = sel ? decisions.get(sel.path) : undefined;
  const pendingFiles = files.filter((f) => !f.cached);

  return (
    
    <div className="module" data-active={active}>
      <section className="workspace">
        <header className="toolbar" data-tauri-drag-region>
          <div className="toolbar-title" data-tauri-drag-region>
            {project && (
              <button className="crumb" onClick={() => setActive(null)} title={t("home.back")}>
                <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M7.5 2.5L4 6l3.5 3.5" /></svg>
                {t("home.back")}
              </button>
            )}
            {project
              ? <EditableTitle value={project.name} tip={t("cull.renameTip")} onCommit={(name) => updateProject(project.id, { name })} />
              : <b>{t("nav.cull")}</b>}
            {project && phase === "done" && <span className="mono ash">{t("cull.photoCount", { count: photos.length })}</span>}
          </div>
          <div className="toolbar-right" data-tauri-drag-region>
            <AnimatePresence>
              {project && phase === "done" && settings.learn && project.learn !== false && dirty > 0 && (
                <motion.button className="btn reeval" onClick={reevaluate} title={t("cull.reevalTip")} initial={{ opacity: 0, x: 8 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0 }}>{t("cull.reeval")}</motion.button>
              )}
            </AnimatePresence>
            {project && phase === "done" && projectSearch && (
              <SearchBox value={query} onChange={setQuery} inputRef={searchRef} placeholder={t("search.placeholder")} progress={index.progress} count={hits ? hits.length : null} onActivate={ensureSearch} />
            )}
            {project && phase === "done" && (
              <button className={`btn icon plain ${panelOpen ? "on" : ""}`} onClick={togglePanel} aria-pressed={panelOpen} aria-label={t("cull.filters")} title={t("cull.filters")}>
                <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><rect x="1.7" y="2.7" width="12.6" height="10.6" rx="2" /><path d="M6 3v10" /></svg>
              </button>
            )}
            {project && phase === "done" && (
              <ViewMenu layout={layout} onLayout={pickLayout} sort={sortBy} onSort={setSortBy}
                layouts={LAYOUTS.map((l) => ({ id: l.id, icon: l.icon, label: t(`cull.layout.${l.id}`) }))}
                sorts={(["group", "order", "time", "score"] as SortBy[]).map((id) => ({ id, label: t(`cull.sort.${id}`) }))} />
            )}
            {project && phase === "done" && <i className="tb-sep" aria-hidden />}
            {project && phase === "done" && <Organize project={project} photos={photos} decisions={decisions} onChanged={() => setReloadKey((k) => k + 1)} />}
            {project && phase === "done" && <i className="tb-sep" aria-hidden />}
            {project && (
              <button className="btn icon plain" onClick={() => setSettingsOpen(true)} disabled={scanning} aria-label={t("pset.settings")} title={t("pset.settings")}>
                <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d="M3 4.5h6.5M12.5 4.5H13M3 11.5h1M7 11.5h6" /><circle cx="11" cy="4.5" r="1.6" /><circle cx="5.5" cy="11.5" r="1.6" /></svg>
              </button>
            )}
            {project && sel && layout !== "compare" && <button className={`btn icon plain ${showInspector ? "on" : ""}`} onClick={onToggleInspector} aria-label={t("cull.inspector")} title={t("cull.inspector")}>
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><rect x="1.7" y="2.7" width="12.6" height="10.6" rx="2" /><path d="M10 3v10" /></svg>
            </button>}
          </div>
        </header>

        <AnimatePresence>
          {showBanner && (
            <motion.div className="banner" initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: DUR.base, ease: EASE_OUT }}>
              <div className="banner-inner">
                {scanning ? (
                  // 同一个数字只出现一次（M4.1）：进度只在这里；保留 / 待复查 / 淘汰的数在筛选栏。两行加一条进度（M4.2）
                  <div className="banner-text">
                    <div className="banner-line">
                      <b>{t("cull.analyzing")}</b>
                      <span className="mono ash banner-count"><Ticker value={progress.done} /> / {progress.total}{eta ? ` · ${t("cull.eta", { s: eta })}` : ""}</span>
                      {showLeave && <span className="ash banner-leave">{t("cull.canLeave")}</span>}
                      <button type="button" className="btn banner-stop" onClick={stopScan}>{t("cull.stop")}</button>
                    </div>
                    <span className="mono ash banner-now">{stageText}</span>
                    <div className="bar live"><motion.i animate={{ width: `${progress.total ? (progress.done / progress.total) * 100 : 0}%` }} transition={{ ease: EASE_OUT, duration: DUR.base }} /></div>
                  </div>
                ) : (
                  <div className="banner-text done">
                    <div className="banner-line"><b>{t("cull.doneTitle")}</b></div>
                    <span className="mono ash">{t("cull.doneBody", counts)}</span>
                  </div>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {scanning && <Flights ref={flights} scrollRef={scrollRef} />}
        <Modal open={!!conflict} onClose={() => setConflict(null)} title={t("cull.busyTitle", { name: conflict?.name ?? "" })}
          footer={<><button className="btn" onClick={() => setConflict(null)}>{t("organize.cancel")}</button><button className="btn primary" onClick={stopOtherAndStart}>{t("cull.busyStop")}</button></>}>
          <p>{t("cull.busyBody", { name: conflict?.name ?? "" })}</p>
        </Modal>
        <div className="body-split">
        {panelShown && (
          <FilterPanel counts={scanning ? { all: files.length, keep: liveCounts?.keep ?? 0, review: liveCounts?.review ?? 0, reject: liveCounts?.reject ?? 0 } : facets.status}
            mode={mode} onMode={setMode} scenes={scanning ? [] : sceneItems} sceneFilter={sceneFilter} onScene={setSceneFilter} onBrowseScenes={openSceneView} width={filterW.width}
            people={scanning ? [] : peopleItems} personFilter={personFilter} onPerson={setPersonFilter} />
        )}
        {panelShown && <Resizer panel={filterW} edge="right" label={t("cull.filters")} />}
        <div className="scroll" ref={scrollRef}>
          {error && error !== "cull.nasOffline" && error !== "cull.nasDisabled" && <p className="error">{t(error, { defaultValue: error })}</p>}
          {(error === "cull.nasOffline" || error === "cull.nasDisabled") && <p className="error">{t(error, { name: nasOff })} <button className="btn sm" onClick={() => setReloadKey((k) => k + 1)}>{t("cull.nasRetry")}</button></p>}
          {saveError && <p className="error">{t("cull.saveFailed", { error: saveError })}</p>}
          {loadError === "corrupt" && <p className="error">{t("cull.loadCorrupt")}</p>}

          {(phase === "probing" || phase === "loading") && nasProg && <p className="ash pad loading">{t("cull.fetchingNas", { share: nasProg.share, done: nasProg.done, total: nasProg.total, size: fmtBytes(nasProg.bytes_total) })}</p>}
          {(phase === "probing" || phase === "loading") && !nasProg && <p className="ash pad loading">{phase === "loading" && mediaProg && mediaProg.done < mediaProg.total ? t("cull.preparingMedia", { done: mediaProg.done, total: mediaProg.total }) : phase === "loading" && peopleProg && peopleProg.done < peopleProg.total ? t("cull.recognising", { done: peopleProg.done, total: peopleProg.total }) : t(phase === "probing" ? "cull.probing" : "cull.loading")}</p>}

          {!project && store.projects.length > 0 && (
            <ProjectHome projects={store.projects} onOpen={(id) => setActive(id)} onNew={() => newProjectFromDialog()}
              running={runningId ? { id: runningId, done: progress.done, total: progress.total } : null} bgDone={bgDone}
              search={index.available && allFolders.length > 0 ? { query, setQuery, hits, progress: index.progress, inputRef: searchRef } : undefined}
              onPick={(id, path) => { setPendingSelect(path); setQuery(""); setActive(id); }}
              onOpenStyles={onOpenStyles} />
          )}

          {!project && store.projects.length === 0 && (
            <div className="empty">
              <motion.h1 initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: DUR.base, ease: EASE_OUT }}>{t("brand.name")}</motion.h1>
              {i18n.language !== "en" && <p className="latin mono">{t("brand.latin")}</p>}
              <motion.h2 initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: DUR.base, ease: EASE_OUT, delay: 0.06 }}>{t("brand.tagline")}</motion.h2>
              <motion.p initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: 0.22 }}>{t("cull.emptyBody", { brand: t("brand.name") })}</motion.p>
              <ol>
                {(t("cull.steps", { returnObjects: true }) as string[]).map((s, i) => (
                  <motion.li key={i} initial={{ opacity: 0, x: -10 }} animate={{ opacity: 1, x: 0 }} transition={{ ...SPRING.soft, delay: 0.3 + i * 0.07 }}>{s}</motion.li>
                ))}
              </ol>
              <motion.div initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: 0.55 }}>
                <button className="btn primary" onClick={() => newProjectFromDialog()}>{t("cull.newProjectBtn")} <kbd>⌘O</kbd></button>
              </motion.div>
            </div>
          )}

          <AnimatePresence>
            {phase === "ready" && project && (
              <ReadyStage key="ready" project={project} count={uncached} sizeText={fmtBytes(bytes)} etaSeconds={etaSeconds} files={pendingFiles} thumbs={previews}
                onStart={startScan} onAddFolder={addFolder} onRemoveFolder={removeFolder} />
            )}
          </AnimatePresence>

          {phase === "done" && sceneView && (
            <SceneGallery groups={sceneGroups} describing={describing} canDescribe={!scenePack && sceneGroups.some((g) => !g.text)} onDescribe={startDescribing} chosen={sceneFilter} onBack={() => setSceneView(false)}
              onPick={(sc) => { setSceneFilter(sc); setSceneView(false); }} />
          )}

          {scanning && mode !== "all" && items.length === 0 && <p className="ash pad">{t("cull.landing")}</p>}
          {phase === "done" && !sceneView && shown.length === 0 && (hits
            ? <div className="pad nosearch"><b>{t("search.none", { q: query.trim() })}</b><p className="ash">{t("search.noneHint")}</p></div>
            : <p className="ash pad">{t("cull.emptyView")}</p>)}

          {showWall && !(phase === "done" && sceneView) && !(phase === "done" && (layout === "single" || layout === "compare")) && (
            <Wall ref={wallRef} scrollRef={scrollRef} widthDelta={wallDelta} items={wallItems} selected={phase === "done" ? selected : null} layout={layout === "grid" ? "grid" : "justified"} render={(i, box) => {
              const it = items[i];
              const photo = scanning ? results.current.get(it.path) : shown[i];
              // 分析中：照片一出结果就按眼下的判定盖章（后来的照片会改变分组和排名，分析完换成正式的）
              const d = photo ? (scanning ? live.current.get(it.path) : decisions.get(it.path)) : undefined;
              const state = !photo ? "" : d?.state ?? "";
              return (
                <Tile key={it.path} path={it.path} ratio={it.ratio} photo={photo} stage={stages.current.get(it.path)}
                  state={state} reveal={celebrate && phase === "done"} dim={state === "reject" && mode === "all"} stamp={scanning}
                  manual={!!d?.manual} groupEnd={groupEnds.has(it.path)} showScore={sortBy === "score"} index={i}
                  interactive={phase === "done" && !!photo} box={box}
                  onSelect={tileSelect} onOpen={openViewer} onDecide={tileDecide} />
              );
            }} />
          )}
          {phase === "done" && !sceneView && layout === "single" && shown.length > 0 && (
            <Single photos={shown} selected={selected} decisions={decisions} onSelect={select} onStep={move} onOpen={openViewer} />
          )}
          {phase === "done" && !sceneView && layout === "compare" && shown.length > 0 && (
            <Compare photos={shown} all={compareAll} scope={sceneFilter !== null ? "scene" : "group"} personFilter={personFilter} onPerson={setPersonFilter} selected={selected} decisions={decisions} onSelect={select} onDecide={decide} onOpen={openViewer} enabled={active && !zoomed} showHint={hintUses < 4} />
          )}
        </div>
        </div>
      </section>

      <AnimatePresence initial={false} onExitComplete={() => setWallDelta(null)}>
        {showInspector && (
          <motion.aside className="inspector" initial={{ width: 0, opacity: 0 }} animate={{ width: inspectorW.width, opacity: 1 }} exit={{ width: 0, opacity: 0 }}
            transition={{ width: resizing ? { duration: 0 } : SPRING.glide, opacity: { duration: 0.2 } }} onAnimationComplete={() => setWallDelta(null)}>
            <Resizer panel={inspectorW} edge="left" label={t("cull.inspector")} onDragging={setResizing} />
            <div className="inspector-fixed" style={{ width: inspectorW.width }}>
              <div className="inspector-head" data-tauri-drag-region><span>{t("cull.inspector")}</span></div>
              <div className="inspector-scroll">
                <AnimatePresence mode="wait" initial={false}>
                  {sel ? (
                    <motion.div key="body" className="inspector-body" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={{ duration: 0.2, ease: EASE_OUT }}>
                      {/* 没有预览缩略图：它就是墙上带焦点框的那一张（M8.1）；名字行右边一个"放大" */}
                      <PhotoVerdict photo={sel} decision={decision} onDecide={(st) => decide(sel.path, st)} reasonText={reasonText}
                        scoreTip={<InfoPop title={t("inspector.overall")} width={280}><p className="ash">{t("inspector.scoreTip")}</p></InfoPop>}
                        onZoom={() => openViewer(sel.path)} />
                      {groupMates.length > 1 && (
                        <div>
                          <h4>{t("inspector.mates")} <span className="mono">{groupMates.length}</span></h4>
                          <div className="mates">
                            {groupMates.map((g) => (
                              <button key={g.path} className={g.path === sel.path ? "cur" : ""} onClick={() => select(g.path)} title={`${g.name} · ${pct(g.score)}`}>
                                <img src={thumbSrc(g)} alt="" />
                                {g.group_best && <i />}
                              </button>
                            ))}
                          </div>
                        </div>
                      )}
                      <div className="accs">
                        <Accordion icon="chart" title={t("inspector.details")} hint={`${t("inspector.local")} ${sel.clip_score === null ? "—" : pct(sel.clip_score)} · ${t("inspector.system")} ${sel.sys_score === null ? "—" : sel.sys_score.toFixed(2)}`}>
                          {/* 只放上面没有的（综合分已在判定里）；人脸也是一行名字 · 值（M8.4 / M8.6） */}
                          <dl className="kv">
                            <div><dt>{t("inspector.local")}</dt><dd className="mono">{sel.clip_score === null ? "—" : pct(sel.clip_score)}</dd></div>
                            <div><dt>{t("inspector.system")}</dt><dd className="mono">{sel.sys_score === null ? "—" : sel.sys_score.toFixed(2)}</dd></div>
                            <div><dt>{t("inspector.clip")}</dt><dd className="mono">{(sel.clipped_high * 100).toFixed(1)}% / {(sel.clipped_low * 100).toFixed(1)}%</dd></div>
                            {sel.face_count > 0 && <div><dt>{t("inspector.faces")}</dt><dd className="mono">{t("inspector.faceValue", { count: sel.face_count, q: sel.face_min === null ? "—" : sel.face_min.toFixed(2) })}</dd></div>}
                          </dl>
                        </Accordion>
                        </div>
                      <FileSection photo={sel} />
                    </motion.div>
                  ) : (
                    <motion.div key="empty" className="inspector-empty" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
                      <p className="ash">{t("inspector.emptyBody")}</p>
                      <p className="ash hints">
                        <span><kbd>←</kbd><kbd>→</kbd> {t("inspector.hintMove")}</span>
                        <span><kbd>{t("inspector.space")}</kbd> {t("inspector.hintZoom")}</span>
                        <span><kbd>P</kbd> {t("state.keep")}</span>
                        <span><kbd>R</kbd> {t("state.review")}</span>
                        <span><kbd>X</kbd> {t("state.reject")}</span>
                      </p>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </div>
          </motion.aside>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {project && phase === "done" && hintUses < 4 && !sceneView && (layout === "justified" || layout === "grid") && (
          <motion.div className="hintcard" initial={{ opacity: 0, y: -8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={{ duration: DUR.base, ease: EASE_OUT, delay: 0.6 }}>
            <span>{t("cull.hint")}</span>
            <button onClick={() => bumpHint(99)}>{t("cull.hintDismiss")}</button>
          </motion.div>
        )}
      </AnimatePresence>

      {project && (
        <ProjectSettings open={settingsOpen} project={project} onClose={() => setSettingsOpen(false)} onAddFolder={addFolder} onRemoveFolder={removeFolder}
          onTrash={() => { setSettingsOpen(false); trashProject(project.id); }}
          onForceRescan={() => { setSettingsOpen(false); forceRescan(); }} />
      )}

      <AnimatePresence>
        {zoomed && sel && (
          <Viewer key="viewer" photo={sel} decision={decision} index={selIndex} total={shown.length}
            onClose={() => setZoomed(false)} onStep={move} onDecide={(st) => decide(sel.path, st)} onRotate={(d) => rotate(sel.path, d)} photos={shown} decisions={decisions} onJump={select} from={from} active={active} />
        )}
      </AnimatePresence>
    </div>
    
  );
}
