import type { CSSProperties } from "react";
import { copy, type Lang } from "../content";
import HeroScene, { type Phase } from "./HeroScene";

const d = (s: number) => ({ "--d": `${s}s` }) as CSSProperties;

/** One headline line rises out of a mask; the first is set quiet and the second in full ink, so the sentence has two beats. */
function Line({ children, delay, quiet }: { children: string; delay: number; quiet?: boolean }) {
  return (
    <span className="block overflow-hidden pb-[0.12em]">
      <span className={`a-line block ${quiet ? "text-ash" : ""}`} style={d(delay)}>{children}</span>
    </span>
  );
}

/**
 * The first screen: the words on the left, the scene on the right, and a field of light across both. The three steps under the
 * words light up as the scene reaches each of them, so what is read and what is seen say the same thing at the same moment.
 */
export default function Hero({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  // which step each part of the story belongs to: the photographs are step 1, the choosing step 2, the reasons step 3
  const lit = (phase: Phase, i: number) => (i === 0 ? phase >= 1 : i === 1 ? phase >= 2 : phase >= 4);
  return (
    <HeroScene lang={lang}>
      {(phase) => (
        <div className="text-center md:text-left">
          <h1 className={`font-semibold leading-[1.04] tracking-[-0.045em] ${lang === "zh" ? "text-[clamp(2rem,min(4vw,7vh),3.1rem)]" : "text-[clamp(2.1rem,min(4.2vw,7.2vh),3rem)]"}`}>
            <Line delay={0.05} quiet>{c.title[0]}</Line>
            <Line delay={0.16}>{c.title[1]}</Line>
          </h1>
          <div className="a-fade mt-5" style={d(0.4)}>
            <p className="mx-auto max-w-[52ch] text-[17px] leading-relaxed text-ash md:mx-0 md:text-lg">{c.body}</p>
            <a href="#flow" className="mt-7 inline-flex min-h-12 items-center whitespace-nowrap rounded-full bg-ink px-6 py-3 font-medium text-on-ink transition-transform duration-150 active:translate-y-px">{c.cta}</a>
          </div>

          <ol className="a-fade mx-auto mt-10 flex max-w-sm flex-col gap-2.5 text-left md:mx-0" style={d(0.6)} aria-label={c.steps.join(", ")}>
            {c.steps.map((s, i) => {
              const on = lit(phase, i);
              return (
                <li key={s} className="flex items-center gap-3">
                  <span className={`grid size-7 shrink-0 place-items-center rounded-full border text-[12px] tabular-nums transition-all duration-500 ${on ? "border-transparent bg-gleam text-gleam-ink shadow-[0_0_14px_color-mix(in_srgb,var(--gleam)_55%,transparent)]" : "border-line text-ash"}`}>{i + 1}</span>
                  <span className={`transition-colors duration-500 ${on ? "text-ink" : "text-ash/70"}`}>{s}</span>
                </li>
              );
            })}
          </ol>
        </div>
      )}
    </HeroScene>
  );
}
