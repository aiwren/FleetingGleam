import { PrefsInfo, bumpPrefsRevision, learnFromFolder, refreshStyles } from "../styles";
import { pickFolder, baseName } from "../projectActions";
import { ensurePack, removePack, usePacks } from "../models";
import { adopt, release } from "../strategy";
import { ReactNode, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import InfoPop from "../components/InfoPop";
import Segmented from "../components/Segmented";
import Select from "../components/Select";
import { LANGS, Pref, setPref, storedPref } from "../i18n";
import { SPRING } from "../motion";
import { Theme, setSettings, useScanInfo, useSettings } from "../settings";


function Group({ id, title, index, children }: { id: string; title: string; index: number; children: ReactNode }) {
  return (
    <motion.section id={id} className="sgroup" initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: 0.05 + index * 0.06 }}>
      <h2>{title}</h2>
      <div className="scard">{children}</div>
    </motion.section>
  );
}

/** One setting: a title and a single plain sentence. Anything technical goes behind the "?". */
function Row({ label, desc, children, info }: { label: string; desc?: string; children?: ReactNode; info?: ReactNode }) {
  return (
    <div className="srow-wrap">
      <div className="srow">
        <div className="stext">
          <b>{label}{info && <InfoPop title={label}>{info}</InfoPop>}</b>
          {desc && <p className="ash">{desc}</p>}
        </div>
        {children && <div className="sctl">{children}</div>}
      </div>
    </div>
  );
}

export default function Settings({ active, onOpenStyles }: { active: boolean; onOpenStyles: () => void }) {
  const { t, i18n } = useTranslation();
  const s = useSettings();
  const info = useScanInfo();
  const [pref, setPrefState] = useState<Pref>(storedPref());
  const [cache, setCache] = useState<number | null>(null);
  const [version, setVersion] = useState("");
  const [cleared, setCleared] = useState(false);
  const [prefs, setPrefs] = useState<PrefsInfo | null>(null);
  const [prefsReset, setPrefsReset] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const sections = [
    { id: "sec-general", label: t("settings.general") },
    { id: "sec-performance", label: t("settings.analysis") },
    { id: "sec-personal", label: t("settings.personal") },
    { id: "sec-data", label: t("settings.data") },
    { id: "sec-about", label: t("settings.about") },
  ];
  const [current, setCurrent] = useState(sections[0].id);

  useEffect(() => {
    if (!active) return;
    setCleared(false); setPrefsReset(false);
    void refreshStyles();
    invoke<PrefsInfo>("prefs_info").then((v) => setPrefs(v && typeof v === "object" ? v : null)).catch(() => setPrefs(null));
    invoke<number>("cache_size").then((n) => setCache(typeof n === "number" ? n : null)).catch(() => setCache(null));
    getVersion().then((v) => setVersion(v ?? "dev")).catch(() => setVersion("dev"));
  }, [active]);

  // Highlights the nav item for whichever section has scrolled closest to the top.
  useEffect(() => {
    if (!active) return;
    const el = scrollRef.current;
    if (!el) return;
    const onScroll = () => {
      // Near the bottom, the last section may never reach the threshold (not enough room below it to scroll further).
      if (el.scrollHeight - el.scrollTop - el.clientHeight < 4) { setCurrent(sections[sections.length - 1].id); return; }
      const top = el.getBoundingClientRect().top;
      let hit = sections[0].id;
      for (const { id } of sections) {
        const sec = document.getElementById(id);
        if (sec && sec.getBoundingClientRect().top - top <= 96) hit = id;
      }
      setCurrent(hit);
    };
    onScroll();
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);

  const bytes = (b: number) => {
    const u = b >= 1e9 ? ["gigabyte", 1e9, 1] : b >= 1e6 ? ["megabyte", 1e6, 0] : ["kilobyte", 1e3, 0];
    return new Intl.NumberFormat(i18n.language, { style: "unit", unit: u[0] as string, maximumFractionDigits: u[2] as number }).format(b / (u[1] as number));
  };
  const clear = async () => {
    try { await invoke("clear_cache"); setCache(0); setCleared(true); } catch { /* ignore */ }
  };
  const resetPrefs = async () => {
    try { const v = await invoke<PrefsInfo>("prefs_reset"); setPrefs(v && typeof v === "object" ? v : { labels: 0, keeps: 0, refs: 0, refs_from: "", active: false }); setPrefsReset(true); bumpPrefsRevision(); } catch { /* ignore */ }
  };

  const packs = usePacks();
  const [refBusy, setRefBusy] = useState<number | null>(null);
  const [refFailed, setRefFailed] = useState(false);
  const learnRefs = async () => {
    const dir = await pickFolder();
    if (!dir) return;
    setRefFailed(false); setRefBusy(0);
    const ok = await learnFromFolder(dir, undefined, setRefBusy);
    setRefBusy(null);
    if (!ok) setRefFailed(true);
    else invoke<PrefsInfo>("prefs_info").then(setPrefs).catch(() => {});
  };
  const clipLabel = !info ? "—" : info.clip === null ? t("settings.off") : info.clip.startsWith("gpu") || info.clip.startsWith("coreml") ? t("settings.clipGpu") : t("settings.clipCpu");
  const learnStatus = !prefs || prefs.labels === 0 ? t("settings.learnNone")
    : `${t("settings.learnStatus", { n: prefs.labels, k: prefs.keeps })} · ${prefs.active ? t("settings.learnActive") : t("settings.learnWarm")}`;

  return (
    <div className="module" data-active={active}>
      <section className="workspace">
        <header className="toolbar" data-tauri-drag-region>
          <div className="toolbar-title" data-tauri-drag-region><b>{t("settings.title")}</b></div>
        </header>
        <div className="scroll" ref={scrollRef}>
          <div className="settings-layout">
          <nav className="settings-nav" aria-label={t("settings.title")}>
            {sections.map((sec) => (
              <button key={sec.id} className={sec.id === current ? "active" : ""}
                onClick={() => document.getElementById(sec.id)?.scrollIntoView({ behavior: "smooth", block: "start" })}>
                {sec.label}
              </button>
            ))}
          </nav>
          <div className="settings-body">
            <Group id="sec-general" title={t("settings.general")} index={0}>
              <Row label={t("settings.language")} desc={t("settings.languageDesc")}>
                <Select className="langselect" value={pref} aria-label={t("settings.language")}
                  onChange={(e) => { const p = e.target.value as Pref; setPrefState(p); setPref(p); }}>
                  <option value="system">{t("nav.systemLang")}</option>
                  {LANGS.map((l) => <option key={l.code} value={l.code}>{l.label}</option>)}
                </Select>
              </Row>
              <Row label={t("settings.theme")} desc={t("settings.themeDesc")}>
                <Segmented<Theme> id="theme" value={s.theme}
                  options={[{ value: "system", label: t("nav.systemLang") }, { value: "light", label: t("settings.light") }, { value: "dark", label: t("settings.dark") }]}
                  onChange={(v) => setSettings({ theme: v })} />
              </Row>
            </Group>

            <Group id="sec-performance" title={t("settings.analysis")} index={1}>
              <Row label={t("settings.analysisAuto")} desc={t("settings.analysisAutoDesc")}
                info={(
                  <>
                    <p className="ash">{t("settings.analysisHow")}</p>
                    <h4>{t("settings.lastRun")}</h4>
                    {info ? (
                      <dl className="kv">
                        <div><dt>{t("settings.modelUsed")}</dt><dd className="mono">{info.model === null ? "—" : t(info.model.includes("SigLIP") ? "settings.qualityUltra" : info.model.includes("L/14") ? "settings.qualityHigh" : "settings.qualityFast")}</dd></div>
                        <div><dt>{t("settings.backendClip")}</dt><dd className="mono">{clipLabel}</dd></div>
                        <div><dt>{t("settings.backendVision")}</dt><dd className="mono">{info.vision ? (info.cpu_only ? t("settings.visionCpu") : t("settings.visionAuto")) : t("settings.off")}</dd></div>
                        <div><dt>{t("settings.backendCores")}</dt><dd className="mono">{info.cores}</dd></div>
                      </dl>
                    ) : <p className="ash">{t("settings.lastRunNone")}</p>}
                  </>
                )}>
                <span className="mono ash">{s.tier ? t(s.tier === "ultra" ? "settings.qualityUltra" : s.tier === "high" ? "settings.qualityHigh" : "settings.qualityFast") : t("settings.auto")}</span>
              </Row>
            </Group>

            <Group id="sec-personal" title={t("settings.personal")} index={3}>
              <Row label={t("settings.learn")} desc={t("settings.learnShort")}
                info={(<><p className="ash">{t("settings.learnDesc")}</p><p className="ash">{learnStatus}</p></>)}>
                <button className="switch" role="switch" aria-checked={s.learn} aria-label={t("settings.learn")} onClick={() => setSettings({ learn: !s.learn })} />
              </Row>
              <Row label={t("styles.fromFolder")} desc={refFailed ? t("styles.failed") : prefs && prefs.refs > 0 ? `${t("styles.fromFolderDone", { count: prefs.refs })} · ${baseName(prefs.refs_from)}` : t("styles.fromFolderDesc")}
                info={<p className="ash">{t("styles.fromFolderInfo")}</p>}>
                <button className="btn" disabled={refBusy !== null} onClick={learnRefs}>{refBusy !== null ? t("styles.learning", { pct: Math.round(refBusy * 100) }) : t("styles.chooseFolder")}</button>
              </Row>
              <Row label={t("styles.title")} desc={t("styles.desc")}>
                <button className="btn" onClick={onOpenStyles}>{t("styles.open")}</button>
              </Row>
              {prefs && (prefs.labels > 0 || prefs.refs > 0) && (
                <Row label={t("settings.learnReset")} desc={t("settings.learned", { count: prefs.labels + prefs.refs, n: prefs.labels + prefs.refs })}>
                  <button className="btn" onClick={resetPrefs}>{prefsReset ? t("settings.learnResetDone") : t("settings.learnReset")}</button>
                </Row>
              )}
            </Group>

            <Group id="sec-data" title={t("settings.data")} index={3}>
              <div className="srow-wrap">
                <div className="srow"><div className="stext"><b>{t("models.title")}</b><p className="ash">{t("models.titleDesc")}</p></div></div>
              </div>
              {packs.map((p) => (
                <Row key={p.id} label={t(`models.name.${p.id}`)} desc={p.installed ? `${t("models.have")} · ${t(`models.desc.${p.id}`)}` : `${t("models.none")} · ${t(`models.desc.${p.id}`)}`}>
                  {p.installed
                    ? <button className="btn" onClick={async () => { await removePack(p.id); release(p.id); }}>{t("models.remove")}</button>
                    : <button className="btn" onClick={async () => { if ((await ensurePack(p.id)) !== "cancelled") adopt(p.id); }}>{t("models.get")}</button>}
                </Row>
              ))}
              <Row label={t("settings.cache")} desc={t("settings.cacheDesc")}>
                <span className="mono ash cachesize">{cache === null ? "—" : bytes(cache)}</span>
                <button className="btn" onClick={clear} disabled={!cache}>{cleared ? t("settings.cleared") : t("settings.clear")}</button>
              </Row>
              <Row label={t("settings.privacy")} desc={t("settings.privacyBody")} />
            </Group>

            <Group id="sec-about" title={t("settings.about")} index={4}>
              <Row label={t("brand.name")} desc={i18n.language === "en" ? undefined : t("brand.latin")} info={<p className="ash">{t("settings.modelsBody")}</p>}>
                <span className="mono ash">{version ? `v${version}` : ""}</span>
              </Row>
            </Group>
          </div>
          </div>
        </div>
      </section>
    </div>
  );
}
