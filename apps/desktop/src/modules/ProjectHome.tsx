import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { SPRING } from "../motion";
import Modal from "../components/Modal";
import SearchBox from "../components/SearchBox";
import { Hit } from "../search";
import { Project, TRASH_DAYS, emptyTrash, purgeProject, restoreProject, trashProject, updateProject } from "../projects";

const icon = (d: string) => <svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>;

/** The module's landing page: every project as a card, plus a recycle bin for the ones you delete. */
export default function ProjectHome({ projects, onOpen, onNew, search, onPick }: {
  projects: Project[];
  onOpen: (id: string) => void;
  onNew: () => void;
  /** Global search over every project (absent while the search model is missing). */
  search?: { query: string; setQuery: (v: string) => void; hits: Hit[] | null; progress: number | null; inputRef: React.RefObject<HTMLInputElement | null> };
  onPick: (projectId: string, path: string) => void;
}) {
  const { t } = useTranslation();
  const live = projects.filter((p) => !p.trashed);
  const bin = projects.filter((p) => p.trashed && Date.now() - p.trashed < TRASH_DAYS * 86400_000);
  const [view, setView] = useState<"projects" | "bin">("projects");
  const [armed, setArmed] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [toast, setToast] = useState<{ id: string; name: string } | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    if (!armed) return;
    const id = setTimeout(() => setArmed(null), 3000);
    return () => clearTimeout(id);
  }, [armed]);
  useEffect(() => { if (view === "bin" && bin.length === 0) setView("projects"); }, [view, bin.length]);

  const [ask, setAsk] = useState<Project | null>(null);
  const moveToBin = (p: Project) => {
    trashProject(p.id);
    setToast({ id: p.id, name: p.name });
    clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 6000);
  };
  const daysLeft = (p: Project) => Math.max(1, Math.ceil(TRASH_DAYS - (Date.now() - (p.trashed ?? 0)) / 86400_000));
  const list = view === "projects" ? live : bin;
  const searching = !!search && view === "projects" && search.query.trim().length > 0;

  return (
    <div className="phome">
      <motion.div className="phome-head" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={SPRING.soft}>
        <div>
          <h1>{t(view === "projects" ? "home.title" : "bin.tab")}</h1>
          <p className="ash">{view === "projects" ? t("home.subtitle") : t("bin.hint", { days: TRASH_DAYS })}</p>
        </div>
        <div className="phome-tools">
          {search && view === "projects" && (
            <SearchBox wide value={search.query} onChange={search.setQuery} inputRef={search.inputRef} placeholder={t("search.globalPlaceholder")} progress={search.progress} count={search.hits ? search.hits.length : null} />
          )}
          <div className="segmented">
            {(["projects", "bin"] as const).map((v) => (
              <button key={v} aria-pressed={view === v} onClick={() => setView(v)} disabled={v === "bin" && bin.length === 0}>
                {view === v && <motion.i layoutId="pill-phome" className="pill" transition={SPRING.glide} />}
                <span className="seg-label">{t(v === "projects" ? "home.title" : "bin.tab")}{v === "bin" && bin.length > 0 ? ` · ${bin.length}` : ""}</span>
              </button>
            ))}
          </div>
          {view === "bin" && (
            <button className="btn" onClick={() => { if (armed === "all") { emptyTrash(); setArmed(null); } else setArmed("all"); }}>
              {armed === "all" ? t("bin.emptyConfirm") : t("bin.emptyAll")}
            </button>
          )}
        </div>
      </motion.div>

      {searching && search && <SearchResults projects={live} hits={search.hits} query={search.query.trim()} onPick={onPick} />}
      <div className="pgrid" style={searching ? { display: "none" } : undefined}>
        <AnimatePresence mode="popLayout" initial>
          {list.map((p, i) => {
            const cover = (p.cover ?? []).slice(0, 4);
            const inBin = view === "bin";
            return (
              <motion.div key={p.id} className={`pcard ${inBin ? "inbin" : ""}`} role="button" tabIndex={0} layout="position"
                initial={{ opacity: 0, y: 16, scale: 0.97 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, scale: 0.94 }}
                transition={{ ...SPRING.soft, delay: 0.04 + i * 0.045 }}
                whileHover={inBin ? undefined : { y: -3, transition: SPRING.tap }} whileTap={inBin ? undefined : { scale: 0.985, transition: SPRING.tap }}
                onClick={() => !inBin && renaming !== p.id && onOpen(p.id)} onKeyDown={(e) => { if (e.key === "Enter" && !inBin && renaming !== p.id) onOpen(p.id); }}>
                <div className={`pcover n${cover.length}`}>
                  {cover.length ? cover.map((c) => <img key={c} src={convertFileSrc(c)} alt="" draggable={false} />) : <span className="ash">{t("home.empty")}</span>}
                </div>
                <div className="pinfo">
                  {renaming === p.id ? (
                    <input className="title-input pname-input" autoFocus defaultValue={p.name} onClick={(e) => e.stopPropagation()}
                      onBlur={(e) => { const v = e.target.value.trim(); if (v) updateProject(p.id, { name: v }); setRenaming(null); }}
                      onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); if (e.key === "Escape") setRenaming(null); }} />
                  ) : <b onDoubleClick={(e) => { if (!inBin) { e.stopPropagation(); setRenaming(p.id); } }}>{p.name}</b>}
                  <span className="mono ash">{inBin ? t("bin.daysLeft", { n: daysLeft(p), count: daysLeft(p) }) : `${t("home.photos", { count: p.count })} · ${t("home.folders", { count: p.folders.length })}`}</span>
                </div>
                {!inBin ? (
                  <div className="pcard-tools" onClick={(e) => e.stopPropagation()}>
                    <button title={t("bin.toBin")} aria-label={t("bin.toBin")} onClick={() => setAsk(p)}>{icon("M2.5 4h9M5.5 4V2.6h3V4M3.8 4l.5 7.4h5.4L10.2 4M5.8 6.2v3.2M8.2 6.2v3.2")}</button>
                  </div>
                ) : (
                  <div className="pbin-actions" onClick={(e) => e.stopPropagation()}>
                    <button className="btn" onClick={() => restoreProject(p.id)}>{t("bin.restore")}</button>
                    <button className={`btn ${armed === p.id ? "on" : ""}`} onClick={() => { if (armed === p.id) { purgeProject(p.id); setArmed(null); } else setArmed(p.id); }}>
                      {armed === p.id ? t("bin.purgeConfirm") : t("bin.purge")}
                    </button>
                  </div>
                )}
              </motion.div>
            );
          })}
          {view === "projects" && (
            <motion.button key="new" className="pcard pnew" layout="position" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}
              transition={{ ...SPRING.soft, delay: 0.04 + list.length * 0.045 }} whileHover={{ y: -3, transition: SPRING.tap }} whileTap={{ scale: 0.985, transition: SPRING.tap }} onClick={onNew}>
              <span className="pplus"><svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round"><path d="M9 3v12M3 9h12" /></svg></span>
              <b>{t("home.new")}</b>
            </motion.button>
          )}
        </AnimatePresence>
      </div>
      {view === "bin" && bin.length === 0 && <p className="ash pad">{t("bin.empty")}</p>}

      <AnimatePresence>
        <Modal open={!!ask} onClose={() => setAsk(null)} title={t("bin.confirmTitle")}
          footer={<>
            <button className="btn" onClick={() => setAsk(null)}>{t("organize.cancel")}</button>
            <button className="btn danger" autoFocus onClick={() => { if (ask) moveToBin(ask); setAsk(null); }}>{t("bin.toBin")}</button>
          </>}>
          <p>{t("bin.confirmBody", { name: ask?.name ?? "", days: TRASH_DAYS })}</p>
        </Modal>
        {toast && (
          <motion.div className="toast" initial={{ opacity: 0, y: 16, scale: 0.96 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 8 }} transition={SPRING.soft}>
            <span>{t("bin.moved")} · {toast.name}</span>
            <button onClick={() => { restoreProject(toast.id); setToast(null); }}>{t("bin.undo")}</button>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}


/** Global search results: every match, grouped under the project it belongs to. Clicking one opens that project with the photo selected. */
function SearchResults({ projects, hits, query, onPick }: { projects: Project[]; hits: Hit[] | null; query: string; onPick: (projectId: string, path: string) => void }) {
  const { t } = useTranslation();
  const groups = useMemo(() => {
    const by = new Map<string, Hit[]>();
    for (const h of hits ?? []) {
      const p = projects.find((x) => x.folders.some((f) => h.path.startsWith(f.endsWith("/") ? f : f + "/")));
      if (p) by.set(p.id, [...(by.get(p.id) ?? []), h]);
    }
    return projects.filter((p) => by.has(p.id)).map((p) => ({ project: p, hits: by.get(p.id)! }));
  }, [projects, hits]);
  if (hits === null) return <p className="ash sres-note">…</p>;
  if (!groups.length) return <div className="sres-note"><b>{t("search.none", { q: query })}</b><p className="ash">{t("search.noneHint")}</p></div>;
  return (
    <div className="sres">
      {groups.map(({ project, hits: hs }, gi) => (
        <motion.section key={project.id} className="sres-group" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: gi * 0.05 }}>
          <h3>{project.name} <span className="mono ash">{t("search.results", { count: hs.length })}</span></h3>
          <div className="sres-grid">
            {hs.slice(0, 60).map((h) => (
              <button key={h.path} className="sres-item" style={{ ["--r" as string]: h.width / h.height }} onClick={() => onPick(project.id, h.path)} title={h.name}>
                <img src={convertFileSrc(h.thumb)} alt="" loading="lazy" decoding="async" draggable={false} />
              </button>
            ))}
          </div>
        </motion.section>
      ))}
    </div>
  );
}
