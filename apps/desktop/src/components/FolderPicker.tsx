import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import Modal from "./Modal";
import { fmtBytes } from "../format";
import { Listing, Share, answerPicker, browseShare, connectShare, usePickRequest, useShares } from "../nas";
import { pickLocalFolder } from "../projectActions";

const icon = (d: string) => <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>;
const FOLDER = "M1.5 4.5A1.5 1.5 0 013 3h3.2l1.6 1.6H13A1.5 1.5 0 0114.5 6v6A1.5 1.5 0 0113 13.5H3A1.5 1.5 0 011.5 12z";
const SHARE = "M1.5 5A1.5 1.5 0 013 3.5h10A1.5 1.5 0 0114.5 5v6A1.5 1.5 0 0113 12.5H3A1.5 1.5 0 011.5 11zM1.5 8h13M4.5 10.2h.01M4.5 5.8h.01";
const DISK = "M2.5 11.5h11a1 1 0 001-1V8.6a1 1 0 00-.1-.4l-1.9-4A1 1 0 0011.6 3.5H4.4a1 1 0 00-.9.6l-1.9 4a1 1 0 00-.1.4v1.9a1 1 0 001 1zM2.5 8h11M11 9.8h.01";
const NAS = "M3 2.5h10a1 1 0 011 1v9a1 1 0 01-1 1H3a1 1 0 01-1-1v-9a1 1 0 011-1zM2 6.3h12M2 9.7h12M5 4.4h.01M5 8h.01M5 11.6h.01";
const CHEVRON = "M6 3.5l4.5 4.5L6 12.5";

/**
 * 问"项目文件夹在哪"：本机，还是某个 NAS 共享里的文件夹。挂在 App 里，谁调 `pickFolder()` 它就出现。
 * 选了共享就能逐级进入它的文件夹，看到每一级里有多少张照片，选中的文件夹以镜像里的本地路径回答。
 */
export default function FolderPicker() {
  const { t, i18n } = useTranslation();
  const req = usePickRequest();
  const shares = useShares();
  const [share, setShare] = useState<Share | null>(null);
  const [listing, setListing] = useState<Listing | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  // 每次请求从头开始；设置页"从这里新建项目"直接进到那个共享
  useEffect(() => {
    setShare(null); setListing(null); setErr(null);
    if (req?.shareId) {
      const s = shares.find((x) => x.id === req.shareId);
      if (s) void enter(s, "");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [req]);

  const enter = async (s: Share, path: string) => {
    setShare(s); setBusy(true); setErr(null);
    try {
      if (!s.connected) await connectShare(s.id);
      setListing(await browseShare(s.id, path));
    } catch (e) {
      setErr(String(e));
      if (!listing) setListing({ path, local: "", entries: [] });
    }
    setBusy(false);
  };
  const local = async () => {
    const dir = await pickLocalFolder();
    if (dir) answerPicker(dir);
  };
  const crumbs = listing ? listing.path.split("/").filter(Boolean) : [];
  const atRoot = crumbs.length === 0;
  const up = (n: number) => share && enter(share, crumbs.slice(0, n).join("/"));
  const photos = listing ? listing.entries.filter((e) => !e.dir) : [];
  const folders = listing ? listing.entries.filter((e) => e.dir) : [];
  const bytes = photos.reduce((a, e) => a + e.size, 0);

  return (
    <Modal open={!!req} onClose={() => answerPicker(null)} title={share ? share.name : (req?.opts?.title ?? t("picker.title"))} wide
      footer={(<>
        {share && <button className="btn" onClick={() => { setShare(null); setListing(null); setErr(null); }}>{t("picker.back")}</button>}
        <button className="btn" onClick={() => answerPicker(null)}>{t("picker.cancel")}</button>
        {share && listing && <button className="btn primary" disabled={busy || !listing.local} onClick={() => answerPicker(listing.local)}>{req?.opts?.choose ?? t("picker.choose")}</button>}
      </>)}>
      {!share && (
        <div className="picker-sources">
          {!req?.opts?.nasOnly && (
            <button className="picker-src" onClick={local}>
              {icon(DISK)}<span><b>{t("picker.local")}</b><small>{t("picker.localDesc")}</small></span>
            </button>
          )}
          {shares.filter((s) => s.enabled).map((s) => (
            <button key={s.id} className="picker-src" onClick={() => enter(s, "")}>
              {icon(NAS)}<span><b>{s.name}</b><small>{s.host}{s.connected ? "" : ` · ${t("settings.nasOffline")}`}</small></span>
            </button>
          ))}
        </div>
      )}
      {share && (
        <div className="picker-browse">
          <div className="picker-crumbs">
            <button className={atRoot ? "cur" : ""} onClick={() => up(0)}>{share.name}</button>
            {crumbs.map((c, i) => (<span key={i}>/<button className={i === crumbs.length - 1 ? "cur" : ""} onClick={() => up(i + 1)}>{c}</button></span>))}
          </div>
          {err && <p className="error">{t("settings.nasFailed", { error: err })}</p>}
          {busy && <p className="ash small">{t("picker.loading")}</p>}
          {!busy && listing && (
            <>
              <ul className="picker-list">
                {folders.map((e) => (
                  <li key={e.name}><button onClick={() => enter(share, listing.path ? `${listing.path}/${e.name}` : e.name)}>{icon(atRoot ? SHARE : FOLDER)}<span>{e.name}</span><i className="picker-go">{icon(CHEVRON)}</i></button></li>
                ))}
                {folders.length === 0 && <li className="ash small pad">{t(atRoot ? "picker.noShares" : "picker.noFolders")}</li>}
              </ul>
              <p className="ash small">{atRoot ? (req?.opts?.hint ?? t("picker.rootHint")) : photos.length > 0 ? t("picker.photosHere", { count: photos.length, size: fmtBytes(i18n.language, bytes) }) : t("picker.noPhotosHere")}</p>
            </>
          )}
        </div>
      )}
    </Modal>
  );
}
