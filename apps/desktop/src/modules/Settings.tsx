import { PrefsInfo, bumpPrefsRevision, learnFromFolder, refreshStyles } from "../styles";
import LearnMeter from "../components/LearnMeter";
import { fmtBytes } from "../format";
import { errKind, errText } from "../errors";
import { pickLocalFolder, baseName } from "../projectActions";
import { ensurePack, ensurePacks, removePack, showGate, useGate, usePacks } from "../models";
import { adopt, release, tierPacks } from "../strategy";
import RowMenu from "../components/RowMenu";
import { ReactNode, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { links } from "../links";
import appIcon from "../assets/logo.png";
import Modal from "../components/Modal";
import Stepper from "../components/Stepper";
import { Share, addShare, probeShare, refreshShares, removeShare, setShareEnabled, updateShare, useShares } from "../nas";
import { BackupTarget, backupNow, peekBackup, refreshBackup, removeTarget, restoreBackup, saveTarget, setBackupOptions, useBackup } from "../backup";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import InfoPop from "../components/InfoPop";
import Segmented from "../components/Segmented";
import Select from "../components/Select";
import { LANGS, Pref, setPref, storedPref } from "../i18n";
import { type Settings as AppSettings, Theme, bumpCacheRev, setSettings, useScanInfo, useSettings } from "../settings";


// 分组不再逐个入场，整页一次淡入（M9.8）
function Group({ id, title, children }: { id: string; title: string; index?: number; children: ReactNode }) {
  return (
    <section id={id} className="sgroup">
      <h2>{title}</h2>
      <div className="scard">{children}</div>
    </section>
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
  /** 重置前先确认，写明会丢掉多少（和删除风格一样）。 */
  const [resetAsk, setResetAsk] = useState(false);
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
  // 网络代理：系统的（启动时读）和手填的（优先）；下载模型、云备份走它
  type ProxyInfo = { system: string | null; manual: string | null; active: string | null; exceptions?: string[]; exclude_simple?: boolean };
  const [proxy, setProxy] = useState<ProxyInfo | null>(null);
  const [proxyOpen, setProxyOpen] = useState(false);
  const [proxyText, setProxyText] = useState("");
  const [proxyErr, setProxyErr] = useState<string | null>(null);
  useEffect(() => {
    if (active) invoke<ProxyInfo>("proxy_info").then((v) => setProxy(v && typeof v === "object" ? v : null)).catch(() => setProxy(null));
  }, [active]);
  const proxySave = async (manual: string | null) => {
    setProxyErr(null);
    try { setProxy(await invoke<ProxyInfo>("proxy_set", { manual })); setProxyOpen(false); }
    catch (e) { const [code, scheme] = String(e).split(":"); setProxyErr(t(`settings.proxyBad.${code}`, { scheme, defaultValue: String(e) })); }
  };
  // 系统设置的"不使用代理的主机"交给了核心，这些地址（以及局域网与本机）直连；走代理时把列表写出来
  const bypass = proxy?.active ? [...(proxy.exceptions ?? []), ...(proxy.exclude_simple ? [t("settings.proxySimple")] : [])] : [];
  const proxyDesc = !proxy ? "" : (proxy.manual ? t("settings.proxyManual", { url: proxy.manual }) : proxy.system ? t("settings.proxySystem", { url: proxy.system }) : t("settings.proxyDirect"))
    + (bypass.length ? ` ${t("settings.proxyBypass", { list: bypass.join(i18n.language.startsWith("zh") ? "、" : ", ") })}` : "");
  // NAS 副本：每台 NAS 的照片在本机镜像里占多少（不算在缩略图缓存里），可以按 NAS 清掉；分析结果保留
  type MirrorUsage = { id: string; name: string; bytes: number };
  const [mirrors, setMirrors] = useState<MirrorUsage[]>([]);
  const [mirrorAsk, setMirrorAsk] = useState<MirrorUsage | null>(null);
  const [mirrorMsg, setMirrorMsg] = useState<string | null>(null);
  const refreshMirrors = () => invoke<MirrorUsage[]>("nas_mirror_usage").then((v) => setMirrors(Array.isArray(v) ? v : [])).catch(() => setMirrors([]));
  useEffect(() => { if (active) { setMirrorMsg(null); void refreshMirrors(); } }, [active]);
  const clearMirror = async (m: MirrorUsage) => {
    setMirrorAsk(null);
    try { const freed = await invoke<number>("nas_mirror_clear", { id: m.id }); setMirrorMsg(t("settings.mirrorCleared", { name: m.name, size: bytes(freed ?? 0) })); }
    catch (e) { setMirrorMsg(errText(t, i18n.language, e)); }
    void refreshMirrors();
  };
  const clear = async () => {
    try { await invoke("clear_cache"); setCache(0); setCleared(true); bumpCacheRev(); } catch { /* ignore */ }
  };
  const resetPrefs = async () => {
    setResetAsk(false);
    try { const v = await invoke<PrefsInfo>("prefs_reset"); setPrefs(v && typeof v === "object" ? v : null); setPrefsReset(true); bumpPrefsRevision(); } catch { /* ignore */ }
  };

  const packs = usePacks();
  // 正在下的那个包（可能已转到后台）：它那一行显示百分比，点一下把下载对话框叫回来
  const gate = useGate();
  const downloading = gate?.phase === "downloading" ? (gate.ids ?? [gate.id]) : [];
  const gatePct = gate?.total ? Math.min(100, Math.round((gate.done / gate.total) * 100)) : 0;
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
    } catch (e) { setNasErr(errText(t, i18n.language, e)); }
    setNasBusy(null);
  };
  const [nasBusy, setNasBusy] = useState<string | null>(null);
  const [nasMsg, setNasMsg] = useState<{ id: string; text: string } | null>(null);
  /** 等着二次确认移除的那台 NAS。 */
  const [nasRemoving, setNasRemoving] = useState<Share | null>(null);
  const nasDo = async (sh: Share, f: () => Promise<void>) => {
    setNasBusy(sh.id); setNasMsg(null);
    try { await f(); } catch (e) { setNasMsg({ id: sh.id, text: errText(t, i18n.language, e) }); }
    setNasBusy(null);
  };
  /** 没连上的那台为什么没连上：密码不对（去"编辑…"里改）、没权限、连不上；说不清就是"未连接"。 */
  const nasWhy = (sh: Share) => {
    const k = errKind(sh.error);
    return k === "auth" ? t("settings.nasWhyAuth") : k === "denied" ? t("settings.nasWhyDenied") : k === "net" ? t("settings.nasWhyNet") : t("settings.nasOffline");
  };
  const nasSubmit = async () => {
    setNasBusy("new"); setNasErr(null);
    try {
      if (nasEditing) await updateShare({ id: nasEditing.id, name: nasForm.name, host: nasForm.host, user: nasForm.user, password: nasForm.password, remember: nasForm.remember });
      else await addShare({ name: nasForm.name, host: nasForm.host, user: nasForm.user, password: nasForm.password, remember: nasForm.remember });
      setNasOpen(false); setNasForm(blankNas); setNasShares(null); setNasEditing(null);
    } catch (e) { setNasErr(errText(t, i18n.language, e)); }
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
    // 手机和电脑可能备份到同一个桶：先说清楚这份是谁、什么时候做的，再问要不要恢复
    let from = "";
    try { const m = await peekBackup(tg.id); if (m) from = " " + t("settings.backupFrom", { when: bkWhen(m.exported_at), device: m.device || m.platform }); } catch { /* 没有说明也能恢复 */ }
    if (!window.confirm(t("settings.backupRestoreConfirm", { name: tg.name }) + from)) return;
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
  // 诊断信息：版本、系统、去掉密码与令牌的设置、最近的日志和崩溃记录，写成一个本地文本文件，发不发由用户自己决定
  const [diagBusy, setDiagBusy] = useState(false);
  const [diagMsg, setDiagMsg] = useState<{ text: string; path?: string } | null>(null);
  const exportDiagnostics = async () => {
    const stamp = new Date().toISOString().slice(0, 10);
    const out = await save({ defaultPath: `fleetinggleam-diagnostics-${stamp}.txt`, filters: [{ name: "Text", extensions: ["txt"] }] });
    if (!out) return;
    setDiagBusy(true); setDiagMsg(null);
    try { const path = await invoke<string>("diagnostics_export", { out }); setDiagMsg({ text: t("settings.diagDone", { path: path || out }), path: path || out }); }
    catch (e) { setDiagMsg({ text: t("settings.diagFailed", { error: errText(t, i18n.language, e) }) }); }
    setDiagBusy(false);
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
      <Modal open={resetAsk} onClose={() => setResetAsk(false)} title={t("settings.learnResetTitle")}
        footer={(<><button className="btn" onClick={() => setResetAsk(false)}>{t("organize.cancel")}</button>
          <button className="btn danger" autoFocus onClick={resetPrefs}>{t("settings.learnResetAction")}</button></>)}>
        <p>{t("settings.learnResetBody", { labels: prefs?.labels ?? 0, refs: prefs?.refs ?? 0 })}</p>
      </Modal>
      <Modal open={!!mirrorAsk} onClose={() => setMirrorAsk(null)} title={t("settings.mirrorClearTitle", { name: mirrorAsk?.name ?? "" })}
        footer={(<><button className="btn" onClick={() => setMirrorAsk(null)}>{t("organize.cancel")}</button>
          <button className="btn primary" autoFocus onClick={() => mirrorAsk && clearMirror(mirrorAsk)}>{t("settings.mirrorClearAction", { size: bytes(mirrorAsk?.bytes ?? 0) })}</button></>)}>
        <p>{t("settings.mirrorClearBody")}</p>
      </Modal>
      <Modal open={proxyOpen} onClose={() => setProxyOpen(false)} title={t("settings.proxy")}
        footer={(<>{proxy?.manual && <button className="btn" onClick={() => proxySave(null)}>{t("settings.proxyUseSystem")}</button>}
          <button className="btn" onClick={() => setProxyOpen(false)}>{t("settings.nasCancel")}</button>
          <button className="btn primary" onClick={() => proxySave(proxyText.trim() || null)}>{t("settings.proxySave")}</button></>)}>
        <div className="nas-form">
          <p className="ash small">{t("settings.proxyHow")}</p>
          <label><span>{t("settings.proxyField")}</span><input className="title-input wide" value={proxyText} placeholder={proxy?.system ?? t("settings.proxyHint")} spellCheck={false} autoFocus
            onChange={(e) => { setProxyText(e.target.value); setProxyErr(null); }} onKeyDown={(e) => { if (e.key === "Enter") void proxySave(proxyText.trim() || null); }} /></label>
          {proxyErr && <p className="error">{proxyErr}</p>}
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
          <motion.div className="settings-body" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.32, ease: [0.16, 1, 0.3, 1] }}>
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
              {/* 模型档位四选一，和 Android 一样（M9.2）：没装的选了就下载；换了之后正在看的项目重新评估 */}
              <div className="srow-wrap"><div className="srow"><div className="stext"><b>{t("settings.tierTitle")}</b></div></div></div>
              {([null, "fast", "high", "ultra"] as const).map((tier) => {
                const pack = tier === null ? null : packs.find((p) => p.id === (tier === "high" ? "high" : tier === "ultra" ? "ultra" : "standard"));
                const installed = tier === null || !!pack?.installed;
                const title = tier === null ? t("settings.tierAuto") : t(tier === "ultra" ? "settings.qualityUltra" : tier === "high" ? "settings.qualityHigh" : "settings.qualityFast");
                const desc = tier === null ? t("settings.tierAutoDesc") : `${t(`settings.tierDesc.${tier}`)} · ${installed ? t("settings.tierInstalled") : t("settings.tierNeeds")}`;
                const on = s.tier === tier;
                return (
                  <button key={String(tier)} className="srow srow-pick" role="radio" aria-checked={on} onClick={async () => {
                    if (tier !== null && (await ensurePacks(tierPacks(tier))) === "cancelled") return;
                    setSettings({ tier, tierRev: s.tierRev + 1 });
                  }}>
                    <div className="stext"><b>{title}</b><p className="ash">{desc}</p></div>
                    <i className={`radio ${on ? "on" : ""}`} aria-hidden />
                  </button>
                );
              })}
              <p className="ash small srow-hint">{t("settings.tierHint")}</p>
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
                  <button className="btn" onClick={() => setResetAsk(true)}>{prefsReset ? t("settings.learnResetDone") : t("settings.learnReset")}</button>
                </Row>
              )}
            </Group>

            <Group id="sec-data" title={t("settings.data")} index={3}>
              <div className="srow-wrap">
                <div className="srow"><div className="stext"><b>{t("models.title")}</b><p className="ash">{t("models.titleDesc")}</p></div></div>
              </div>
              {packs.map((p) => (
                <Row key={p.id} label={t(`models.name.${p.id}`)} desc={`${p.bundled ? t("models.bundled") : p.installed ? t("models.have") : t("models.none")} · ${t(`models.desc.${p.id}`)}`}>
                  {p.bundled ? null : downloading.includes(p.id)
                    ? <button className="btn mono" onClick={showGate}>{gatePct}%</button>
                    : p.installed
                    ? <button className="btn" onClick={async () => { await removePack(p.id); release(p.id); }}>{t("models.remove")}</button>
                    : <button className="btn" onClick={async () => { if ((await ensurePack(p.id)) !== "cancelled") adopt(p.id); }}>{t("models.get")}</button>}
                </Row>
              ))}
              {proxy && (
                <Row label={t("settings.proxy")} desc={proxyDesc}>
                  <button className="btn" onClick={() => { setProxyText(proxy.manual ?? ""); setProxyErr(null); setProxyOpen(true); }}>{t("settings.proxyEdit")}</button>
                </Row>
              )}
              <Row label={t("settings.cache")} desc={t("settings.cacheDesc")}>
                <span className="mono ash cachesize">{cache === null ? "—" : bytes(cache)}</span>
                <button className="btn" onClick={clear} disabled={!cache}>{cleared ? t("settings.cleared") : t("settings.clear")}</button>
              </Row>
              {mirrors.length > 0 && (
                <div className="srow-wrap"><div className="srow"><div className="stext"><b>{t("settings.mirror")}</b><p className="ash">{mirrorMsg ?? t("settings.mirrorDesc")}</p></div></div></div>
              )}
              {mirrors.map((m) => (
                <Row key={m.id} label={m.name}>
                  <span className="mono ash cachesize">{m.bytes ? bytes(m.bytes) : "—"}</span>
                  <button className="btn" disabled={!m.bytes} onClick={() => setMirrorAsk(m)}>{t("settings.mirrorClear")}</button>
                </Row>
              ))}
              <Row label={t("settings.privacy")} desc={t("settings.privacyBody")} />
            </Group>

            <Group id="sec-backup" title={t("settings.backup")} index={4}>
              <Row label={t("settings.exportTaste")} desc={tasteMsg ?? t("settings.exportTasteDesc")} info={<p className="ash">{t("settings.exportTasteInfo")}</p>}>
                <span className="sctl-pair">
                  <button className="btn" disabled={tasteBusy} onClick={exportTaste}>{t("settings.exportTasteBtn")}</button>
                  <button className="btn" disabled={tasteBusy} onClick={importTaste}>{t("settings.importTasteBtn")}</button>
                </span>
              </Row>
              {/* 先添加备份方式；自动备份、保留份数在有目标之后才出现（M9.3）。状态点写在说明前，编辑 / 移除收进 ⋯（M9.6） */}
              <Row label={t("settings.backupTargets")} desc={t("settings.backupTargetsDesc")}>
                <button className="btn" disabled={bkBusy !== null} onClick={bkAdd}>{t("settings.backupAddTarget")}</button>
              </Row>
              {(backup?.targets ?? []).map((tg) => (
                <Row key={tg.id} label={tg.name} desc={`${tg.last_error ? "○" : "●"} ${tg.last_error ? t("settings.backupErrored") : tg.last_ok ? t("settings.backupLast", { when: bkWhen(tg.last_ok) }) : t("settings.backupReady")} · Cloudflare R2 · ${tg.bucket}`}>
                  <RowMenu label={t("more.open")}>
                    <button role="menuitem" disabled={bkBusy !== null} onClick={() => bkEdit(tg)}><i aria-hidden />{t("settings.backupEdit")}</button>
                    <button role="menuitem" disabled={bkBusy !== null} onClick={() => setBkRemoving(tg)}><i aria-hidden />{t("settings.backupRemove")}</button>
                  </RowMenu>
                </Row>
              ))}
              {backup?.configured && (
                <Row label={t("settings.backupTitle")} desc={bkMsg ?? bkSummary()} info={<p className="ash">{t("settings.backupInfo")}</p>}>
                  <span className="sctl-pair">
                    <button className="btn" disabled={bkBusy !== null} onClick={bkNow}>{bkBusy === "now" ? t("settings.backupRunning") : t("settings.backupNow")}</button>
                    <button className="btn" disabled={bkBusy !== null} onClick={bkRestore}>{bkBusy === "restore" ? t("settings.backupRestoring") : t("settings.backupRestore")}</button>
                    <button className="switch" role="switch" aria-checked={backup.enabled} aria-label={t("settings.backupAuto")} title={t("settings.backupAuto")} disabled={bkBusy !== null} onClick={bkToggle} />
                  </span>
                </Row>
              )}
              {backup?.configured && (
                <Row label={t("settings.backupKeep")} desc={t("settings.backupKeepHint")}>
                  <Stepper value={backup.keep} min={1} max={1000} label={t("settings.backupKeep")} onChange={bkKeep} />
                </Row>
              )}
            </Group>

            <Group id="sec-nas" title={t("settings.nas")} index={5}>
              <Row label={t("settings.nasTitle")} desc={t("settings.nasDesc")} info={<p className="ash">{t("settings.nasInfo")}</p>}>
                <button className="btn" onClick={() => { setNasErr(null); setNasShares(null); setNasEditing(null); setNasForm(blankNas); setNasOpen(true); }}>{t("settings.nasAdd")}</button>
              </Row>
              {shares.map((sh) => (
                <Row key={sh.id} label={sh.name} desc={`${sh.enabled && sh.connected ? "●" : "○"} ${!sh.enabled ? t("settings.nasDisabled") : sh.connected ? t("settings.nasConnected") : nasBusy === sh.id ? t("settings.nasConnecting") : nasWhy(sh)} · ${sh.host} · ${sh.user || t("settings.nasGuest")}${nasMsg?.id === sh.id ? ` · ${nasMsg.text}` : ""}`}>
                  <span className="sctl-pair">
                    <button className="switch" role="switch" aria-checked={sh.enabled} aria-label={t("settings.nasEnabled")} title={t("settings.nasEnabled")} disabled={nasBusy === sh.id} onClick={() => nasDo(sh, () => setShareEnabled(sh.id, !sh.enabled))} />
                    <RowMenu label={t("more.open")}>
                      <button role="menuitem" disabled={nasBusy === sh.id} onClick={() => nasEdit(sh)}><i aria-hidden />{t("settings.nasEdit")}</button>
                      <button role="menuitem" disabled={nasBusy === sh.id} onClick={() => setNasRemoving(sh)}><i aria-hidden />{t("settings.nasRemove")}</button>
                    </RowMenu>
                  </span>
                </Row>
              ))}
            </Group>

            <Group id="sec-about" title={t("settings.about")} index={6}>
              {/* 应用介绍 + 链接（官网、源代码、隐私、条款、反馈、第三方许可）+ 版本；链接在系统浏览器里打开 */}
              <div className="srow about-head">
                <img className="about-icon" src={appIcon} alt="" draggable={false} />
                <div className="stext">
                  <b>{t("brand.name")}{i18n.language !== "en" && <span className="mono ash about-latin">{t("brand.latin")}</span>}</b>
                  <p className="ash">{t("brand.tagline")}</p>
                  <p className="about-intro">{t("settings.aboutIntro")}</p>
                </div>
              </div>
              {(() => {
                const l = links(i18n.language.startsWith("zh"));
                const rows: [string, string | undefined, string][] = [
                  [t("settings.aboutSite"), t("settings.aboutSiteDesc"), l.site],
                  [t("settings.aboutSource"), t("settings.aboutSourceDesc"), l.source],
                  [t("settings.aboutPrivacy"), undefined, l.privacy],
                  [t("settings.aboutTerms"), undefined, l.terms],
                  [t("settings.aboutIssues"), t("settings.aboutIssuesDesc"), l.issues],
                ];
                return rows.map(([label, desc, url]) => (
                  <button key={url} className="srow about-link" onClick={() => { void openUrl(url).catch(() => window.open(url, "_blank")); }} title={url}>
                    <div className="stext"><b>{label}</b>{desc && <p className="ash">{desc}</p>}</div>
                    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d="M6 3.5l4.5 4.5L6 12.5" /></svg>
                  </button>
                ));
              })()}
              <Row label={t("settings.aboutVersion")}>
                <span className="mono ash">{version ? `v${version} (${__BUILD__})` : ""}</span>
              </Row>
              <Row label={t("settings.diag")} desc={diagMsg?.text ?? t("settings.diagDesc")} info={<p className="ash">{t("settings.diagInfo")}</p>}>
                <span className="sctl-pair">
                  {diagMsg?.path && <button className="btn" onClick={() => invoke("reveal", { path: diagMsg.path }).catch(() => {})}>{t("organize.showInFinder")}</button>}
                  <button className="btn" disabled={diagBusy} onClick={exportDiagnostics}>{diagBusy ? t("settings.diagBusy") : t("settings.diagBtn")}</button>
                </span>
              </Row>
            </Group>
          </motion.div>
          </div>
        </div>
      </section>
    </div>
  );
}
