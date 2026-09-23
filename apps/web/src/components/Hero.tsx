import type { CSSProperties } from "react";
import { copy, type Lang } from "../content";
import HeroFan from "./HeroFan";

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
 * The first screen makes its whole case without asking for a scroll: headline and promise up top, and below them the one
 * showpiece — a burst of real photos fanned out, settling with weight and answering to the pointer. The full pass, scrubbed
 * by scroll, is its own section further down — a second act, not the opening line.
 */
export default function Hero({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  return (
    <section className="mx-auto flex min-h-[calc(100dvh-4rem)] w-full max-w-[1400px] flex-col justify-center px-5 pb-10 pt-8">
      <div className="flex flex-col gap-8 md:flex-row md:items-end md:justify-between md:gap-12">
        <h1 className="max-w-[16ch] text-[clamp(2.1rem,4.6vw,3.4rem)] font-semibold leading-[1.04] tracking-[-0.04em]">
          <Line delay={0.05} quiet>{c.title[0]}</Line>
          <Line delay={0.16}>{c.title[1]}</Line>
        </h1>
        <div className="a-fade shrink-0 md:max-w-[30ch] md:text-right" style={d(0.4)}>
          <p className="text-lg text-ash">{c.body}</p>
          <a href="#flow" className="mt-6 inline-flex min-h-12 items-center whitespace-nowrap rounded-full bg-ink px-6 py-3 font-medium text-on-ink transition-transform duration-150 active:translate-y-px">{c.cta}</a>
        </div>
      </div>

      <div className="mt-10 sm:mt-14 md:mt-16">
        <HeroFan />
      </div>
    </section>
  );
}
