import type { CSSProperties } from "react";

type Tile = { w: number; h: number; a: string; b: string; state?: "keep" | "out"; best?: boolean };

// Painted stand-ins for photographs (no real pictures are shown on the page): a frame of one moment, then a few others.
const TILES: Tile[] = [
  { w: 3, h: 2, a: "#c9865a", b: "#3b2a36", best: true, state: "keep" },
  { w: 3, h: 2, a: "#b57955", b: "#33263a" },
  { w: 3, h: 2, a: "#a86f52", b: "#2f2438", state: "out" },
  { w: 2, h: 3, a: "#6f8f9c", b: "#1f2f3c" },
  { w: 3, h: 2, a: "#8a9a6c", b: "#26332a" },
  { w: 3, h: 2, a: "#d1a45e", b: "#4a3320", state: "keep" },
  { w: 3, h: 2, a: "#7e86a8", b: "#262a44" },
  { w: 2, h: 3, a: "#b58a86", b: "#3a2a2c" },
  { w: 3, h: 2, a: "#79a08e", b: "#213a34" },
];

export default function AppMock({ labels }: { labels: { keep: string; out: string; best: string; group: string; name: string } }) {
  return (
    <div className="mock" aria-hidden="true">
      <div className="mock-bar"><i /><i /><i /><span className="mono">{labels.name}</span></div>
      <div className="mock-body">
        <div className="mock-rail"><b /><span /><span /><span className="on" /></div>
        <div className="mock-wall">
          <div className="mock-cap mono">{labels.group}</div>
          <div className="mock-grid">
            {TILES.map((t, i) => (
              <div key={i} className={`mock-tile ${t.state ?? ""}`} style={{ "--a": t.a, "--b": t.b, gridRow: t.h > t.w ? "span 2" : undefined, "--i": i } as CSSProperties}>
                {t.best && <em className="mono">{labels.best}</em>}
                {t.state === "keep" && <s className="chk"><svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M2.5 6.5l2.5 2.5 4.5-5.5" /></svg></s>}
                {t.state === "out" && <s className="x mono">× {labels.out}</s>}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
