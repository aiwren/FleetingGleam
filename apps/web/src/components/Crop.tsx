/**
 * One region of a real app screenshot, cut out and given its own edge. Coordinates are in the 1200×750 space of the shots;
 * `scale` is how many screen pixels one of those units gets. Both themes are in the markup but lazy, so only the one the theme
 * shows is ever downloaded.
 */
export default function Crop({ shot, x, y, w, h, scale, className = "" }: { shot: "wall" | "compare"; x: number; y: number; w: number; h: number; scale: number; className?: string }) {
  return (
    <div className={`relative overflow-hidden rounded-[18px] border border-line bg-panel ${className}`} style={{ width: w * scale, height: h * scale }}>
      {(["dark", "light"] as const).map((t) => (
        <img key={t} src={`/shots/${shot}-${t}-2400.webp`} alt="" width={2400} height={1500} loading="lazy" decoding="async" draggable={false}
          className={t === "dark" ? "for-dark" : "for-light"}
          style={{ position: "absolute", maxWidth: "none", height: "auto", width: 1200 * scale, left: -x * scale, top: -y * scale }} />
      ))}
    </div>
  );
}
