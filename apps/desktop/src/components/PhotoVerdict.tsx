import { ReactNode } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { SPRING } from "../motion";
import { Decision, Photo, Reason, State, pct } from "../types";
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
export default function PhotoVerdict({ photo, decision, onDecide, reasonText, scoreTip, onZoom }: {
  photo: Photo; decision?: Decision; onDecide: (s: State | null) => void; reasonText: (r: Reason) => string; scoreTip?: ReactNode;
  /** 名字行右边的"放大"（检查器里没有预览图了，这是打开大图的入口）。 */
  onZoom?: () => void;
}) {
  const { t, i18n } = useTranslation();
  const state = decision?.state ?? "review";
  // sentences joined the way the language joins them (a full-width semicolon in Chinese, a plain one elsewhere)
  const reasons = decision?.reasons.map((r) => reasonText(r)).join(i18n.language.startsWith("zh") ? "；" : "; ");
  return (
    <>
      <div className="vname-row">
        <h3 className="mono vname">{photo.name}{photo.kind && <span className="ktag">{t(`kind.${photo.kind}`)}</span>}</h3>
        {onZoom && (
          <button className="btn icon plain sm" onClick={onZoom} aria-label={t("inspector.zoomLabel")} title={t("inspector.zoomHint")}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9" /></svg>
          </button>
        )}
      </div>
      {/* 综合分是数字，不画星：四角星只表示保留 / 待复查（R2） */}
      <div className="sscore" title={`${t("inspector.overall")} ${pct(photo.score)}`}>
        <span className="snum mono">{pct(photo.score)}</span>
        <span className="ash small">{t("inspector.overall")}</span>
        {scoreTip}
      </div>
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
    </>
  );
}
