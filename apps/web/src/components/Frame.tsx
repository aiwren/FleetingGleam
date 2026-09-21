/**
 * A real screenshot of the app in a quiet frame, in the dark or the light version of the app to match the page. Photos are samples.
 * Both versions are in the markup but lazy: the one hidden by the theme is never rendered, so it is never downloaded.
 */
export default function Frame({ shot, alt, priority = false, className = "", sizes = "(min-width: 1200px) 1112px, 100vw" }: { shot: "wall" | "compare"; alt: string; priority?: boolean; className?: string; sizes?: string }) {
  const img = (theme: "dark" | "light") => (
    <img src={`/shots/${shot}-${theme}-1200.webp`} srcSet={`/shots/${shot}-${theme}-1200.webp 1200w, /shots/${shot}-${theme}-2400.webp 2400w`} sizes={sizes}
      alt={alt} width={2400} height={1500} decoding="async" loading="lazy" fetchPriority={priority ? "high" : "auto"}
      className={`h-auto w-full ${theme === "dark" ? "for-dark" : "for-light"}`} draggable={false} />
  );
  // The shadow is tinted with the ink colour of the theme, and a hairline of light runs along the top edge like a real bezel.
  return (
    <div className={`overflow-hidden rounded-3xl border border-line bg-panel shadow-[inset_0_1px_0_color-mix(in_srgb,var(--ink)_14%,transparent),0_50px_100px_-50px_color-mix(in_srgb,var(--ink)_38%,transparent)] ${className}`}>
      {img("dark")}{img("light")}
    </div>
  );
}
