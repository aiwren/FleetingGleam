/** A real screenshot of the app in a quiet frame. Photos in it are painted samples from the development build. */
export default function Frame({ src, alt, priority = false, className = "" }: { src: string; alt: string; priority?: boolean; className?: string }) {
  return (
    <div className={`overflow-hidden rounded-3xl border border-line bg-panel shadow-[0_50px_100px_-50px_rgba(0,0,0,0.7)] ${className}`}>
      <img src={src} alt={alt} width={2400} height={1500} decoding="async" fetchPriority={priority ? "high" : "auto"} className="block h-auto w-full" draggable={false} />
    </div>
  );
}
