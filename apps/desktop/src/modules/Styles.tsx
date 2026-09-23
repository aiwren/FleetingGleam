import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "motion/react";
import { pickFolder, baseName } from "../projectActions";
import { StyleInfo, createStyle, deleteStyle, learnFromFolder, refreshStyles, renameStyle, resetStyle, setDefaultStyle, useStyles } from "../styles";
import { useProjects } from "../projects";
import Modal from "../components/Modal";
import SearchBox from "../components/SearchBox";
import Segmented from "../components/Segmented";
import { SPRING, EASE_OUT } from "../motion";

type Sort = "name" | "recent" | "usage";

/** A custom style's id is `s<millis-since-epoch>`, so its creation time is already in there; nothing to store twice. */
function createdAt(id: string): number | null {
  if (id === "default" || !id.startsWith("s")) return null;
  const n = Number(id.slice(1));
  return Number.isFinite(n) ? n : null;
}

/** A small "..." menu for the actions that are secondary or destructive, so the row itself only ever shows the one
 * action worth a full button (learning from a folder). Same open/close/Escape wiring as ViewMenu. */
function RowMenu({ label, children }: { label: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => { if (!root.current?.contains(e.target as Node)) setOpen(false); };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    window.addEventListener("pointerdown", away);
    window.addEventListener("keydown", key);
    return () => { window.removeEventListener("pointerdown", away); window.removeEventListener("keydown", key); };
  }, [open]);
  return (
    <div className="style-menu" ref={root}>
      <button className={`btn sm icon ${open ? "on" : ""}`} aria-label={label} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden><circle cx="3.2" cy="8" r="1.3" /><circle cx="8" cy="8" r="1.3" /><circle cx="12.8" cy="8" r="1.3" /></svg>
      </button>
      <AnimatePresence>
        {open && (
          <motion.div className="vmpanel style-menu-panel" role="menu" initial={{ opacity: 0, y: -6, scale: 0.97 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: -4, scale: 0.98 }} transition={{ duration: 0.18, ease: EASE_OUT }}
            onClick={() => setOpen(false)}>
            <div className="vmlist">{children}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
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

/** New style: a compact "+" that opens into a small form in place, instead of a permanent dashed card the size of a real style. */
function NewStyle({ onCreate }: { onCreate: (name: string) => void }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => { if (open) ref.current?.focus(); }, [open]);
  if (!open) {
    return <button className="btn sm" onClick={() => setOpen(true)}>+ {t("styles.new")}</button>;
  }
  const submit = () => { if (name.trim()) { onCreate(name.trim()); setName(""); setOpen(false); } };
  return (
    <form className="style-new-inline" onSubmit={(e) => { e.preventDefault(); submit(); }}>
      <input ref={ref} value={name} onChange={(e) => setName(e.target.value)} placeholder={t("styles.newPlaceholder")} aria-label={t("styles.new")}
        onKeyDown={(e) => { if (e.key === "Escape") { setName(""); setOpen(false); } }} onBlur={() => { if (!name.trim()) setOpen(false); }} />
      <button className="btn sm" type="submit" disabled={!name.trim()}>{t("styles.new")}</button>
    </form>
  );
}

/** Full-page style management: several learned tastes, each with its own labels, reference photos and model. */
export default function Styles({ active, onBack }: { active: boolean; onBack: () => void }) {
  const { t, i18n } = useTranslation();
  const styles = useStyles();
  const projects = useProjects().projects.filter((p) => !p.trashed);
  const [busy, setBusy] = useState<{ id: string; p: number } | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [confirming, setConfirming] = useState<StyleInfo | null>(null);
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [styles, query, sort, usedBy]);

  const learn = async (s: StyleInfo) => {
    const dir = await pickFolder();
    if (!dir) return;
    setFailed(null);
    setBusy({ id: s.id, p: 0 });
    const ok = await learnFromFolder(dir, s.id, (p) => setBusy({ id: s.id, p }));
    setBusy(null);
    if (!ok) setFailed(s.id);
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
            <button className="crumb" onClick={onBack} title={t("styles.backToSettings")}>
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M7.5 2.5L4 6l3.5 3.5" /></svg>
              {t("styles.backToSettings")}
            </button>
            <b>{t("styles.title")}</b>
            <span className="mono ash">{t("styles.count", { count: styles.length })}</span>
          </div>
        </header>
        <div className="scroll">
          <div className="styles-page">
            <p className="ash">{t("styles.pageDesc")}</p>

            {styles.length > 1 && (
              <div className="styles-toolbar">
                <SearchBox value={query} onChange={setQuery} inputRef={searchRef} placeholder={t("styles.search")} progress={null} count={query ? shown.length : null} />
                <Segmented<Sort> id="style-sort" value={sort} label={t("styles.sortAria")}
                  options={[{ value: "name", label: t("styles.sortName") }, { value: "recent", label: t("styles.sortRecent") }, { value: "usage", label: t("styles.sortUsage") }]}
                  onChange={setSort} />
                <div className="styles-toolbar-add"><NewStyle onCreate={createStyle} /></div>
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
                    <motion.div key={s.id} className="style-card" layout initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} transition={{ ...SPRING.soft, delay: i * 0.04 }}>
                      <div className="style-head">
                        <span className="style-avatar" aria-hidden>{displayName(s).slice(0, 1).toUpperCase()}</span>
                        <NameField value={s.name} placeholder={t("styles.defaultName")} onCommit={(name) => renameStyle(s.id, name)} />
                        {s.is_default && <span className="ktag">{t("styles.isDefault")}</span>}
                        {hasData && <span className={`ktag ${s.info.active ? "on" : ""}`}>{s.info.active ? t("styles.active") : t("styles.warming")}</span>}
                        <RowMenu label={t("styles.more", { name: displayName(s) })}>
                          {!s.is_default && <button role="menuitem" onClick={() => setDefaultStyle(s.id)}><i aria-hidden />{t("styles.makeDefault")}</button>}
                          {hasData && <button role="menuitem" onClick={() => resetStyle(s.id)}><i aria-hidden />{t("styles.reset")}</button>}
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
            {styles.length <= 1 && <div className="styles-toolbar-add single"><NewStyle onCreate={createStyle} /></div>}
          </div>
        </div>
      </section>

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
              <p className="ash">{t("styles.confirmDeleteUsed", { count: usedBy.get(confirming.id)!.length, names: usedBy.get(confirming.id)!.join("、") })}</p>
            )}
          </>
        )}
      </Modal>
    </div>
  );
}
