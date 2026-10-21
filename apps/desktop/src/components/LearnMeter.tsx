import { useTranslation } from "react-i18next";
import { PrefsInfo } from "../styles";

/**
 * How far a learned taste is from being used, and from its full say: a track with three marks (starts working, half strength,
 * full strength) and one line that says what is still missing. Reference photos count as keeps, like they do in training.
 */
export default function LearnMeter({ info, compact }: { info: PrefsInfo; compact?: boolean }) {
  const { t } = useTranslation();
  const min = info.min_per_class || 3;
  const full = info.full_at || 81, half = info.half_at || 29, start = 2 * min;
  // what will be trained on; once in use the backend's own count (which includes the sampled "not chosen" examples) is the truth
  const have = Math.max(info.trained || 0, info.labels + info.refs);
  const keeps = info.keeps + info.refs, rejects = info.rejects ?? info.labels - info.keeps;
  const needKeep = Math.max(0, min - keeps), needReject = Math.max(0, min - rejects);
  const status = info.active
    ? `${t("meter.active", { pct: Math.round(info.strength * 100), max: Math.round(info.max_strength * 100) })}${have < full ? ` · ${t("meter.toFull", { n: full - have })}` : ""}`
    : needKeep && needReject ? t("meter.needBoth", { keep: needKeep, reject: needReject, min })
    : needKeep ? t("meter.needKeep", { keep: needKeep, min })
    : needReject ? t("meter.needReject", { reject: needReject, min })
    : t("meter.pending");
  const marks = [[start, t("meter.markStart"), "pos-left"], [half, t("meter.markHalf"), "pos-mid"], [full, t("meter.markFull"), "pos-right"]] as const;
  return (
    <div className={`learn-meter${compact ? " compact" : ""}`}>
      <div className="lm-track" role="img" aria-label={t("meter.aria", { have, full })}>
        <i className="lm-fill" style={{ width: `${Math.min(100, (have / full) * 100)}%` }} />
        {marks.map(([at, label, pos]) => (
          <span key={at} className={`lm-mark ${pos}${have >= at ? " on" : ""}`} style={{ left: `${(at / full) * 100}%` }}>
            <small>{at} · {label}</small>
          </span>
        ))}
      </div>
      <p className="ash small lm-status">{status}</p>
    </div>
  );
}
