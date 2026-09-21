import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import Modal from "../components/Modal";
import Select from "../components/Select";
import { baseName } from "../projectActions";
import { Project, updateProject } from "../projects";
import { pct } from "../types";
import { ensurePack } from "../models";
import { useStyles } from "../styles";
import { useSettings } from "../settings";

/** Everything about one project in one place: its name, folders, how it suggests keepers, and deleting it. */
export default function ProjectSettings({ open, project, onClose, onAddFolder, onRemoveFolder, onTrash }: {
  open: boolean;
  project: Project;
  onClose: () => void;
  onAddFolder: () => void;
  onRemoveFolder: (dir: string) => void;
  onTrash: () => void;
}) {
  const { t } = useTranslation();
  const settings = useSettings();
  const styles = useStyles();
  const searchOn = project.search ?? settings.search;
  const [name, setName] = useState(project.name);
  const [armed, setArmed] = useState(false);
  useEffect(() => { setName(project.name); setArmed(false); }, [project.id, open, project.name]);
  useEffect(() => {
    if (!armed) return;
    const id = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(id);
  }, [armed]);
  const commitName = () => { const v = name.trim(); if (v && v !== project.name) updateProject(project.id, { name: v }); else setName(project.name); };

  return (
    <Modal open={open} onClose={onClose} title={t("pset.title")} footer={<button className="btn primary sm" onClick={onClose}>{t("organize.done")}</button>}>
      <div className="pset">
        <section>
          <h4>{t("pset.name")}</h4>
          <input className="title-input wide" value={name} onChange={(e) => setName(e.target.value)} onBlur={commitName}
            onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
        </section>

        <section>
          <h4>{t("pset.folders")}</h4>
          <ul className="pfolders">
            {project.folders.map((f) => (
              <li key={f}>
                <div className="fpath"><b>{baseName(f)}</b><span className="mono ash">{f}</span></div>
                <button className="fx" onClick={() => onRemoveFolder(f)} title={t("cull.removeFolder")} aria-label={t("cull.removeFolder")}>×</button>
              </li>
            ))}
          </ul>
          <div className="prow2"><button className="btn" onClick={onAddFolder}>{t("cull.addFolder")}</button><span className="ash small">{t("cull.folderHint")}</span></div>
        </section>

        <section>
          <h4>{t("pset.prefs")}</h4>
          <div className="pline">
            <div><b>{t("pset.keepShare")}</b><p className="ash small">{t("pset.keepShareDesc")}</p></div>
            <label className="keepshare">
              <input type="range" min={0.05} max={0.8} step={0.05} value={project.keepRatio}
                style={{ ["--fill" as string]: `${((project.keepRatio - 0.05) / 0.75) * 100}%` }}
                onChange={(e) => updateProject(project.id, { keepRatio: Number(e.target.value) })} aria-label={t("pset.keepShare")} />
              <span className="mono">{pct(project.keepRatio)}%</span>
            </label>
          </div>
          <div className="pline">
            <div><b>{t("pset.sort")}</b></div>
            <Select value={project.sort ?? "group"} onChange={(e) => updateProject(project.id, { sort: e.target.value as Project["sort"] })} aria-label={t("pset.sort")}>
              <option value="group">{t("cull.sort.group")}</option>
              <option value="order">{t("cull.sort.order")}</option>
              <option value="time">{t("cull.sort.time")}</option>
              <option value="score">{t("cull.sort.score")}</option>
            </Select>
          </div>
          <div className="pline">
            <div><b>{t("pset.learn")}</b><p className="ash small">{t("pset.learnDesc")}</p></div>
            <button className="switch" role="switch" aria-checked={project.learn !== false} aria-label={t("pset.learn")} onClick={() => updateProject(project.id, { learn: project.learn === false })} />
          </div>
          {styles.length > 1 && (
            <div className="pline">
              <div><b>{t("styles.project")}</b><p className="ash small">{t("styles.projectDesc")}</p></div>
              <Select value={project.style ?? ""} onChange={(e) => updateProject(project.id, { style: e.target.value || undefined })} aria-label={t("styles.project")}>
                <option value="">{t("styles.followDefault")}</option>
                {styles.map((st) => <option key={st.id} value={st.id}>{st.name || t("styles.defaultName")}</option>)}
              </Select>
            </div>
          )}
          <div className="pline">
            <div><b>{t("searchset.project")}</b><p className="ash small">{t("searchset.projectDesc")}</p></div>
            <button className="switch" role="switch" aria-checked={searchOn} aria-label={t("searchset.project")} onClick={async () => {
              if (!searchOn && ((await ensurePack("search")) === "cancelled" || (await ensurePack("standard")) === "cancelled")) return;
              updateProject(project.id, { search: !searchOn });
            }} />
          </div>
        </section>

        <section className="danger">
          <div className="pline">
            <div><b>{t("pset.danger")}</b><p className="ash small">{t("pset.dangerDesc")}</p></div>
            <button className={`btn ${armed ? "on" : ""}`} onClick={() => { if (armed) onTrash(); else setArmed(true); }}>{armed ? t("pset.confirmBin") : t("bin.toBin")}</button>
          </div>
        </section>
      </div>
    </Modal>
  );
}
