import { ReactNode } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { SPRING } from "../motion";
import { Decision, Photo, Reason, State, pct } from "../types";
import Ticker from "./Ticker";
import { Star } from "./Tile";

const STATES: State[] = ["keep", "review", "reject"];

function Bar({ label, value }: { label: string; value: number }) {
  return (
    <div className="mbar">
      <span>{label}</span>
      <i><motion.b initial={{ width: 0 }} animate={{ width: `${Math.max(2, value)}%` }} transition={{ ...SPRING.soft }} /></i>
      <em className="mono">{value}</em>
    </div>
  );
}

/**
 * The verdict for one photo, in one compact block: the score, the three marks, why, and the numbers behind it as bars.
 * It lives in the inspector and is the only place these are shown.
 */
export default function PhotoVerdict({ photo, decision, onDecide, reasonText, scoreTip }: {
  photo: Photo; decision?: Decision; onDecide: (s: State | null) => void; reasonText: (r: Reason) => string; scoreTip?: ReactNode;
}) {
  const { t } = useTranslation();
  const state = decision?.state ?? "review";
  const reasons = decision?.reasons.map((r) => reasonText(r)).join("；");
  return (
    <>
      <h3 className="mono vname">{photo.name}{photo.kind && <span className="ktag">{t(`kind.${photo.kind}`)}</span>}</h3>
      <div className="sscore mono"><Ticker value={pct(photo.score)} /><small>{t("inspector.overall")}</small>{scoreTip}</div>
      <div className="strio">
        {STATES.map((st, i) => (
          <button key={st} className={`sbtn ${st} ${state === st ? "on" : ""}`} onClick={() => onDecide(st)} data-tip={`${t(`state.${st}`)} · ${["P", "R", "X"][i]}`}>
            {st === "keep" && <Star filled size={13} />}{st === "review" && <Star filled={false} size={13} />}{st === "reject" && <span aria-hidden>×</span>}
            {t(`state.${st}`)}
          </button>
        ))}
      </div>
      {reasons && <p className="sreasons">{reasons}</p>}
      <div className="mbars">
        <Bar label={t("inspector.sharp")} value={pct(photo.sharp_score)} />
        <Bar label={t("inspector.expo")} value={pct(photo.expo_score)} />
        {photo.ai_score !== null && <Bar label={t("inspector.ai")} value={pct(photo.ai_score)} />}
        {photo.pref_score !== null && <Bar label={t("inspector.pref")} value={pct(photo.pref_score)} />}
      </div>
      {photo.face_count > 0 && (
        <p className="sreasons">{t("inspector.faces")} · {t("inspector.faceValue", { count: photo.face_count, q: photo.face_min === null ? "—" : photo.face_min.toFixed(2) })}</p>
      )}
    </>
  );
}
