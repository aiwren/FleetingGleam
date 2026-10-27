import type { Lang } from "../content";
import { GleamDefs } from "./Gleam";
import { IconDefs } from "./Icons";
import Header from "./Header";
import Hero from "./Hero";
import Privacy from "./Privacy";
import Features from "./Features";
import Zoom from "./Zoom";
import Honest from "./Honest";
import How from "./How";
import Closing from "./Closing";
import Lightbox from "./Lightbox";

/** 首屏沿用原来的星云场景；从隐私起是 2026-10-27 的新设计，铺在纯色底上。 */
export default function Landing({ lang }: { lang: Lang }) {
  return (
    <div className="min-h-dvh overflow-x-clip">
      <GleamDefs />
      <IconDefs />
      <Header lang={lang} />
      <main>
        <Hero lang={lang} />
        <div className="ld">
          <Privacy lang={lang} />
          <Features lang={lang} />
          <Zoom lang={lang} />
          <Honest lang={lang} />
          <How lang={lang} />
          <Closing lang={lang} />
          <Lightbox lang={lang} />
        </div>
      </main>
    </div>
  );
}
