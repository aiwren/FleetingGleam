import { ReactNode } from "react";
import { TipBody, useTip } from "./InfoTip";

/**
 * One row of a collapsible card: the whole row is the button, a short summary on the right says what is inside without opening it,
 * the chevron turns, and the content unfolds smoothly. Rows are separated by hairlines (see `.accs` in App.css).
 */
const ICONS = {
  chart: "M2.5 12.5V8.5M6.5 12.5V4M10.5 12.5V6.5M14 12.5H1.5",
  file: "M4 1.8h5l3 3V13a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2.8a1 1 0 0 1 1-1zM9 1.8V5h3",
};

export default function Accordion({ title, hint, icon, children }: { title: string; hint?: string; icon?: keyof typeof ICONS; children: ReactNode }) {
  const tip = useTip();
  return (
    <div className={`acc ${tip.open ? "open" : ""}`}>
      <button className="acc-head" onClick={tip.toggle} aria-expanded={tip.open}>
        {icon && <svg className="acc-ico" width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d={ICONS[icon]} /></svg>}
        <span className="acc-title">{title}</span>
        {hint && !tip.open && <span className="acc-hint mono">{hint}</span>}
        <svg className="acc-chev" width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d="M3 2.5L6.5 5 3 7.5" /></svg>
      </button>
      <TipBody open={tip.open}><div className="acc-body">{children}</div></TipBody>
    </div>
  );
}
