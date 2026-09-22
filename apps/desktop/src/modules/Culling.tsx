import { useCallback, useEffect, useMemo, useRef, useState, ReactNode } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import InfoPop from "../components/InfoPop";
import Reveal from "../components/Reveal";
import Wall, { WallHandle } from "../components/Wall";
import Single from "./Single";
import ViewMenu from "../components/ViewMenu";
import SearchBox from "../components/SearchBox";
import { usePrefsRevision } from "../styles";
import { ensurePack } from "../models";
import { Hit, searchPhotos, setPendingSelect, takePendingSelect, useSearchIndex } from "../search";
import PhotoVerdict from "../components/PhotoVerdict";
import { FileSection } from "../components/FileFacts";
import Accordion from "../components/Accordion";
import { prefetchLarge } from "../largeSrc";
import Compare from "./Compare";
import Ticker from "../components/Ticker";
import Tile, { Star, StageInfo } from "../components/Tile";
import { decide as computeDecisions } from "../decide";
import { DUR, EASE_OUT, SPRING } from "../motion";
import { newProjectFromDialog, pickFolder } from "../projectActions";
import { Project, setActive, trashProject, updateProject, useProjects } from "../projects";
import { getSettings, ScanInfo, setScanInfo, useSettings } from "../settings";
import { Photo, Probe, ProbeFile, Reason, State, pct } from "../types";
import Organize from "./Organize";
import ReadyStage from "./ReadyStage";
import ProjectSettings from "./ProjectSettings";
import ProjectHome from "./ProjectHome";
import Viewer from "./Viewer";

type ViewMode = "all" | State;
type Phase = "idle" | "probing" | "ready" | "scanning" | "loading" | "done";
type SortBy = "group" | "order" | "time" | "score";
type Item = { path: string; ratio: number; photo?: Photo };

const TABS: ViewMode[] = ["all", "keep", "review", "reject"];
const MAX_TILES = 100000; // the wall is windowed, so there is no need to cap it

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
  const [hits, setHits] = useState<Hit[] | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [layout, setLayout] = useState<ViewStyle>(() => {
    try { const v = localStorage.getItem("fg.layout"); if (v === "justified" || v === "grid" || v === "single" || v === "compare") return v; } catch { /* storage unavailable */ }
    return "justified";
  });
  const pickLayout = (v: ViewStyle) => { setLayout(v); try { localStorage.setItem("fg.layout", v); } catch { /* storage unavailable */ } };
  const [from, setFrom] = useState<{ x: number; y: number; width: number; height: number } | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const undoStack = useRef<{ path: string; prev: State | undefined }[]>([]);
  const [hintUses, setHintUses] = useState(() => { try { return Number(localStorage.getItem("fg.hint.dock") ?? 0); } catch { return 0; } });
  const bumpHint = (n: number) => { setHintUses((u) => { const v = Math.max(u, n === 99 ? 99 : u + 1); try { localStorage.setItem("fg.hint.dock", String(v)); } catch { /* ignore */ } return v; }); };

  const keepRatio = project?.keepRatio ?? 0.3;
  const sortBy: SortBy = project?.sort ?? "group";
  const setSortBy = (v: SortBy) => { if (project) updateProject(project.id, { sort: v }); };
  const overrides = useMemo(() => new Map(Object.entries(project?.overrides ?? {})) as Map<string, State>, [project?.overrides]);

  // Live scan data lives in refs and is flushed once per frame, so thousands of events stay cheap.
  const results = useRef(new Map<string, Photo>());
  const stages = useRef(new Map<string, StageInfo>());
  const lastStage = useRef<{ path: string; stage: string } | null>(null);
  const startedAt = useRef(0);
  const raf = useRef(0);
  const token = useRef(0);
  const [, setTick] = useState(0);
  const flush = useCallback(() => {
    if (!raf.current) raf.current = requestAnimationFrame(() => { raf.current = 0; setTick((n) => n + 1); });
  }, []);

  const runScan = useCallback(async (proj: Project, total: number, quiet: boolean, tok: number, force = false) => {
    results.current.clear(); stages.current.clear(); lastStage.current = null;
    startedAt.current = performance.now();
    setProgress({ done: 0, total });
    setPhase(quiet ? "loading" : "scanning");
    const offs = quiet ? [] : await Promise.all([
      listen<{ done: number; total: number }>("scan-progress", (e) => { if (tok === token.current) setProgress(e.payload); }),
      listen<ScanInfo>("scan-info", (e) => setScanInfo(e.payload)),
      listen<{ path: string; stage: string; pct: number; thumb?: string | null; width?: number | null; height?: number | null }>("scan-stage", (e) => {
        if (tok !== token.current) return;
        const p = e.payload, old = stages.current.get(p.path);
        stages.current.set(p.path, { stage: p.stage, pct: p.pct, thumb: p.thumb ?? old?.thumb, width: p.width ?? old?.width, height: p.height ?? old?.height });
        lastStage.current = { path: p.path, stage: p.stage };
        flush();
      }),
      listen<Photo>("scan-photo", (e) => {
        if (tok !== token.current) return;
        results.current.set(e.payload.path, e.payload);
        stages.current.delete(e.payload.path);
        flush();
      }),
    ]);
    try {
      const res = await invoke<Photo[]>("scan_project", { dirs: proj.folders, compute: getSettings().compute, quality: getSettings().quality, style: proj.style, learn: getSettings().learn && proj.learn !== false, quiet, force, eyeMethod: getSettings().eyeMethod });
      if (tok !== token.current) return;
      setPhotos(res);
      setPhase("done");
      setDirty(0);
      updateProject(proj.id, { cover: [...res].sort((a, b) => b.score - a.score).slice(0, 4).map((p) => p.thumb) });
      if (!quiet) { setCelebrate(true); setTimeout(() => setCelebrate(false), 3200); }
    } catch (e) {
      if (tok === token.current) { setError(String(e)); setPhase("ready"); }
    } finally {
      offs.forEach((f) => f());
    }
  }, [flush]);

  // Entering a project (or changing its folders): read sizes first, then either wait for the user's go-ahead
  // (new photos to analyse) or open instantly from the cache.
  const folderKey = project ? `${project.id}|${project.folders.join("|")}` : "";
  useEffect(() => {
    const tok = ++token.current;
    setSelected(null); setQuery(""); setZoomed(false); setError(null); setMode("all"); setCelebrate(false); setDirty(0);
    if (!project || project.folders.length === 0) { setPhase("idle"); setPhotos([]); setFiles([]); return; }
    setPhase("probing");
    (async () => {
      try {
        const probe = await invoke<Probe>("probe_folder", { dirs: project.folders, compute: getSettings().compute, quality: getSettings().quality });
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
  }, [folderKey, reloadKey, settings.quality, settings.compute, settings.eyeMethod]);

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
    // the analysis model is downloaded the first time it is needed
    const q = getSettings().quality;
    for (const pack of q === "ultra" ? (["standard", "ultra"] as const) : q === "high" ? (["high"] as const) : (["standard"] as const)) {
      if ((await ensurePack(pack)) === "cancelled") return;
    }
    if (getSettings().eyeMethod === "mediapipe" && (await ensurePack("eyes")) === "cancelled") return;
    runScan(project, files.length, false, token.current);
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
  const decisions = useMemo(() => computeDecisions(photos, keepRatio, overrides), [photos, keepRatio, overrides]);
  const counts = useMemo(() => {
    const c = { all: photos.length, keep: 0, review: 0, reject: 0 };
    decisions.forEach((d) => { c[d.state]++; });
    return c;
  }, [decisions, photos.length]);

  // The selected photo stays visible until you move on, even if the change you just made means it no longer matches this view.
  const matches = useCallback((p: Photo) => mode === "all" || decisions.get(p.path)?.state === mode || p.path === selected, [mode, decisions, selected]);
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
  const shown: Photo[] = useMemo(() => {
    const base = ordered.filter(matches);
    if (!hits) return base;
    // While searching, the wall shows the matches, best first, whatever the sort order.
    const rank = new Map(hits.map((h, i) => [h.path, i]));
    return base.filter((p) => rank.has(p.path)).sort((a, b) => rank.get(a.path)! - rank.get(b.path)!);
  }, [ordered, matches, hits]);

  const groupEnds = useMemo(() => {
    const ends = new Set<string>();
    if (sortBy !== "group") return ends;
    const size = new Map<number, number>();
    shown.forEach((p) => size.set(p.group, (size.get(p.group) ?? 0) + 1));
    shown.forEach((p, i) => { if ((size.get(p.group) ?? 0) > 1 && shown[i + 1]?.group !== p.group) ends.add(p.path); });
    return ends;
  }, [shown, sortBy]);

  // Tiles have their final size from the start: real aspect ratios come from the file headers, not from analysis.
  const items: Item[] = useMemo(() => {
    if (phase === "scanning") {
      return files.slice(0, MAX_TILES).map((f) => ({ path: f.path, ratio: f.width / f.height, photo: results.current.get(f.path) }));
    }
    return shown.map((p) => ({ path: p.path, ratio: p.width / p.height, photo: p }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, files, shown, progress.done]);
  const wallItems = useMemo(() => items.map((it) => ({ key: it.path, ratio: it.ratio })), [items]);
  const openViewer = useCallback((path: string) => {
    const r = document.getElementById(`t-${path}`)?.getBoundingClientRect();
    setFrom(r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null);
    setZoomed(true);
  }, []);

  const selIndex = selected ? shown.findIndex((p) => p.path === selected) : -1;
  const sel = selIndex >= 0 ? shown[selIndex] : null;
  /** The inspector is there when a photo is selected and not otherwise (Compare has no selection). */
  const showInspector = !!sel && !inspectorHidden && !!project && layout !== "compare";
  const groupMates = useMemo(() => (sel ? photos.filter((p) => p.group === sel.group) : []), [sel, photos]);

  const select = useCallback((path: string) => setSelected(path), []);
  const decide = useCallback((path: string, st: State | null, fromUndo = false) => {
    if (!project) return;
    const prev = project.overrides[path];
    const next = { ...project.overrides };
    if (st === null) delete next[path]; else next[path] = st;
    updateProject(project.id, { overrides: next });
    // Only explicit keep / reject teach the taste model; "review" and "cleared" mean "not sure".
    invoke("record_label", { path, label: st === null || st === "review" ? null : st === "keep", style: project?.style }).catch(() => { /* not in Tauri */ });
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
  }, [project, mode, shown]);
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
      if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
      const k = e.key.toLowerCase();
      if ((e.metaKey || e.ctrlKey) && k === "z") { e.preventDefault(); undoLast(); return; }
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
  }, [active, zoomed, phase, layout, move, sel, decide, undoLast, openViewer]);

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

  useEffect(() => {
    if (selected) wallRef.current?.reveal(selected);
  }, [selected]);

  useEffect(() => { setQuery(""); }, [project?.id]);
  // ---- search: the index is built in the background, the query runs shortly after typing stops
  const searchOf = (p: Project) => p.search ?? settings.search;
  const allFolders = useMemo(() => store.projects.filter((p) => !p.trashed && (p.search ?? settings.search)).flatMap((p) => p.folders), [store.projects, settings.search]);
  const projectSearch = !!project && searchOf(project);
  const searchDirs = project ? (projectSearch ? project.folders : []) : allFolders;
  const searchKey = searchDirs.join("\n");
  const index = useSearchIndex(project ? (phase === "done" && projectSearch ? project.folders : []) : allFolders);
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
  const reasonText = (r: Reason) => t(r.k, { ...r.p, state: r.p?.stateKey ? t(String(r.p.stateKey)) : undefined });

  const scanning = phase === "scanning";
  const showWall = scanning || phase === "done";
  const elapsed = (performance.now() - startedAt.current) / 1000;
  const eta = progress.done >= 4 ? Math.max(1, Math.round((elapsed / progress.done) * (progress.total - progress.done))) : null;
  const last = lastStage.current;
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
            {!project && <span className="ash">{t("nav.cullSub")}</span>}
            {project && phase === "done" && <span className="mono ash">{t("cull.photoCount", { count: photos.length })}</span>}
            {project && scanning && <span className="mono ash">{progress.done} / {progress.total}</span>}
          </div>
          {phase === "done" && (
            <div className="segmented" role="tablist">
              {TABS.map((id) => (
                <button key={id} role="tab" aria-selected={mode === id} onClick={() => setMode(id)}>
                  {mode === id && <motion.i layoutId="segpill" className="pill" transition={SPRING.glide} />}
                  <span className="seg-label">{id === "keep" && <Star filled size={12} />}{id === "review" && <Star filled={false} size={12} />}{id === "reject" && <svg className="tabx" width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden><path d="M3 3l6 6M9 3L3 9" /></svg>}{t(`state.${id}`)} <Ticker value={counts[id]} className="cnt" /></span>
                </button>
              ))}
            </div>
          )}
          <div className="toolbar-right" data-tauri-drag-region>
            <AnimatePresence>
              {project && phase === "done" && settings.learn && project.learn !== false && dirty > 0 && (
                <motion.button className="btn reeval" onClick={reevaluate} title={t("cull.reevalTip")} initial={{ opacity: 0, x: 8 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0 }}>{t("cull.reeval")}</motion.button>
              )}
            </AnimatePresence>
            {project && phase === "done" && projectSearch && index.available && (
              <SearchBox value={query} onChange={setQuery} inputRef={searchRef} placeholder={t("search.placeholder")} progress={index.progress} count={hits ? hits.length : null} />
            )}
            {project && phase === "done" && (
              <ViewMenu layout={layout} onLayout={pickLayout} sort={sortBy} onSort={setSortBy}
                layouts={LAYOUTS.map((l) => ({ id: l.id, icon: l.icon, label: t(`cull.layout.${l.id}`) }))}
                sorts={(["group", "order", "time", "score"] as SortBy[]).map((id) => ({ id, label: t(`cull.sort.${id}`) }))} />
            )}
            {project && phase === "done" && <Organize project={project} photos={photos} decisions={decisions} onChanged={() => setReloadKey((k) => k + 1)} />}
            {project && (
              <button className="btn icon" onClick={() => setSettingsOpen(true)} disabled={scanning} aria-label={t("pset.settings")} title={t("pset.settings")}>
                <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d="M3 4.5h6.5M12.5 4.5H13M3 11.5h1M7 11.5h6" /><circle cx="11" cy="4.5" r="1.6" /><circle cx="5.5" cy="11.5" r="1.6" /></svg>
              </button>
            )}
            {project && sel && layout !== "compare" && <button className={`btn icon ${showInspector ? "on" : ""}`} onClick={onToggleInspector} aria-label={t("cull.inspector")} title={t("cull.inspector")}>
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><rect x="1.7" y="2.7" width="12.6" height="10.6" rx="2" /><path d="M10 3v10" /></svg>
            </button>}
          </div>
        </header>

        <AnimatePresence>
          {showBanner && (
            <motion.div className="banner" initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: DUR.base + 0.1, ease: EASE_OUT }}>
              <div className="banner-inner">
                {scanning ? (
                  <div className="banner-text">
                    <div className="banner-line">
                      <b>{t("cull.analyzing")}</b>
                      <span className="mono ash banner-count"><Ticker value={progress.done} /> / {progress.total}{eta ? ` · ${t("cull.eta", { s: eta })}` : ""}</span>
                    </div>
                    <span className="mono ash banner-now">
                      {last ? `${last.path.split("/").pop()} · ${t(`cull.stage.${last.stage}`, { defaultValue: last.stage })}` : t("cull.loadingModel")}
                    </span>
                    <div className="bar live"><motion.i animate={{ width: `${progress.total ? (progress.done / progress.total) * 100 : 0}%` }} transition={{ ease: EASE_OUT, duration: 0.5 }} /></div>
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

        <div className="scroll" ref={scrollRef}>
          {error && <p className="error">{t(error, { defaultValue: error })}</p>}

          {(phase === "probing" || phase === "loading") && <p className="ash pad loading">{t(phase === "probing" ? "cull.probing" : "cull.loading")}</p>}

          {!project && store.projects.length > 0 && (
            <ProjectHome projects={store.projects} onOpen={(id) => setActive(id)} onNew={() => newProjectFromDialog()}
              search={index.available && allFolders.length > 0 ? { query, setQuery, hits, progress: index.progress, inputRef: searchRef } : undefined}
              onPick={(id, path) => { setPendingSelect(path); setQuery(""); setActive(id); }}
              onOpenStyles={settings.customStyles ? onOpenStyles : undefined} />
          )}

          {!project && store.projects.length === 0 && (
            <div className="empty">
              <Reveal as="h1" text={t("brand.name")} delay={0.05} />
              {i18n.language !== "en" && <p className="latin mono">{t("brand.latin")}</p>}
              <Reveal as="h2" text={t("brand.tagline")} delay={0.18} />
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
              <ReadyStage key="ready" name={project.name} count={uncached} sizeText={fmtBytes(bytes)} files={pendingFiles} thumbs={previews}
                onStart={startScan} onAddFolder={addFolder} />
            )}
          </AnimatePresence>

          {phase === "done" && shown.length === 0 && (hits
            ? <div className="pad nosearch"><b>{t("search.none", { q: query.trim() })}</b><p className="ash">{t("search.noneHint")}</p></div>
            : <p className="ash pad">{t("cull.emptyView")}</p>)}

          {showWall && !(phase === "done" && (layout === "single" || layout === "compare")) && (
            <Wall ref={wallRef} scrollRef={scrollRef} items={wallItems} selected={phase === "done" ? selected : null} layout={layout === "grid" ? "grid" : "justified"} render={(i, box) => {
              const it = items[i];
              const d = it.photo ? decisions.get(it.path) : undefined;
              const state = phase !== "done" || !it.photo ? "" : d?.state ?? "";
              return (
                <Tile key={it.path} path={it.path} ratio={it.ratio} photo={it.photo} stage={stages.current.get(it.path)}
                  state={state} reveal={celebrate && phase === "done"} dim={state === "reject" && mode === "all"}
                  selected={it.path === selected} manual={!!d?.manual} groupEnd={groupEnds.has(it.path)} showScore={sortBy === "score"} index={i}
                  interactive={phase === "done" && !!it.photo} box={box}
                  onSelect={() => select(it.path)} onOpen={() => openViewer(it.path)} onDecide={(st) => decide(it.path, st)} />
              );
            }} />
          )}
          {phase === "done" && layout === "single" && shown.length > 0 && (
            <Single photos={shown} selected={selected} decisions={decisions} onSelect={select} onStep={move} onOpen={openViewer} />
          )}
          {phase === "done" && layout === "compare" && shown.length > 0 && (
            <Compare photos={shown} all={photos} selected={selected} decisions={decisions} onSelect={select} onDecide={decide} onOpen={openViewer} />
          )}
        </div>
      </section>

      <AnimatePresence initial={false}>
        {showInspector && (
          <motion.aside className="inspector" initial={{ width: 0, opacity: 0 }} animate={{ width: 300, opacity: 1 }} exit={{ width: 0, opacity: 0 }}
            transition={{ width: SPRING.glide, opacity: { duration: 0.2 } }}>
            <div className="inspector-fixed">
              <div className="inspector-head" data-tauri-drag-region><span>{t("cull.inspector")}</span></div>
              <div className="inspector-scroll">
                <AnimatePresence mode="wait" initial={false}>
                  {sel ? (
                    <motion.div key="body" className="inspector-body" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={{ duration: 0.2, ease: EASE_OUT }}>
                      {layout !== "single" && <button className="preview" onClick={() => openViewer(sel.path)} title={t("inspector.zoomHint")}>
                        <img src={convertFileSrc(sel.thumb)} alt="" />
                        <span className="preview-hint">{t("inspector.zoomLabel")}</span>
                      </button>}
                      <PhotoVerdict photo={sel} decision={decision} onDecide={(st) => decide(sel.path, st)} reasonText={reasonText}
                        scoreTip={<InfoPop title={t("inspector.overall")} width={280}><p className="ash">{t("inspector.scoreTip")}</p></InfoPop>} />
                      {groupMates.length > 1 && (
                        <div>
                          <h4>{t("inspector.mates")} <span className="mono">{groupMates.length}</span></h4>
                          <div className="mates">
                            {groupMates.map((g) => (
                              <button key={g.path} className={g.path === sel.path ? "cur" : ""} onClick={() => select(g.path)} title={`${g.name} · ${pct(g.score)}`}>
                                <img src={convertFileSrc(g.thumb)} alt="" />
                                {g.group_best && <i />}
                              </button>
                            ))}
                          </div>
                        </div>
                      )}
                      <div className="accs">
                        <Accordion icon="chart" title={t("inspector.details")} hint={`${sel.clip_score === null ? "—" : pct(sel.clip_score)} · ${sel.sys_score === null ? "—" : sel.sys_score.toFixed(2)}`}>
                          {/* only what the verdict above does not already show */}
                          <dl className="kv">
                            <div><dt>{t("inspector.local")}</dt><dd className="mono">{sel.clip_score === null ? "—" : pct(sel.clip_score)}</dd></div>
                            <div><dt>{t("inspector.system")}</dt><dd className="mono">{sel.sys_score === null ? "—" : sel.sys_score.toFixed(2)}</dd></div>
                            <div><dt>{t("inspector.clip")}</dt><dd className="mono">{(sel.clipped_high * 100).toFixed(1)}% / {(sel.clipped_low * 100).toFixed(1)}%</dd></div>
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
        {project && phase === "done" && hintUses < 4 && (
          <motion.div className="hintcard" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 8 }} transition={{ ...SPRING.soft, delay: 0.6 }}>
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
            onClose={() => setZoomed(false)} onStep={move} onDecide={(st) => decide(sel.path, st)} from={from} />
        )}
      </AnimatePresence>
    </div>
  );
}
