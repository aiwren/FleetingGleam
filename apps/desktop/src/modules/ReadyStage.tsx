import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import ProjectPrefs from "../components/ProjectPrefs";
import Ticker from "../components/Ticker";
import { SPRING } from "../motion";
import { baseName } from "../projectActions";
import { Project, updateProject } from "../projects";
import { ProbeFile } from "../types";

/** A rough length of time in words: "about 40 seconds", "about 12 minutes", "about 3 hours". */
const duration = (s: number, t: (k: string, o?: Record<string, unknown>) => string) =>
  s < 90 ? t("cull.etaSeconds", { n: Math.max(5, Math.round(s / 5) * 5) }) : s < 5400 ? t("cull.etaMinutes", { n: Math.round(s / 60) }) : t("cull.etaHours", { n: Math.round(s / 360) / 10 });

/**
 * 分析前的那一屏（设计审查 M3）：左边是名字、文件夹、偏好、备注；右边一张和 Android 一样的封面拼图（左一大右两小），压着张数、大小和预计时间。
 * 没有灯箱桌、不倾斜、不视差、不追光；底部只有"开始分析"。
 */
export default function ReadyStage({ project, count, sizeText, etaSeconds, files, thumbs, onStart, waiting, onAddFolder, onRemoveFolder }: {
  project: Project; count: number; sizeText: string; etaSeconds: number | null; files: ProbeFile[]; thumbs: Record<string, string>;
  /** 正在等模型（询问或下载中）：按钮置灰，写"等待模型"。 */
  waiting: boolean;
  onStart: () => void; onAddFolder: () => void; onRemoveFolder: (dir: string) => void;
}) {
  // typed into local state; written once the typing pauses or the field is left (every character would otherwise rewrite the projects file)
  const [note, setNoteState] = useState(project.note ?? "");
  useEffect(() => { setNoteState(project.note ?? ""); }, [project.id, project.note]);
  const noteTimer = useRef<number | null>(null);
  const flushNote = () => {
    if (noteTimer.current !== null) { window.clearTimeout(noteTimer.current); noteTimer.current = null; }
    if ((project.note ?? "") !== note) updateProject(project.id, { note });
  };
  const setNote = (v: string) => {
    setNoteState(v);
    if (noteTimer.current !== null) window.clearTimeout(noteTimer.current);
    noteTimer.current = window.setTimeout(() => { noteTimer.current = null; if ((project.note ?? "") !== v) updateProject(project.id, { note: v }); }, 500);
  };
  const { t } = useTranslation();
  const [name, setName] = useState(project.name);
  useEffect(() => setName(project.name), [project.id, project.name]);
  const commitName = () => { const v = name.trim(); if (v && v !== project.name) updateProject(project.id, { name: v }); else setName(project.name); };
  const [n, setN] = useState(0);
  useEffect(() => { const id = setTimeout(() => setN(count), 350); return () => clearTimeout(id); }, [count]);
  const src = (f: ProbeFile) => (thumbs[f.path] ? convertFileSrc(thumbs[f.path]) : convertFileSrc(f.path));
  const covers = files.slice(0, 3);

  return (
    <motion.div className="ready-stage" exit={{ opacity: 0, transition: { duration: 0.16 } }}>
      {/* 左栏整体一次淡入（M3.4） */}
      <motion.div className="ready-left" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.32, ease: [0.16, 1, 0.3, 1] }}>
        <div className="ready-scroll">
          <header className="ready-head">
            <input className="ready-name" value={name} onChange={(e) => setName(e.target.value)} onBlur={commitName} aria-label={t("pset.name")}
              onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
          </header>

          <section className="ready-card">
            <h4>{t("pset.folders")}</h4>
            <ul className="pfolders">
              {project.folders.map((f) => (
                <li key={f}>
                  <div className="fpath"><b>{baseName(f)}</b><span className="mono ash">{f}</span></div>
                  {project.folders.length > 1 && <button className="fx" onClick={() => onRemoveFolder(f)} title={t("cull.removeFolder")} aria-label={t("cull.removeFolder")}>×</button>}
                </li>
              ))}
            </ul>
            <button className="btn sm" onClick={onAddFolder}>{t("cull.addFolder")}</button>
          </section>

          <section className="ready-card">
            <h4>{t("pset.prefs")}</h4>
            <ProjectPrefs project={project} />
          </section>

          <section className="ready-card">
            <h4>{t("pset.note")}</h4>
            <textarea className="ready-note" rows={3} value={note} placeholder={t("pset.notePh")}
              onChange={(e) => setNote(e.target.value)} onBlur={flushNote} />
          </section>
        </div>

        {/* 底部只有按钮：解释段删掉了，预计时间在拼图上（M3.2） */}
        <div className="ready-bar">
          <button className="btn primary" onClick={onStart} disabled={waiting}>{waiting ? t("cull.waitModel") : t("cull.start")}</button>
        </div>
      </motion.div>

      {/* 封面拼图：左一大右两小，底部渐变上压着张数、大小、预计时间（M3.1） */}
      <motion.div className="ready-collage" initial={{ opacity: 0, scale: 0.98 }} animate={{ opacity: 1, scale: 1 }} transition={SPRING.soft}>
        <div className={`rc-grid n${Math.min(3, covers.length)}`}>
          {covers.map((f) => <img key={f.path} src={src(f)} alt="" draggable={false} />)}
        </div>
        <div className="rc-shade" />
        <div className="rc-text">
          <div className="rc-count"><Ticker value={n} /><span>{t("cull.readyUnit")}</span></div>
          <span className="mono rc-meta">{[t("cull.readySize", { size: sizeText }), etaSeconds !== null ? t("cull.readyEta", { time: duration(etaSeconds, t) }) : null].filter(Boolean).join(" · ")}</span>
        </div>
      </motion.div>
    </motion.div>
  );
}
