/**
 * The app's own mark: the four-point gleam from the logo, filled with the same gold. In the app a kept photo wears this and a
 * rejected one wears nothing at all, so that is what the page does too.
 */
const STAR = "M0,-10 Q1.7,-1.7 10,0 Q1.7,1.7 0,10 Q-1.7,1.7 -10,0 Q-1.7,-1.7 0,-10Z";

export function GleamDefs() {
  return (
    <svg width="0" height="0" aria-hidden className="absolute">
      <defs>
        <linearGradient id="fg-gold" x1=".2" y1="0" x2=".8" y2="1">
          <stop offset="0" stopColor="#FFF3C4" /><stop offset=".55" stopColor="#FFD87A" /><stop offset="1" stopColor="#FFB84D" />
        </linearGradient>
      </defs>
    </svg>
  );
}

export function Gleam({ size = 18 }: { size?: number }) {
  return (
    <svg viewBox="-12 -12 24 24" width={size} height={size} aria-hidden>
      <path d={STAR} fill="url(#fg-gold)" />
    </svg>
  );
}
