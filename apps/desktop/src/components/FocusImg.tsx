import { useEffect, useRef, useState } from "react";

/** Images already shown once this session. Re-mounts (view switches) skip the reveal. */
const seen = new Set<string>();
export const markSeen = (src: string) => { seen.add(src); };

/**
 * `quick` (browsing finished photos) skips the effect and only fades in briefly.
 * "Focus pull" reveal: a new photo arrives soft, slightly over-exposed and a touch large,
 * then racks into focus. Purely a filter/transform/opacity transition (see .focus in App.css).
 */
export default function FocusImg({ src, className = "", quick = false }: { src: string; className?: string; quick?: boolean }) {
  const [ready, setReady] = useState(() => seen.has(src));
  const [missing, setMissing] = useState(false);
  const ref = useRef<HTMLImageElement>(null);
  useEffect(() => { setMissing(false); }, [src]);

  useEffect(() => {
    const el = ref.current;
    if (el && el.complete && el.naturalWidth > 0) { seen.add(src); setReady(true); }
  }, [src]);

  // a thumbnail that is gone (the cache was cleared, the file damaged) shows as a quiet blank rather than an invisible tile
  if (missing) return <span className={`focus in missing ${className}`} aria-hidden />;
  return (
    <img
      ref={ref}
      className={`focus ${quick ? "quick" : ""} ${ready ? "in" : ""} ${className}`}
      src={src}
      alt=""
      draggable={false}
      decoding="async"
      onLoad={() => { seen.add(src); setReady(true); }}
      onError={() => setMissing(true)}
    />
  );
}
