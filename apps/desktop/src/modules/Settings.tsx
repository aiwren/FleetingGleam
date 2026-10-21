import { PrefsInfo, bumpPrefsRevision, learnFromFolder, refreshStyles } from "../styles";
import LearnMeter from "../components/LearnMeter";
import { fmtBytes } from "../format";
import { pickLocalFolder, baseName } from "../projectActions";
import { ensurePack, removePack, usePacks } from "../models";
import { adopt, release } from "../strategy";
import { ReactNode, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { open, save } from "@tauri-apps/plugin-dialog";
import Modal from "../components/Modal";
import Stepper from "../components/Stepper";
import { Share, addShare, probeShare, refreshShares, removeShare, setShareEnabled, updateShare, useShares } from "../nas";
import { BackupTarget, backupNow, refreshBackup, removeTarget, restoreBackup, saveTarget, setBackupOptions, useBackup } from "../backup";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import InfoPop from "../components/InfoPop";
import Segmented from "../components/Segmented";
import Select from "../components/Select";
import { LANGS, Pref, setPref, storedPref } from "../i18n";
import { SPRING } from "../motion";
import { type Settings as AppSettings, Theme, bumpCacheRev, setSettings, useScanInfo, useSettings } from "../settings";


function Group({ id, title, index, children }: { id: string; title: string; index: number; children: ReactNode }) {
  return (
    <motion.section id={id} className="sgroup" initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: 0.05 + index * 0.06 }}>
      <h2>{title}</h2>
      <div className="scard">{children}</div>
    </motion.section>
  );
}

/** One setting: a title and a single plain sentence. Anything technical goes behind the "?". */
function Row({ label, desc, children, info, below }: { label: string; desc?: string; children?: ReactNode; info?: ReactNode; below?: ReactNode }) {
  return (
    <div className="srow-wrap">
      <div className="srow">
        <div className="stext">
          <b>{label}{info && <InfoPop title={label}>{info}</InfoPop>}</b>
          {desc && <p className="ash">{desc}</p>}
        </div>
        {children && <div className="sctl">{children}</div>}
      </div>
      {below}
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
    { id: "sec-backup", label: t("settings.backup") },
    { id: "sec-nas", label: t("settings.nas") },
    { id: "sec-about", label: t("settings.about") },
  ];
  const [current, setCurrent] = useState(sections[0].id);

  useEffect(() => {
    if (!active) return;
    setCleared(false); setPrefsReset(false);
    void refreshStyles();
    void refreshShares();
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

  const bytes = (b: number) => fmtBytes(i18n.language, b);
  const clear = async () => {
    try { await invoke("clear_cache"); setCache(0); setCleared(true); bumpCacheRev(); } catch { /* ignore */ }
  };
  const resetPrefs = async () => {
    try { const v = await invoke<PrefsInfo>("prefs_reset"); setPrefs(v && typeof v === "object" ? v : null); setPrefsReset(true); bumpPrefsRevision(); } catch { /* ignore */ }
  };

  const packs = usePacks();
  // network shares the app connects to itself: added here, then a project is started straight from the share
  const shares = useShares();
  const [nasOpen, setNasOpen] = useState(false);
  // 添加 NAS：先填服务器和账号，"测试连接"连上后列出共享让人选，选了才能保存
  const blankNas = { name: "", host: "", user: "", password: "", remember: true };
  const [nasForm, setNasForm] = useState(blankNas);
  /** 正在编辑的那台 NAS 的 id；null = 在添加。 */
  const [nasEditing, setNasEditing] = useState<Share | null>(null);
  const [nasShares, setNasShares] = useState<string[] | null>(null);
  const nasEdit = (sh: Share) => { setNasErr(null); setNasShares(null); setNasEditing(sh); setNasForm({ name: sh.name, host: sh.host, user: sh.user, password: "", remember: true }); setNasOpen(true); };
  const [nasErr, setNasErr] = useState<string | null>(null);
  const nasProbe = async () => {
    setNasBusy("probe"); setNasErr(null); setNasShares(null);
    try {
      const list = await probeShare({ host: nasForm.host, user: nasForm.user, password: nasForm.password, id: nasEditing?.id });
      setNasShares(list);
    } catch (e) { setNasErr(String(e)); }
    setNasBusy(null);
  };
  const [nasBusy, setNasBusy] = useState<string | null>(null);
  const [nasMsg, setNasMsg] = useState<{ id: string; text: string } | null>(null);
  /** 等着二次确认移除的那台 NAS。 */
  const [nasRemoving, setNasRemoving] = useState<Share | null>(null);
  const nasDo = async (sh: Share, f: () => Promise<void>) => {
    setNasBusy(sh.id); setNasMsg(null);
    try { await f(); } catch (e) { setNasMsg({ id: sh.id, text: String(e) }); }
    setNasBusy(null);
  };
  const nasSubmit = async () => {
    setNasBusy("new"); setNasErr(null);
    try {
      if (nasEditing) await updateShare({ id: nasEditing.id, name: nasForm.name, host: nasForm.host, user: nasForm.user, password: nasForm.password, remember: nasForm.remember });
      else await addShare({ name: nasForm.name, host: nasForm.host, user: nasForm.user, password: nasForm.password, remember: nasForm.remember });
      setNasOpen(false); setNasForm(blankNas); setNasShares(null); setNasEditing(null);
    } catch (e) { setNasErr(String(e)); }
    setNasBusy(null);
  };
  // 备份：和"我的设置与喜好"同一份内容，自动存到用户自己的云存储；可以有多个目标（现在只有 Cloudflare R2 一种），各有名字；Secret 只进钥匙串
  const backup = useBackup();
  const blankTarget = { id: undefined as string | undefined, name: "", kind: "r2" as const, account: "", bucket: "", access_key: "", secret: "" };
  const [bkOpen, setBkOpen] = useState(false);
  const [bkForm, setBkForm] = useState(blankTarget);
  const [bkBusy, setBkBusy] = useState<string | null>(null);
  const [bkErr, setBkErr] = useState<string | null>(null);
  const [bkMsg, setBkMsg] = useState<string | null>(null);
  /** 等着二次确认移除的目标。 */
  const [bkRemoving, setBkRemoving] = useState<BackupTarget | null>(null);
  /** 有多个目标时，先问从哪一个恢复。 */
  const [bkPick, setBkPick] = useState(false);
  const bkAdd = () => { setBkErr(null); setBkForm(blankTarget); setBkOpen(true); };
  const bkEdit = (tg: BackupTarget) => { setBkErr(null); setBkForm({ id: tg.id, name: tg.name, kind: tg.kind, account: tg.account, bucket: tg.bucket, access_key: tg.access_key, secret: "" }); setBkOpen(true); };
  const bkEditing = bkForm.id ? backup?.targets.find((x) => x.id === bkForm.id) : undefined;
  const bkSave = async () => {
    setBkBusy("save"); setBkErr(null);
    try { await saveTarget(bkForm); setBkOpen(false); setBkMsg(t("settings.backupSaved")); } catch (e) { setBkErr(String(e)); }
    setBkBusy(null);
  };
  const bkToggle = async () => { if (!backup) return; setBkBusy("toggle"); try { await setBackupOptions({ enabled: !backup.enabled }); } catch (e) { setBkMsg(String(e)); } setBkBusy(null); };
  const bkKeep = async (n: number) => { if (!backup || !(n >= 1)) return; try { await setBackupOptions({ keep: Math.round(n) }); } catch (e) { setBkMsg(String(e)); } };
  const bkNow = async () => {
    setBkBusy("now"); setBkMsg(null);
    try { await backupNow(); setBkMsg(t("settings.backupDone")); } catch (e) { setBkMsg(t("settings.backupFailed", { error: String(e) })); await refreshBackup(); }
    setBkBusy(null);
  };
  const bkRestoreFrom = async (tg: BackupTarget) => {
    setBkPick(false);
    if (!window.confirm(t("settings.backupRestoreConfirm", { name: tg.name }))) return;
    setBkBusy("restore"); setBkMsg(null);
    try {
      const [r, carried] = await restoreBackup(tg.id);
      if (carried && typeof carried === "object") {
        const keep = ["theme", "learn", "search", "customStyles", "eyeMethod", "quality"] as const;
        const patch: Partial<AppSettings> = {};
        for (const k of keep) if (k in carried) (patch as Record<string, unknown>)[k] = (carried as Record<string, unknown>)[k];
        setSettings(patch);
      }
      await refreshStyles(); bumpPrefsRevision();
      setBkMsg(t("settings.importTasteDone", { styles: r.styles, examples: r.examples, refs: r.refs }));
    } catch (e) { setBkMsg(t("settings.backupFailed", { error: String(e) })); }
    setBkBusy(null);
  };
  const bkRestore = () => { const list = backup?.targets ?? []; if (list.length === 1) void bkRestoreFrom(list[0]); else if (list.length > 1) setBkPick(true); };
  const bkRemove = async (tg: BackupTarget) => { setBkRemoving(null); setBkBusy("remove"); setBkMsg(null); try { await removeTarget(tg.id); } catch (e) { setBkMsg(String(e)); } setBkBusy(null); };
  const bkWhen = (secs: number) => new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(new Date(secs * 1000));
  /** 自动备份那一行的说明：最近一次成功、或最近的错误。 */
  const bkSummary = () => {
    if (!backup?.configured) return t("settings.backupDesc");
    const errs = backup.targets.filter((x) => x.last_error);
    if (errs.length) return t("settings.backupLastError", { error: `${errs[0].name}: ${errs[0].last_error}` });
    const last = Math.max(0, ...backup.targets.map((x) => x.last_ok));
    return last ? t("settings.backupLast", { when: bkWhen(last) }) : t("settings.backupNever");
  };
  // "my settings and taste" as one file: what the taste models learned, as embeddings, plus the settings; no photos, no paths
  const [tasteBusy, setTasteBusy] = useState(false);
  const [tasteMsg, setTasteMsg] = useState<string | null>(null);
  type TasteSummary = { styles: number; examples: number; refs: number; skipped: number };
  const exportTaste = async () => {
    const path = await save({ defaultPath: "fleetinggleam-taste.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    setTasteBusy(true); setTasteMsg(null);
    try {
      const r = await invoke<TasteSummary>("taste_export", { path, settings: s });
      setTasteMsg(t("settings.exportTasteDone", { styles: r.styles, examples: r.examples, refs: r.refs }) + (r.skipped ? ` ${t("settings.exportTasteSkipped", { n: r.skipped })}` : ""));
    } catch (e) { setTasteMsg(t("settings.tasteFailed", { error: String(e) })); }
    setTasteBusy(false);
  };
  const importTaste = async () => {
    const path = await open({ filters: [{ name: "JSON", extensions: ["json"] }], multiple: false });
    if (typeof path !== "string") return;
    setTasteBusy(true); setTasteMsg(null);
    try {
      const [r, carried] = await invoke<[TasteSummary, Record<string, unknown> | null]>("taste_import", { path });
      // only the settings that mean the same on any machine; which model is installed here, and the analysis backend, stay as they are
      if (carried && typeof carried === "object") {
        const keep = ["theme", "learn", "search", "customStyles", "eyeMethod", "quality"] as const;
        const patch: Partial<AppSettings> = {};
        for (const k of keep) if (k in carried) (patch as Record<string, unknown>)[k] = carried[k];
        setSettings(patch);
      }
      await refreshStyles(); bumpPrefsRevision();
      invoke<PrefsInfo>("prefs_info").then(setPrefs).catch(() => {});
      setTasteMsg(t("settings.importTasteDone", { styles: r.styles, examples: r.examples, refs: r.refs }));
    } catch (e) { setTasteMsg(t("settings.tasteFailed", { error: String(e) })); }
    setTasteBusy(false);
  };
  const [refBusy, setRefBusy] = useState<number | null>(null);
  const [refFailed, setRefFailed] = useState(false);
  const learnRefs = async () => {
    const dir = await pickLocalFolder();
    if (!dir) return;
    setRefFailed(false); setRefBusy(0);
    const ok = await learnFromFolder(dir, undefined, setRefBusy);
    setRefBusy(null);
    if (!ok) setRefFailed(true);
    else invoke<PrefsInfo>("prefs_info").then(setPrefs).catch(() => {});
  };
  const clipLabel = !info ? "—" : info.clip === null ? t("settings.off") : info.clip.startsWith("gpu") || info.clip.startsWith("coreml") ? t("settings.clipGpu") : t("settings.clipCpu");
  const learnStatus = !prefs || prefs.labels === 0 ? t("settings.learnNone") : t("settings.learnStatus", { n: prefs.labels, k: prefs.keeps });

  return (
    <div className="module" data-active={active}>
      <Modal open={bkOpen} onClose={() => bkBusy !== "save" && setBkOpen(false)} locked={bkBusy === "save"} title={bkEditing ? t("settings.backupEditTitle", { name: bkEditing.name }) : t("settings.backupAddTitle")}
        footer={(<><button className="btn" disabled={bkBusy === "save"} onClick={() => setBkOpen(false)}>{t("settings.nasCancel")}</button>
          <button className="btn primary" disabled={bkBusy !== null || !bkForm.account.trim() || !bkForm.bucket.trim() || !bkForm.access_key.trim() || (!bkForm.secret && !bkEditing?.has_secret)} onClick={bkSave}>{bkBusy === "save" ? t("settings.backupTesting") : t("settings.backupSave")}</button></>)}>
        <div className="nas-form">
          <label><span>{t("settings.backupName")}</span><input className="title-input wide" value={bkForm.name} placeholder={t("settings.backupNameHint")} autoFocus onChange={(e) => setBkForm({ ...bkForm, name: e.target.value })} /></label>
          <label><span>{t("settings.backupKind")}</span><Select value={bkForm.kind} disabled={!!bkEditing} onChange={() => { /* 目前只有一种 */ }}><option value="r2">Cloudflare R2</option></Select></label>
          <p className="ash small">{t("settings.backupHow")}</p>
          <label><span>{t("settings.backupAccount")}</span><input className="title-input wide" value={bkForm.account} placeholder={t("settings.backupAccountHint")} spellCheck={false} onChange={(e) => setBkForm({ ...bkForm, account: e.target.value })} /></label>
          <label><span>{t("settings.backupBucket")}</span><input className="title-input wide" value={bkForm.bucket} placeholder="fleetinggleam" spellCheck={false} onChange={(e) => setBkForm({ ...bkForm, bucket: e.target.value })} /></label>
          <label><span>Access Key ID</span><input className="title-input wide" value={bkForm.access_key} spellCheck={false} onChange={(e) => setBkForm({ ...bkForm, access_key: e.target.value })} /></label>
          <label><span>Secret Access Key</span><input className="title-input wide" type="password" value={bkForm.secret} placeholder={bkEditing?.has_secret ? t("settings.backupSecretKept") : ""} onChange={(e) => setBkForm({ ...bkForm, secret: e.target.value })} /></label>
          {bkErr && <p className="error">{t("settings.backupFailed", { error: bkErr })}</p>}
        </div>
      </Modal>
      <Modal open={!!bkRemoving} onClose={() => setBkRemoving(null)} title={t("settings.backupRemoveTitle", { name: bkRemoving?.name ?? "" })}
        footer={(<><button className="btn" onClick={() => setBkRemoving(null)}>{t("settings.nasCancel")}</button>
          <button className="btn primary" onClick={() => bkRemoving && bkRemove(bkRemoving)}>{t("settings.backupRemoveConfirm")}</button></>)}>
        <p className="ash">{t("settings.backupRemoveBody")}</p>
      </Modal>
      <Modal open={bkPick} onClose={() => setBkPick(false)} title={t("settings.backupRestorePick")} footer={<button className="btn" onClick={() => setBkPick(false)}>{t("settings.nasCancel")}</button>}>
        <div className="picker-sources">
          {(backup?.targets ?? []).map((tg) => (
            <button key={tg.id} className="picker-src" onClick={() => bkRestoreFrom(tg)}><span><b>{tg.name}</b><small>{`Cloudflare R2 · ${tg.bucket}`}{tg.last_ok ? ` · ${t("settings.backupLast", { when: bkWhen(tg.last_ok) })}` : ""}</small></span></button>
          ))}
        </div>
      </Modal>
      <Modal open={!!nasRemoving} onClose={() => setNasRemoving(null)} title={t("settings.nasRemoveTitle", { name: nasRemoving?.name ?? "" })}
        footer={(<><button className="btn" onClick={() => setNasRemoving(null)}>{t("settings.nasCancel")}</button>
          <button className="btn primary" onClick={() => { const sh = nasRemoving; setNasRemoving(null); if (sh) void nasDo(sh, () => removeShare(sh.id)); }}>{t("settings.nasRemoveConfirm")}</button></>)}>
        <p className="ash">{t("settings.nasRemoveBody")}</p>
      </Modal>
      <Modal open={nasOpen} onClose={() => nasBusy !== "new" && setNasOpen(false)} locked={nasBusy === "new"} title={nasEditing ? t("settings.nasEditTitle", { name: nasEditing.name }) : t("settings.nasAddTitle")}
        footer={(<><button className="btn" disabled={nasBusy === "new"} onClick={() => setNasOpen(false)}>{t("settings.nasCancel")}</button>
          <button className="btn primary" disabled={nasBusy !== null || (!nasShares && !(nasEditing && nasForm.host.trim() === nasEditing.host && nasForm.user.trim() === nasEditing.user && !nasForm.password))} onClick={nasSubmit}>{nasBusy === "new" ? t("settings.nasConnecting") : nasEditing ? t("settings.nasSaveChanges") : t("settings.nasConnectSave")}</button></>)}>
        <div className="nas-form">
          <p className="ash small">{t("settings.nasHow")}</p>
          <label><span>{t("settings.nasHost")}</span><input className="title-input wide" value={nasForm.host} placeholder={t("settings.nasHostHint")} spellCheck={false} autoFocus onChange={(e) => { setNasForm({ ...nasForm, host: e.target.value }); setNasShares(null); }} /></label>
          <label><span>{t("settings.nasUser")}</span><input className="title-input wide" value={nasForm.user} placeholder={t("settings.nasGuestHint")} spellCheck={false} onChange={(e) => { setNasForm({ ...nasForm, user: e.target.value }); setNasShares(null); }} /></label>
          <label><span>{t("settings.nasPassword")}</span><input className="title-input wide" type="password" value={nasForm.password} placeholder={nasEditing && nasEditing.user ? t("settings.nasPasswordKept") : ""} onChange={(e) => { setNasForm({ ...nasForm, password: e.target.value }); setNasShares(null); }} onKeyDown={(e) => { if (e.key === "Enter" && nasForm.host.trim()) void nasProbe(); }} /></label>
          <div className="nas-probe">
            <button className="btn" disabled={nasBusy !== null || !nasForm.host.trim()} onClick={nasProbe}>{nasBusy === "probe" ? t("settings.nasProbing") : t("settings.nasProbe")}</button>
            {nasShares && <span className={`ktag ${nasShares.length ? "on" : ""}`}>{nasShares.length ? t("settings.nasProbeOk", { count: nasShares.length }) : t("settings.nasProbeNone")}</span>}
          </div>
          {nasErr && <p className="error">{t("settings.nasFailed", { error: nasErr })}</p>}
          {nasShares && nasShares.length > 0 && (
            <div className="nas-shares">
              {nasShares.map((sh) => <span key={sh} className="ktag">{sh}</span>)}
              <span className="ash small">{t("settings.nasSharesHint")}</span>
            </div>
          )}
          <label><span>{t("settings.nasName")}</span><input className="title-input wide" value={nasForm.name} placeholder={t("settings.nasNameHint")} onChange={(e) => setNasForm({ ...nasForm, name: e.target.value })} /></label>
          <label className="nas-remember"><input type="checkbox" checked={nasForm.remember} onChange={(e) => setNasForm({ ...nasForm, remember: e.target.checked })} /><span>{t("settings.nasRemember")}</span></label>
        </div>
      </Modal>
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
                info={(<><p className="ash">{t("settings.learnDesc")}</p><p className="ash">{learnStatus}</p></>)}
                below={prefs && <LearnMeter info={prefs} />}>
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

            <Group id="sec-backup" title={t("settings.backup")} index={4}>
              <Row label={t("settings.exportTaste")} desc={tasteMsg ?? t("settings.exportTasteDesc")} info={<p className="ash">{t("settings.exportTasteInfo")}</p>}>
                <span className="sctl-pair">
                  <button className="btn" disabled={tasteBusy} onClick={exportTaste}>{t("settings.exportTasteBtn")}</button>
                  <button className="btn" disabled={tasteBusy} onClick={importTaste}>{t("settings.importTasteBtn")}</button>
                </span>
              </Row>
              <Row label={t("settings.backupTitle")} desc={bkMsg ?? bkSummary()} info={<p className="ash">{t("settings.backupInfo")}</p>}>
                <span className="sctl-pair">
                  {backup?.configured && <button className="btn" disabled={bkBusy !== null} onClick={bkNow}>{bkBusy === "now" ? t("settings.backupRunning") : t("settings.backupNow")}</button>}
                  {backup?.configured && <button className="btn" disabled={bkBusy !== null} onClick={bkRestore}>{bkBusy === "restore" ? t("settings.backupRestoring") : t("settings.backupRestore")}</button>}
                  <button className="switch" role="switch" aria-checked={!!backup?.configured && backup.enabled} aria-label={t("settings.backupAuto")} title={t("settings.backupAuto")} disabled={!backup?.configured || bkBusy !== null} onClick={bkToggle} />
                </span>
              </Row>
              <Row label={t("settings.backupKeep")} desc={t("settings.backupKeepHint")}>
                <Stepper value={backup?.keep ?? 30} min={1} max={1000} disabled={!backup?.configured} label={t("settings.backupKeep")} format={(n) => t("settings.backupKeepN", { count: n })} onChange={bkKeep} />
              </Row>
              <Row label={t("settings.backupTargets")} desc={t("settings.backupTargetsDesc")}>
                <button className="btn" disabled={bkBusy !== null} onClick={bkAdd}>{t("settings.backupAddTarget")}</button>
              </Row>
              {(backup?.targets ?? []).map((tg) => (
                <Row key={tg.id} label={tg.name} desc={`Cloudflare R2 · ${tg.bucket}`}>
                  <span className="sctl-pair">
                    <span className={`nas-state ${tg.last_error ? "" : "on"}`}><i />{tg.last_error ? t("settings.backupErrored") : tg.last_ok ? t("settings.backupLast", { when: bkWhen(tg.last_ok) }) : t("settings.backupReady")}</span>
                    <button className="btn icon sm" disabled={bkBusy !== null} onClick={() => bkEdit(tg)} title={t("settings.backupEdit")} aria-label={t("settings.backupEdit")}>
                      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d="M11.3 2.7l2 2L5.5 12.5l-2.8.8.8-2.8zM10 4l2 2" /></svg>
                    </button>
                    <button className="btn icon sm" disabled={bkBusy !== null} onClick={() => setBkRemoving(tg)} title={t("settings.backupRemove")} aria-label={t("settings.backupRemove")}>
                      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d="M2.5 4.5h11M6 4.5V3a.5.5 0 01.5-.5h3A.5.5 0 0110 3v1.5M4 4.5l.6 8.1a1 1 0 001 .9h4.8a1 1 0 001-.9l.6-8.1M6.5 7v4M9.5 7v4" /></svg>
                    </button>
                  </span>
                </Row>
              ))}
            </Group>

            <Group id="sec-nas" title={t("settings.nas")} index={5}>
              <Row label={t("settings.nasTitle")} desc={t("settings.nasDesc")} info={<p className="ash">{t("settings.nasInfo")}</p>}>
                <button className="btn" onClick={() => { setNasErr(null); setNasShares(null); setNasEditing(null); setNasForm(blankNas); setNasOpen(true); }}>{t("settings.nasAdd")}</button>
              </Row>
              {shares.map((sh) => (
                <Row key={sh.id} label={sh.name} desc={`${sh.host} · ${sh.user || t("settings.nasGuest")}${nasMsg?.id === sh.id ? ` · ${nasMsg.text}` : ""}`}>
                  <span className="sctl-pair">
                    <span className={`nas-state ${sh.enabled && sh.connected ? "on" : ""}`}><i />{!sh.enabled ? t("settings.nasDisabled") : sh.connected ? t("settings.nasConnected") : nasBusy === sh.id ? t("settings.nasConnecting") : t("settings.nasOffline")}</span>
                    <button className="switch" role="switch" aria-checked={sh.enabled} aria-label={t("settings.nasEnabled")} title={t("settings.nasEnabled")} disabled={nasBusy === sh.id} onClick={() => nasDo(sh, () => setShareEnabled(sh.id, !sh.enabled))} />
                    <button className="btn icon sm" disabled={nasBusy === sh.id} onClick={() => nasEdit(sh)} title={t("settings.nasEdit")} aria-label={t("settings.nasEdit")}>
                      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d="M11.3 2.7l2 2L5.5 12.5l-2.8.8.8-2.8zM10 4l2 2" /></svg>
                    </button>
                    <button className="btn icon sm" disabled={nasBusy === sh.id} onClick={() => setNasRemoving(sh)} title={t("settings.nasRemove")} aria-label={t("settings.nasRemove")}>
                      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d="M2.5 4.5h11M6 4.5V3a.5.5 0 01.5-.5h3A.5.5 0 0110 3v1.5M4 4.5l.6 8.1a1 1 0 001 .9h4.8a1 1 0 001-.9l.6-8.1M6.5 7v4M9.5 7v4" /></svg>
                    </button>
                  </span>
                </Row>
              ))}
            </Group>

            <Group id="sec-about" title={t("settings.about")} index={6}>
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
