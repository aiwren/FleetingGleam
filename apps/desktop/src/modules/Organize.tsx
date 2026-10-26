import { useEffect, useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import Modal from "../components/Modal";
import { EASE_OUT } from "../motion";
import { baseName, pickLocalFolder } from "../projectActions";
import { Project, updateProject } from "../projects";
import { Decision, Photo } from "../types";
import { getEnabledShares, nasLabel, openPicker } from "../nas";
import { errText } from "../errors";

type Dlg = null | { kind: "export"; dest: string } | { kind: "trash" };
type Phase = "confirm" | "running" | "done";

/**
 * "整理": what to do with the result. Export copies the keepers somewhere new (originals untouched).
 * Trash moves rejects to the *system* trash, so anything can be put back from Finder.
 */
export default function Organize({ project, photos, decisions, offline = false, onChanged }: {
  project: Project;
  photos: Photo[];
  decisions: Map<string, Decision>;
  /** 项目所在的 NAS 现在连不上（照片来自本机镜像）：传到 NAS、在 NAS 上移到回收站都做不了，置灰。 */
  offline?: boolean;
  onChanged: () => void;
}) {
  const { t, i18n } = useTranslation();
  const [menu, setMenu] = useState(false);
  const [dlg, setDlg] = useState<Dlg>(null);
  const [phase, setPhase] = useState<Phase>("confirm");
  const [progress, setProgress] = useState({ done: 0, total: 0 });
  const [scope, setScope] = useState<"manual" | "all">("manual");
  /** `skipped`：目标里已经有了（同名、同大小、同时间）；`failed`：没导出的那些和原因（约定的错误前缀）；`cancelled`：中途停下了。 */
  const [result, setResult] = useState<{ ok: number; bad: number; skipped?: number; failed?: { path: string; error: string }[]; cancelled?: boolean; dest?: string; bins?: string[]; remote?: boolean; error?: string } | null>(null);
  const [stopping, setStopping] = useState(false);

  const keep = photos.filter((p) => decisions.get(p.path)?.state === "keep");
  const rejectAll = photos.filter((p) => decisions.get(p.path)?.state === "reject");
  const rejectManual = rejectAll.filter((p) => decisions.get(p.path)?.manual);
  const trashList = scope === "manual" ? rejectManual : rejectAll;

  useEffect(() => {
    if (!menu) return;
    const close = (e: MouseEvent) => { if (!(e.target as HTMLElement).closest(".org-wrap")) setMenu(false); };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [menu]);

  const open = (d: Dlg) => { setResult(null); setStopping(false); setPhase("confirm"); setProgress({ done: 0, total: 0 }); setDlg(d); setMenu(false); };
  const close = () => { setDlg(null); if (phase === "done") onChanged(); };

  const startExport = async () => {
    if (dlg?.kind !== "export") return;
    setPhase("running");
    setProgress({ done: 0, total: keep.length });
    const un = await listen<{ done: number; total: number }>("export-progress", (e) => setProgress(e.payload));
    try {
      const r = await invoke<{ copied: number; skipped: number; dest: string; remote?: boolean; failed?: { path: string; error: string }[]; cancelled?: boolean }>("export_photos", { paths: keep.map((p) => p.path), dest: dlg.dest });
      setResult({ ok: r.copied, bad: r.failed?.length ?? 0, skipped: r.skipped, failed: r.failed ?? [], cancelled: !!r.cancelled, dest: r.dest, remote: r.remote });
    } catch (e) { setResult({ ok: 0, bad: keep.length, error: String(e) }); }
    un();
    setPhase("done");
  };

  /** 导出中途停下：核心做完手上这一张就停，返回的结果里 `cancelled` 为真。 */
  const stopExport = () => { setStopping(true); invoke("export_cancel").catch(() => { /* not in Tauri */ }); };

  const startTrash = async () => {
    setPhase("running");
    try {
      const r = await invoke<{ moved: number; failed: string[]; network_bins?: string[] }>("trash_photos", { paths: trashList.map((p) => p.path) });
      const failed = new Set(r.failed);
      const gone = trashList.filter((p) => !failed.has(p.path)).map((p) => p.path);
      // Forget decisions about files that no longer exist.
      const next = { ...project.overrides };
      gone.forEach((g) => delete next[g]);
      updateProject(project.id, { overrides: next });
      setResult({ ok: r.moved, bad: r.failed.length, bins: r.network_bins ?? [] });
    } catch { setResult({ ok: 0, bad: trashList.length }); }
    setPhase("done");
  };

  const chooseDest = async () => {
    const dir = await pickLocalFolder();
    if (dir) open({ kind: "export", dest: dir });
  };
  // 导出到 NAS：选择器只列 NAS；选中的是那个文件夹的镜像路径，核心库认出来就直接传到 NAS 上
  const nasShares = getEnabledShares();
  const chooseNas = async () => {
    setMenu(false);
    const dir = await openPicker(nasShares.length === 1 ? nasShares[0].id : undefined, { nasOnly: true, title: t("organize.exportNasPick"), choose: t("organize.exportHere"), hint: t("picker.rootHintExport") });
    if (dir) open({ kind: "export", dest: dir });
  };

  // 镜像里的照片（…/nas/<id>/…）只有 NAS 连着时才能在 NAS 上移到回收站
  const onNas = (p: Photo) => /\/nas\/[^/]+\//.test(p.path);
  const trashBlocked = offline && rejectAll.some(onNas);
  const items = [
    { key: "export", count: keep.length, title: t("organize.exportItem"), desc: t("organize.exportDesc", { count: keep.length }), run: chooseDest },
    ...(nasShares.length > 0 ? [{ key: "nas", count: keep.length, blocked: offline, title: t("organize.exportNasItem"), desc: t("organize.exportNasDesc", { count: keep.length }), run: chooseNas }] : []),
    { key: "trash", count: rejectAll.length, blocked: trashBlocked, title: t("organize.trashItem"), desc: t("organize.trashDesc", { count: rejectAll.length }), run: () => { setScope(rejectManual.length > 0 ? "manual" : "all"); open({ kind: "trash" }); } },
  ];

  return (
    <div className="org-wrap fmenu-wrap">
      <button className="btn" onClick={() => setMenu((v) => !v)}>{t("organize.button")}</button>
      <AnimatePresence>
        {menu && (
          <motion.div className="fmenu orgmenu" initial={{ opacity: 0, y: -6, scale: 0.98 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: -4, scale: 0.98 }} transition={{ duration: 0.16, ease: EASE_OUT }}>
            {items.map((it) => (
              <button key={it.key} className="orgitem" disabled={it.count === 0 || !!it.blocked} onClick={it.run}>
                <b>{it.title}</b><span className="ash">{it.count === 0 ? t("organize.none") : it.blocked ? t("organize.nasOffline") : it.desc}</span>
              </button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>

      <Modal open={dlg?.kind === "export"} onClose={close} locked={phase === "running"} title={t("organize.exportTitle")}
        footer={phase === "done" ? (
          <>
            {result?.dest && result.ok > 0 && !result.remote && <button className="btn" onClick={() => invoke("reveal", { path: result.dest })}>{t("organize.showInFinder")}</button>}
            <button className="btn primary sm" onClick={close}>{t("organize.done")}</button>
          </>
        ) : (
          <>
            {phase === "running"
              ? <button className="btn" onClick={stopExport} disabled={stopping}>{stopping ? t("organize.exportStopping") : t("organize.exportStop")}</button>
              : <button className="btn" onClick={close}>{t("organize.cancel")}</button>}
            <button className="btn primary sm" onClick={startExport} disabled={phase === "running"}>{phase === "running" ? t("organize.exporting", progress) : t("organize.exportGo")}</button>
          </>
        )}>
        {phase === "done" ? (
          <>
            {result?.error
              ? <p>{t("organize.exportFailed", { error: errText(t, i18n.language, result.error) })}</p>
              : <p>
                  {result?.cancelled ? t("organize.exportStopped", { count: result.ok, left: Math.max(0, keep.length - result.ok - (result.skipped ?? 0) - result.bad) }) : t("organize.exported", { count: result?.ok ?? 0 })}
                  {result?.skipped ? ` · ${t("organize.exportedAlready", { n: result.skipped })}` : ""}
                  {result && result.bad > 0 ? ` · ${t("organize.exportedFailed", { n: result.bad })}` : ""}
                </p>}
            {/* 没导出的列出来：文件名和原因（磁盘满、没权限……），多了只列前几张 */}
            {result?.failed && result.failed.length > 0 && (
              <ul className="mlist export-failed">
                {result.failed.slice(0, 5).map((f) => <li key={f.path}><span className="mono">{baseName(f.path)}</span><span className="ash">{errText(t, i18n.language, f.error)}</span></li>)}
                {result.failed.length > 5 && <li><span className="ash">{t("organize.exportFailedMore", { n: result.failed.length - 5 })}</span></li>}
              </ul>
            )}
            {result?.remote && result.dest && <p className="mono dest">{result.dest}</p>}
          </>
        ) : (
          <>
            <p>{t("organize.exportBody", { count: keep.length })}</p>
            <p className="mono dest">{dlg?.kind === "export" ? nasLabel(dlg.dest) : ""}</p>
            <p className="ash small">{t("organize.exportSafe")}</p>
            {phase === "running" && <div className="bar"><motion.i animate={{ width: `${progress.total ? (progress.done / progress.total) * 100 : 0}%` }} /></div>}
          </>
        )}
      </Modal>

      <Modal open={dlg?.kind === "trash"} onClose={close} locked={phase === "running"} title={t("organize.trashTitle")}
        footer={phase === "done" ? (
          <button className="btn primary sm" onClick={close}>{t("organize.done")}</button>
        ) : (
          <>
            <button className="btn" onClick={close} disabled={phase === "running"}>{t("organize.cancel")}</button>
            <button className="btn primary sm" onClick={startTrash} disabled={phase === "running" || trashList.length === 0}>
              {phase === "running" ? t("organize.trashing") : t("organize.trashGo", { count: trashList.length })}
            </button>
          </>
        )}>
        {phase === "done" ? (
          <>
            <p>{t("organize.trashed", { count: result?.ok ?? 0 })}</p>
            {result && result.bad > 0 && <p className="ash small">{t("organize.trashFailed", { n: result.bad })}</p>}
            {result && (result.bins?.length ?? 0) > 0
              ? <p className="ash small">{t("organize.trashedToShare", { where: result.bins!.join(" · ") })}</p>
              : <p className="ash small">{t("organize.trashRecover")}</p>}
          </>
        ) : (
          <>
            <p>{t("organize.trashBody")}</p>
            <div className="opts" role="radiogroup">
              <label className={`opt ${scope === "manual" ? "on" : ""}`}>
                <input type="radio" name="scope" checked={scope === "manual"} disabled={rejectManual.length === 0} onChange={() => setScope("manual")} />
                <span>{t("organize.scopeManual", { count: rejectManual.length })}</span>
              </label>
              <label className={`opt ${scope === "all" ? "on" : ""}`}>
                <input type="radio" name="scope" checked={scope === "all"} onChange={() => setScope("all")} />
                <span>{t("organize.scopeAll", { count: rejectAll.length })}</span>
              </label>
            </div>
            {scope === "all" && rejectAll.length > rejectManual.length && <p className="warn small">{t("organize.warnAll", { n: rejectAll.length - rejectManual.length })}</p>}
            <div className="mthumbs" aria-hidden>
              {trashList.slice(0, 8).map((p) => <img key={p.path} src={convertFileSrc(p.thumb)} alt="" draggable={false} title={baseName(p.path)} />)}
              {trashList.length > 8 && <span className="mono ash">+{trashList.length - 8}</span>}
            </div>
          </>
        )}
      </Modal>
    </div>
  );
}
