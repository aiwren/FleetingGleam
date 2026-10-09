import type { CSSProperties } from "react";
import { copy, type Lang } from "../content";
import HeroStage from "./HeroStage";

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
 * The first screen says what the product does, in the product's own pictures: the headline and promise, and under them the
 * real app window, large and still. The scrolled culling pass is the next section, not the opening line.
 */
export default function Hero({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  return (
    <section className="mx-auto w-full max-w-6xl px-5 pb-8 pt-8 text-center md:pt-10">
      <h1 className={`mx-auto font-semibold leading-[1.04] tracking-[-0.045em] ${lang === "zh" ? "text-[clamp(1.9rem,min(4vw,7.4vh),3.5rem)]" : "text-[clamp(2.1rem,min(4.4vw,7.8vh),3.9rem)]"}`}>
        <Line delay={0.05} quiet>{c.title[0]}</Line>
        <Line delay={0.16}>{c.title[1]}</Line>
      </h1>
      <div className="a-fade mx-auto mt-5" style={d(0.4)}>
        <p className="mx-auto max-w-[66ch] text-[17px] leading-relaxed text-ash md:text-lg">{c.body}</p>
        <a href="#flow" className="mt-6 inline-flex min-h-12 items-center whitespace-nowrap rounded-full bg-ink px-6 py-3 font-medium text-on-ink transition-transform duration-150 active:translate-y-px">{c.cta}</a>
      </div>

      <div className="mt-9 md:mt-10">
        <HeroStage lang={lang} />
      </div>
    </section>
  );
}
