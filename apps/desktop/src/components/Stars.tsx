import { useTranslation } from "react-i18next";

/**
 * 手动星级：五颗可点的星。点第 n 颗打 n 星，再点同一颗取消；`value` 是现在的星数（0 / undefined = 没打）。
 * 键盘上对应数字键 1–5（0 清除）。星是普通的五角星，和应用自己的四角"惊鸿"标记（保留 / 待复查）区分开。
 */
export default function Stars({ value = 0, onChange, size = 14, dark = false }: { value?: number; onChange: (n: number | null) => void; size?: number; dark?: boolean }) {
  const { t } = useTranslation();
  return (
    <span className={`stars ${dark ? "dark" : ""}`} role="radiogroup" aria-label={t("stars.label")}>
      {[1, 2, 3, 4, 5].map((n) => (
        <button key={n} type="button" role="radio" aria-checked={value === n} className={`star-btn ${n <= value ? "on" : ""}`}
          data-tip={`${value === n ? t("stars.clear") : t("stars.set", { count: n })} · ${n}`}
          onClick={(e) => { e.stopPropagation(); onChange(value === n ? null : n); }}>
          <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
            <path d="M12 2.6l2.9 6.1 6.7.8-4.9 4.6 1.3 6.6L12 17.4l-6 3.3 1.3-6.6L2.4 9.5l6.7-.8z" fill={n <= value ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
          </svg>
        </button>
      ))}
    </span>
  );
}
