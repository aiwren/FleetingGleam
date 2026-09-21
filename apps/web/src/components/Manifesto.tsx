import { useRef } from "react";
import { motion, useReducedMotion, useScroll, useTransform, type MotionValue } from "motion/react";
import { Aperture, ShieldCheck, Sparkle } from "@phosphor-icons/react";
import { copy, type Lang } from "../content";

const ICONS = { ShieldCheck, Aperture, Sparkle } as const;

function Word({ w, progress, i, n, sep }: { w: string; progress: MotionValue<number>; i: number; n: number; sep: string }) {
  const opacity = useTransform(progress, [i / n, Math.min(1, (i + 3) / n)], [0.16, 1]);
  return <motion.span style={{ opacity }}>{w}{sep}</motion.span>;
}

/** The privacy promise is the one sentence that has to be read whole, so it is lit word by word as the reader scrolls. */
export default function Manifesto({ lang }: { lang: Lang }) {
  const c = copy[lang].privacy;
  const reduce = useReducedMotion();
  const ref = useRef<HTMLParagraphElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start 0.85", "end 0.5"] });
  // Chinese has no spaces: light it a character group at a time instead.
  const words = lang === "zh" ? c.text.match(/.{1,3}/g)! : c.text.split(" ");
  return (
    <section id="privacy" className="mx-auto max-w-6xl px-5 pt-32 md:pt-48">
      <p ref={ref} className="max-w-4xl text-[clamp(1.6rem,3.6vw,2.75rem)] font-medium leading-[1.25] tracking-tight">
        {words.map((w, i) => reduce ? <span key={i}>{w}{lang === "zh" ? "" : " "}</span> : <Word key={i} w={w} sep={lang === "zh" ? "" : " "} progress={scrollYProgress} i={i} n={words.length} />)}
      </p>
      <ul className="mt-14 flex flex-wrap gap-x-10 gap-y-4 text-ash">
        {c.points.map(([icon, label]) => {
          const Icon = ICONS[icon as keyof typeof ICONS];
          return <li key={label} className="flex items-center gap-2.5"><Icon size={20} weight="regular" className="text-gleam" />{label}</li>;
        })}
      </ul>
    </section>
  );
}
