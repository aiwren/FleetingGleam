import { useTranslation } from "react-i18next";
import Segmented from "./Segmented";
import Select from "./Select";
import { Project, updateProject } from "../projects";
import { pct } from "../types";
import { useStyles } from "../styles";

/** The per-project choices that shape how photos are picked. Shared by the settings dialog and the screen before analysis. */
export default function ProjectPrefs({ project }: { project: Project }) {
  const { t } = useTranslation();
  const styles = useStyles();
  return (
    <>
      <div className="pline">
        <div><b>{t("pset.keepShare")}</b><p className="ash small">{project.keepCap === undefined ? t("pset.keepShareDescAuto") : t("pset.keepShareDesc")}</p></div>
        <div className="keepcap">
          {project.keepCap !== undefined && (
            <label className="keepshare">
              <input type="range" min={0.05} max={0.95} step={0.05} value={project.keepCap}
                style={{ ["--fill" as string]: `${((project.keepCap - 0.05) / 0.9) * 100}%` }}
                onChange={(e) => updateProject(project.id, { keepCap: Number(e.target.value) })} aria-label={t("pset.keepShare")} />
              <span className="mono">{pct(project.keepCap)}%</span>
            </label>
          )}
          <button className="switch" role="switch" aria-checked={project.keepCap !== undefined} aria-label={t("pset.keepShareLimit")}
            title={t("pset.keepShareLimit")} onClick={() => updateProject(project.id, { keepCap: project.keepCap === undefined ? 0.5 : undefined })} />
        </div>
      </div>
      <div className="pline">
        <div><b>{t("pset.onePerGroup")}</b><p className="ash small">{t("pset.onePerGroupDesc")}</p></div>
        <button className="switch" role="switch" aria-checked={!!project.onePerGroup} aria-label={t("pset.onePerGroup")} onClick={() => updateProject(project.id, { onePerGroup: !project.onePerGroup })} />
      </div>
      <div className="pline">
        <div><b>{t("cull.pickMode")}</b><p className="ash small">{project.pickMode === "best" ? t("cull.pickModeBestDesc") : t("cull.pickModeDiverseDesc")}</p></div>
        <Segmented<"diverse" | "best"> id="pset-pickmode" value={project.pickMode ?? "diverse"} label={t("cull.pickMode")}
          options={[{ value: "diverse", label: t("cull.pickModeDiverse") }, { value: "best", label: t("cull.pickModeBest") }]}
          onChange={(v) => updateProject(project.id, { pickMode: v })} />
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
      {styles.length > 1 && (
        <div className="pline">
          <div><b>{t("styles.project")}</b><p className="ash small">{t("styles.projectDesc")}</p></div>
          <Select value={project.style ?? ""} onChange={(e) => updateProject(project.id, { style: e.target.value || undefined })} aria-label={t("styles.project")}>
            <option value="">{t("styles.followDefault")}</option>
            {styles.map((st) => <option key={st.id} value={st.id}>{st.name || t("styles.defaultName")}</option>)}
          </Select>
        </div>
      )}
    </>
  );
}
