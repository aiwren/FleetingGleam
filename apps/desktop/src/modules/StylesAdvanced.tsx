import { useState } from "react";
import { useTranslation } from "react-i18next";
import Accordion from "../components/Accordion";
import { pickFolder, baseName } from "../projectActions";
import { StyleInfo, createStyle, deleteStyle, learnFromFolder, renameStyle, resetStyle, setDefaultStyle, useStyles } from "../styles";

/** Advanced: several selection styles. Each one is its own learned taste; a project can be judged by any of them. */
export default function StylesAdvanced() {
  const { t } = useTranslation();
  const styles = useStyles();
  const [busy, setBusy] = useState<{ id: string; p: number } | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  const learn = async (s: StyleInfo) => {
    const dir = await pickFolder();
    if (!dir) return;
    setFailed(null);
    setBusy({ id: s.id, p: 0 });
    const ok = await learnFromFolder(dir, s.id, (p) => setBusy({ id: s.id, p }));
    setBusy(null);
    if (!ok) setFailed(s.id);
  };
  if (!styles.length) return null;
  return (
    <Accordion icon="chart" title={t("styles.title")} hint={styles.length > 1 ? String(styles.length) : undefined}>
      <p className="ash small">{t("styles.desc")}</p>
      <div className="styles">
        {styles.map((s) => (
          <div key={s.id} className="style">
            <div className="style-head">
              <input className="style-name" value={s.name} placeholder={t("styles.defaultName")} aria-label={t("styles.name")} onChange={(e) => renameStyle(s.id, e.target.value)} />
              {s.is_default && <span className="ktag">{t("styles.isDefault")}</span>}
            </div>
            <p className="ash small">
              {s.info.labels + s.info.refs === 0 ? t("styles.empty") : t("styles.stats", { labels: s.info.labels, refs: s.info.refs })}
              {s.info.refs > 0 && s.info.refs_from ? ` · ${baseName(s.info.refs_from)}` : ""}
            </p>
            <div className="style-actions">
              <button className="btn sm" disabled={!!busy} onClick={() => learn(s)}>
                {busy?.id === s.id ? t("styles.learning", { pct: Math.round(busy.p * 100) }) : t("styles.learnFolder")}
              </button>
              {!s.is_default && <button className="btn sm" onClick={() => setDefaultStyle(s.id)}>{t("styles.makeDefault")}</button>}
              {s.info.labels + s.info.refs > 0 && <button className="btn sm" onClick={() => resetStyle(s.id)}>{t("styles.reset")}</button>}
              {!s.is_default && <button className="btn sm" onClick={() => deleteStyle(s.id)}>{t("styles.delete")}</button>}
            </div>
            {failed === s.id && <p className="ash small">{t("styles.failed")}</p>}
          </div>
        ))}
      </div>
      <form className="style-new" onSubmit={(e) => { e.preventDefault(); if (draft.trim()) { createStyle(draft); setDraft(""); } }}>
        <input value={draft} onChange={(e) => setDraft(e.target.value)} placeholder={t("styles.newPlaceholder")} aria-label={t("styles.new")} />
        <button className="btn sm" type="submit" disabled={!draft.trim()}>{t("styles.new")}</button>
      </form>
    </Accordion>
  );
}
