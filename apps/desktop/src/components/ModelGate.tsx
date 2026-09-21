import { useTranslation } from "react-i18next";
import { cancelGate, startDownload, useGate } from "../models";
import Modal from "./Modal";

const mb = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.max(1, Math.round(b / 1e6))} MB`);

/** The one dialog for downloading models: says what and how big, shows progress, can be cancelled at any moment. */
export default function ModelGate() {
  const { t } = useTranslation();
  const g = useGate();
  const pct = g && g.total ? Math.min(100, Math.round((g.done / g.total) * 100)) : 0;
  return (
    <Modal open={!!g} onClose={cancelGate} locked={g?.phase === "downloading"} title={g ? t(`models.ask.${g.id}`) : ""}
      footer={g && (
        <>
          <button className="btn" onClick={cancelGate}>{t("organize.cancel")}</button>
          {g.phase === "ask" && <button className="btn primary" autoFocus onClick={startDownload}>{t("models.download", { size: mb(g.bytes) })}</button>}
          {g.phase === "error" && <button className="btn primary" onClick={startDownload}>{t("models.retry")}</button>}
        </>
      )}>
      {g?.phase === "ask" && <p>{t(`models.body.${g.id}`, { size: mb(g.bytes) })}</p>}
      {g?.phase === "downloading" && (
        <div className="mprog">
          <div className="mbar2"><i style={{ width: `${pct}%` }} /></div>
          <p className="mono ash">{mb(g.done)} / {mb(g.total)} · {pct}%</p>
        </div>
      )}
      {g?.phase === "error" && <p className="ash">{t("models.error")} <span className="mono">{g.error}</span></p>}
    </Modal>
  );
}
