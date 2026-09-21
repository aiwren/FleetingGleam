/** A real screenshot of the app in a quiet frame, in the dark or the light version of the app to match the page. Photos are samples. */
export default function Frame({ shot, alt, priority = false, className = "" }: { shot: "wall" | "compare"; alt: string; priority?: boolean; className?: string }) {
  const img = (theme: "dark" | "light") => (
    <img src={`/shots/${shot}-${theme}.jpg`} alt={alt} width={2400} height={1500} decoding="async" fetchPriority={priority && theme === "dark" ? "high" : "auto"}
      className={`h-auto w-full ${theme === "dark" ? "for-dark" : "for-light"}`} draggable={false} />
  );
  return (
    <div className={`overflow-hidden rounded-3xl border border-line bg-panel shadow-[0_50px_100px_-50px_rgba(0,0,0,0.55)] ${className}`}>
      {img("dark")}{img("light")}
    </div>
  );
}
