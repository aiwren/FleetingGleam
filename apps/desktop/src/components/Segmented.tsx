import { motion } from "motion/react";
import { SPRING } from "../motion";

/** Segmented control with a gliding pill. `id` keeps pills of different groups from sharing a layout. */
export default function Segmented<T extends string>({ id, value, options, onChange, block = false, label }: {
  id: string;
  value: T;
  options: { value: T; label: string; disabled?: boolean; tip?: string }[];
  onChange: (v: T) => void;
  block?: boolean;
  label?: string;
}) {
  return (
    <div className={`segmented ${block ? "block" : ""}`} role="group" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} aria-pressed={value === o.value} disabled={o.disabled} data-tip={o.disabled ? o.tip : undefined} onClick={() => onChange(o.value)}>
          {value === o.value && <motion.i layoutId={`pill-${id}`} className="pill" transition={SPRING.glide} />}
          <span className="seg-label">{o.label}</span>
        </button>
      ))}
    </div>
  );
}
