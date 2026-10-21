import { useEffect, useState } from "react";

/** 左右带 − / + 的数字框：中间可以直接输入，失焦或回车时提交；超出范围会被夹回 min / max。 */
export default function Stepper({ value, min = 1, max = 1000, step = 1, disabled = false, label, format, onChange }: {
  value: number; min?: number; max?: number; step?: number; disabled?: boolean;
  /** 无障碍名称。 */
  label: string;
  /** 显示用的格式（例如加单位）；编辑时显示纯数字。 */
  format?: (n: number) => string;
  onChange: (n: number) => void;
}) {
  const clamp = (n: number) => Math.min(max, Math.max(min, Math.round(n)));
  const [text, setText] = useState(String(value));
  const [editing, setEditing] = useState(false);
  useEffect(() => { if (!editing) setText(String(value)); }, [value, editing]);
  const commit = () => {
    setEditing(false);
    const n = Number(text);
    if (Number.isFinite(n) && clamp(n) !== value) onChange(clamp(n)); else setText(String(value));
  };
  const nudge = (d: number) => { const n = clamp(value + d); if (n !== value) onChange(n); };
  return (
    <span className={`stepper ${disabled ? "off" : ""}`} role="group" aria-label={label}>
      <button type="button" className="stepper-btn" disabled={disabled || value <= min} aria-label="−" onClick={() => nudge(-step)}>
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round"><path d="M2.5 6h7" /></svg>
      </button>
      <input className="stepper-input" inputMode="numeric" value={editing ? text : (format ? format(value) : String(value))} disabled={disabled} aria-label={label}
        onFocus={(e) => { setEditing(true); setText(String(value)); requestAnimationFrame(() => e.target.select()); }}
        onChange={(e) => setText(e.target.value.replace(/[^\d]/g, ""))}
        onBlur={commit}
        onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); if (e.key === "ArrowUp") { e.preventDefault(); nudge(step); } if (e.key === "ArrowDown") { e.preventDefault(); nudge(-step); } }} />
      <button type="button" className="stepper-btn" disabled={disabled || value >= max} aria-label="+" onClick={() => nudge(step)}>
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round"><path d="M2.5 6h7M6 2.5v7" /></svg>
      </button>
    </span>
  );
}
