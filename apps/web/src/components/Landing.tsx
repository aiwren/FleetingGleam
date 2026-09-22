import type { Lang } from "../content";
import Header from "./Header";
import Hero from "./Hero";
import Showcase from "./Showcase";
import Manifesto from "./Manifesto";
import Bento from "./Bento";
import Flow from "./Flow";
import Closing from "./Closing";

export default function Landing({ lang }: { lang: Lang }) {
  return (
    <div className="min-h-dvh overflow-x-clip">
      <Header lang={lang} />
      <main>
        <Hero lang={lang} />
        <Showcase lang={lang} />
        <Manifesto lang={lang} />
        <Bento lang={lang} />
        <Flow lang={lang} />
        <Closing lang={lang} />
      </main>
    </div>
  );
}
