import { useTranslation } from "react-i18next";
import { showGate, useGate } from "../models";

/**
 * 转到后台的模型下载：侧栏里一行，圆环是进度，点一下把对话框叫回来。收起成图标栏时只剩圆环。
 * 进度用墨色，不用琥珀（琥珀只表示"保留"）。
 */
export default function DownloadNav() {
  const { t } = useTranslation();
  const g = useGate();
  if (!g?.hidden) return null;
  const f = g.total ? Math.min(1, g.done / g.total) : 0;
  const pct = Math.round(f * 100);
  const r = 6, c = 2 * Math.PI * r;
  const label = t("models.backgroundNav", { pct });
  return (
    <ul className="modules nav-dl">
      <li>
        <button onClick={showGate} title={label} aria-label={label}>
          <span className="nav-inner">
            <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden>
              <circle cx="8" cy="8" r={r} fill="none" stroke="var(--line)" strokeWidth="2" />
              <circle cx="8" cy="8" r={r} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"
                strokeDasharray={`${c * f} ${c}`} transform="rotate(-90 8 8)" style={{ transition: "stroke-dasharray .16s linear" }} />
            </svg>
            <span className="nav-text">{label}</span>
          </span>
        </button>
      </li>
    </ul>
  );
}
