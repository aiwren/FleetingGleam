import { useTranslation } from "react-i18next";
import { cancelGate, hideGate, startDownload, useGate } from "../models";
import Modal from "./Modal";

import { fmtBytes } from "../format";

/**
 * The one dialog for downloading models: says what and how big, shows progress, can be cancelled at any moment.
 * 下载中可以"后台下载"（和 Android 一样）：对话框收起，应用照常可用，侧栏显示进度；按 Esc 或点空白处也是转到后台，不是取消。
 */
export default function ModelGate() {
  const { t, i18n } = useTranslation();
  const g = useGate();
  const pct = g && g.total ? Math.min(100, Math.round((g.done / g.total) * 100)) : 0;
  return (
    <Modal open={!!g && !g.hidden} onClose={g?.phase === "downloading" ? hideGate : cancelGate} title={g ? t(`models.ask.${g.id}`) : ""}
      footer={g && (
        <>
          <button className="btn" onClick={cancelGate}>{t("organize.cancel")}</button>
          {g.phase === "downloading" && <button className="btn primary" autoFocus onClick={hideGate}>{t("models.background")}</button>}
          {g.phase === "ask" && <button className="btn primary" autoFocus onClick={startDownload}>{t("models.download", { size: fmtBytes(i18n.language, g.bytes) })}</button>}
          {g.phase === "error" && <button className="btn primary" onClick={startDownload}>{t("models.retry")}</button>}
        </>
      )}>
      {g?.phase === "ask" && <p>{t(`models.body.${g.id}`, { size: fmtBytes(i18n.language, g.bytes) })}</p>}
      {g?.phase === "ask" && g.ids && (
        <ul className="mlist">
          {g.ids.map((id, i) => <li key={id}><span>{t(`models.name.${id}`)}</span><span className="mono ash">{fmtBytes(i18n.language, g.sizes?.[i] ?? 0)}</span></li>)}
        </ul>
      )}
      {g?.phase === "downloading" && (
        <div className="mprog">
          <div className="mbar2"><i style={{ width: `${pct}%` }} /></div>
          <p className="mono ash">{fmtBytes(i18n.language, g.done)} / {fmtBytes(i18n.language, g.total)} · {pct}%</p>
        </div>
      )}
      {g?.phase === "error" && <p className="ash">{t("models.error")} <span className="mono">{g.error}</span></p>}
    </Modal>
  );
}
