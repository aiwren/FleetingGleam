import { SITE, type Lang } from "./content";

/** The head of one page: title, description, the share card, its canonical address and the same page in the other language. */
export function pageHead(lang: Lang, path: string, title: string, description: string) {
  const en = path.replace(/^\/zh/, "") || "/";
  const zh = `/zh${en === "/" ? "/" : en}`;
  const here = `${SITE}${lang === "zh" ? zh : en}`;
  return {
    meta: [
      { title },
      { name: "description", content: description },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: here },
      { property: "og:locale", content: lang === "zh" ? "zh_CN" : "en_US" },
    ],
    links: [
      { rel: "canonical", href: here },
      { rel: "alternate", hreflang: "en", href: `${SITE}${en}` },
      { rel: "alternate", hreflang: "zh-CN", href: `${SITE}${zh}` },
      { rel: "alternate", hreflang: "x-default", href: `${SITE}${en}` },
    ],
  };
}
