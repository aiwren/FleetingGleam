import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import Modal from "../components/Modal";
import { baseName } from "../projectActions";
import { Project, updateProject } from "../projects";
import ProjectPrefs from "../components/ProjectPrefs";

/** Everything about one project in one place: its name, folders, how it suggests keepers, and deleting it. */
export default function ProjectSettings({ open, project, onClose, onAddFolder, onRemoveFolder, onTrash, onForceRescan }: {
  open: boolean;
  project: Project;
  onClose: () => void;
  onAddFolder: () => void;
  onRemoveFolder: (dir: string) => void;
  onTrash: () => void;
  onForceRescan: () => void;
}) {
  const { t } = useTranslation();
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
          <ProjectPrefs project={project} />
        </section>

        <section>
          <div className="pline">
            <div><b>{t("pset.rescan")}</b><p className="ash small">{t("pset.rescanDesc")}</p></div>
            <button className="btn" onClick={onForceRescan}>{t("pset.rescanBtn")}</button>
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
