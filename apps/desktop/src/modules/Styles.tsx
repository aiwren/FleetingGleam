import { useEffect, useMemo, useRef, useState } from "react";
import RowMenu from "../components/RowMenu";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { pickFolder, baseName } from "../projectActions";
import { StyleInfo, createStyle, deleteStyle, learnFromFolder, refreshStyles, renameStyle, resetStyle, setDefaultStyle, useStyles } from "../styles";
import LearnMeter from "../components/LearnMeter";
import { useProjects } from "../projects";
import Modal from "../components/Modal";
import SearchBox from "../components/SearchBox";
import Segmented from "../components/Segmented";
import { SPRING } from "../motion";

type Sort = "name" | "recent" | "usage";

/** A custom style's id is `s<millis-since-epoch>`, so its creation time is already in there; nothing to store twice. */
function createdAt(id: string): number | null {
  if (id === "default" || !id.startsWith("s")) return null;
  const n = Number(id.slice(1));
  return Number.isFinite(n) ? n : null;
}

/** Rename in place: a plain visible field (more discoverable in a management list than a double-click reveal), but it
 * only ever tells the backend once editing is done, not on every keystroke. */
function NameField({ value, placeholder, onCommit }: { value: string; placeholder: string; onCommit: (v: string) => void }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  const commit = () => { const v = text.trim(); if (v && v !== value) onCommit(v); else setText(value); };
  return (
    <input className="style-name" value={text} placeholder={placeholder} aria-label={placeholder}
      onChange={(e) => setText(e.target.value)} onBlur={commit}
      onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); if (e.key === "Escape") { setText(value); (e.target as HTMLInputElement).blur(); } }} />
  );
}

/** New style, as a proper dialog: a name, and whether to start teaching it from a folder of photos right away. */
function NewStyleDialog({ open, onClose, onCreate }: { open: boolean; onClose: () => void; onCreate: (name: string, learnNow: boolean) => void }) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [learnNow, setLearnNow] = useState(true);
  useEffect(() => { if (open) { setName(""); setLearnNow(true); } }, [open]);
  const submit = () => { const v = name.trim(); if (v) { onCreate(v, learnNow); onClose(); } };
  return (
    <Modal open={open} onClose={onClose} title={t("styles.new")}
      footer={(
        <>
          <button className="btn" onClick={onClose}>{t("organize.cancel")}</button>
          <button className="btn primary" disabled={!name.trim()} onClick={submit}>{learnNow ? t("styles.createAndLearn") : t("styles.create")}</button>
        </>
      )}>
      <form className="style-new" onSubmit={(e) => { e.preventDefault(); submit(); }}>
        <label className="style-new-field">
          <span>{t("styles.newName")}</span>
          <input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder={t("styles.newPlaceholder")} maxLength={40} />
        </label>
        <div className="style-new-tags">
          {(t("styles.newExamples", { returnObjects: true }) as string[]).map((x) => (
            <button type="button" key={x} className="style-tag" onClick={() => setName(x)}>{x}</button>
          ))}
        </div>
        <div className="pline">
          <div><b>{t("styles.newLearn")}</b><p className="ash small">{t("styles.newLearnDesc")}</p></div>
          <button type="button" className="switch" role="switch" aria-checked={learnNow} aria-label={t("styles.newLearn")} onClick={() => setLearnNow((v) => !v)} />
        </div>
      </form>
    </Modal>
  );
}

/** Full-page style management: several learned tastes, each with its own labels, reference photos and model. */
export default function Styles({ active, from, onBack }: { active: boolean; from: "culling" | "settings"; onBack: () => void }) {
  const { t, i18n } = useTranslation();
  const styles = useStyles();
  const projects = useProjects().projects.filter((p) => !p.trashed);
  const [busy, setBusy] = useState<{ id: string; p: number } | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [confirming, setConfirming] = useState<StyleInfo | null>(null);
  /** 等着确认清空的风格（清空只丢学到的内容，风格和用它的项目都在）。 */
  const [clearing, setClearing] = useState<StyleInfo | null>(null);
  const [creating, setCreating] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => { if (active) void refreshStyles(); }, [active]);

  // Who actually uses each style: an unset project.style means "whichever style is default", not a fixed id.
  const usedBy = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const p of projects) {
      const id = p.style || styles.find((s) => s.is_default)?.id;
      if (!id) continue;
      map.set(id, [...(map.get(id) ?? []), p.name]);
    }
    return map;
  }, [projects, styles]);

  const displayName = (s: StyleInfo) => s.name.trim() || t("styles.defaultName");

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    let list = styles.filter((s) => !q || displayName(s).toLowerCase().includes(q) || (usedBy.get(s.id) ?? []).some((n) => n.toLowerCase().includes(q)));
    const rest = list.filter((s) => !s.is_default).sort((a, b) => {
      if (sort === "recent") return (createdAt(b.id) ?? 0) - (createdAt(a.id) ?? 0);
      if (sort === "usage") return (usedBy.get(b.id)?.length ?? 0) - (usedBy.get(a.id)?.length ?? 0) || displayName(a).localeCompare(displayName(b));
      return displayName(a).localeCompare(displayName(b));
    });
    const def = list.find((s) => s.is_default);
    return def ? [def, ...rest] : rest;
    // displayName 每次渲染新建，只随 t 变（换语言时没起名的风格按新名字重新筛、排）
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [styles, query, sort, usedBy, t]);

  const learn = async (s: StyleInfo) => {
    const dir = await pickFolder();
    if (!dir) return;
    setFailed(null);
    setBusy({ id: s.id, p: 0 });
    const ok = await learnFromFolder(dir, s.id, (p) => setBusy({ id: s.id, p }));
    setBusy(null);
    if (!ok) setFailed(s.id);
  };

  const create = async (name: string, learnNow: boolean) => {
    const id = await createStyle(name);
    if (id && learnNow) { const made = { id } as StyleInfo; await learn(made); }
  };

  const confirmDelete = () => {
    if (!confirming) return;
    deleteStyle(confirming.id);
    setConfirming(null);
  };

  return (
    <div className="module" data-active={active}>
      <section className="workspace">
        <header className="toolbar" data-tauri-drag-region>
          <div className="toolbar-title" data-tauri-drag-region>
            <button className="crumb" onClick={onBack} title={t(from === "settings" ? "styles.backToSettings" : "styles.backToProjects")}>
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M7.5 2.5L4 6l3.5 3.5" /></svg>
              {t(from === "settings" ? "styles.backToSettings" : "styles.backToProjects")}
            </button>
            <b>{t("styles.title")}</b>
            <span className="mono ash">{t("styles.count", { count: styles.length })}</span>
          </div>
        </header>
        <div className="scroll">
          <div className="styles-page">
            <div className="styles-head">
              <p className="ash">{t("styles.pageDesc")}</p>
              <button className="btn primary" onClick={() => setCreating(true)}>+ {t("styles.new")}</button>
            </div>

            {styles.length > 1 && (
              <div className="styles-toolbar">
                <SearchBox value={query} onChange={setQuery} inputRef={searchRef} placeholder={t("styles.search")} progress={null} count={query ? shown.length : null} />
                <Segmented<Sort> id="style-sort" value={sort} label={t("styles.sortAria")}
                  options={[{ value: "name", label: t("styles.sortName") }, { value: "recent", label: t("styles.sortRecent") }, { value: "usage", label: t("styles.sortUsage") }]}
                  onChange={setSort} />
              </div>
            )}

            {shown.length === 0 ? (
              <div className="style-empty">
                <p><b>{t("styles.searchNone", { query })}</b></p>
                <p className="ash">{t("styles.searchNoneBody")}</p>
              </div>
            ) : (
              <div className="styles-list">
                {shown.map((s, i) => {
                  const used = usedBy.get(s.id) ?? [];
                  const created = createdAt(s.id);
                  const hasData = s.info.labels + s.info.refs > 0;
                  return (
                    <motion.div key={s.id} className="style-card" layout initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} transition={{ ...SPRING.soft, delay: Math.min(i, 8) * 0.024 }}>
                      <div className="style-head">
                        <span className="style-avatar" aria-hidden>{displayName(s).slice(0, 1).toUpperCase()}</span>
                        <NameField value={s.name} placeholder={t("styles.defaultName")} onCommit={(name) => renameStyle(s.id, name)} />
                        {s.is_default && <span className="ktag">{t("styles.isDefault")}</span>}
                        {hasData && <span className={`ktag ${s.info.active ? "on" : ""}`}>{s.info.active ? t("styles.active") : t("styles.warming")}</span>}
                        <RowMenu label={t("styles.more", { name: displayName(s) })}>
                          {!s.is_default && <button role="menuitem" onClick={() => setDefaultStyle(s.id)}><i aria-hidden />{t("styles.makeDefault")}</button>}
                          {hasData && <button role="menuitem" onClick={() => setClearing(s)}><i aria-hidden />{t("styles.reset")}</button>}
                          {!s.is_default && <button role="menuitem" className="danger" onClick={() => setConfirming(s)}><i aria-hidden />{t("styles.delete")}</button>}
                        </RowMenu>
                      </div>
                      <p className="ash small style-stats">
                        {hasData ? t("styles.stats", { labels: s.info.labels, refs: s.info.refs }) : t("styles.empty")}
                        {s.info.refs > 0 && s.info.refs_from ? ` · ${baseName(s.info.refs_from)}` : ""}
                        {used.length > 0 ? ` · ${t("styles.usedBy", { count: used.length })}` : ""}
                        {created ? ` · ${t("styles.createdOn", { date: new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" }).format(created) })}` : ""}
                        {failed === s.id ? ` · ${t("styles.failed")}` : ""}
                      </p>
                      <LearnMeter info={s.info} compact />
                      <div className="style-actions">
                        <button className="btn sm" disabled={!!busy} onClick={() => learn(s)}>
                          {busy?.id === s.id ? t("styles.learning", { pct: Math.round(busy.p * 100) }) : t("styles.learnFolder")}
                        </button>
                      </div>
                    </motion.div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </section>

      <NewStyleDialog open={creating} onClose={() => setCreating(false)} onCreate={create} />

      <Modal open={!!clearing} onClose={() => setClearing(null)} title={clearing ? t("styles.confirmResetTitle", { name: displayName(clearing) }) : ""}
        footer={(
          <>
            <button className="btn" onClick={() => setClearing(null)}>{t("organize.cancel")}</button>
            <button className="btn danger" autoFocus onClick={() => { if (clearing) void resetStyle(clearing.id); setClearing(null); }}>{t("styles.confirmResetAction")}</button>
          </>
        )}>
        {clearing && <p>{t("styles.confirmResetBody", { labels: clearing.info.labels, refs: clearing.info.refs })}</p>}
      </Modal>

      <Modal open={!!confirming} onClose={() => setConfirming(null)} title={confirming ? t("styles.confirmDeleteTitle", { name: confirming.name.trim() || t("styles.defaultName") }) : ""}
        footer={(
          <>
            <button className="btn" onClick={() => setConfirming(null)}>{t("organize.cancel")}</button>
            <button className="btn danger" autoFocus onClick={confirmDelete}>{t("styles.confirmDeleteAction")}</button>
          </>
        )}>
        {confirming && (
          <>
            <p>{t("styles.confirmDeleteBody", { labels: confirming.info.labels, refs: confirming.info.refs })}</p>
            {(usedBy.get(confirming.id) ?? []).length > 0 && (
              <p className="ash">{t("styles.confirmDeleteUsed", { count: usedBy.get(confirming.id)!.length, names: new Intl.ListFormat(i18n.language, { type: "conjunction" }).format(usedBy.get(confirming.id)!) })}</p>
            )}
          </>
        )}
      </Modal>
    </div>
  );
}
