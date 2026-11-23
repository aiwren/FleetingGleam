import { SITE, release, type Lang } from "./content";

/** The head of one page: title, description, the share card, its canonical address and the same page in the other language. */
export function pageHead(lang: Lang, path: string, title: string, description: string, keywords?: string) {
  const en = path.replace(/^\/zh/, "") || "/";
  const zh = `/zh${en === "/" ? "/" : en}`;
  const here = `${SITE}${lang === "zh" ? zh : en}`;
  const isZh = lang === "zh";
  return {
    meta: [
      { title },
      { name: "description", content: description },
      ...(keywords ? [{ name: "keywords", content: keywords }] : []),
      { name: "robots", content: "index, follow, max-image-preview:large" },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: here },
      { property: "og:locale", content: isZh ? "zh_CN" : "en_US" },
      { property: "og:locale:alternate", content: isZh ? "en_US" : "zh_CN" },
      { property: "og:image:alt", content: title },
      { name: "twitter:title", content: title },
      { name: "twitter:description", content: description },
      { name: "twitter:image:alt", content: title },
    ],
    links: [
      { rel: "canonical", href: here },
      { rel: "alternate", hreflang: "en", href: `${SITE}${en}` },
      { rel: "alternate", hreflang: "zh-CN", href: `${SITE}${zh}` },
      { rel: "alternate", hreflang: "x-default", href: `${SITE}${en}` },
    ],
  };
}

/** Structured data (JSON-LD) for search engines to display rich software results. */
export function softwareJsonLd(lang: Lang, description: string) {
  const isZh = lang === "zh";
  return {
    type: "application/ld+json",
    children: JSON.stringify({
      "@context": "https://schema.org",
      "@type": "SoftwareApplication",
      name: isZh ? "惊鸿 FleetingGleam" : "FleetingGleam",
      alternateName: isZh ? "FleetingGleam" : "惊鸿",
      applicationCategory: "PhotographyApplication",
      operatingSystem: "macOS 15+, Android 12+",
      url: isZh ? `${SITE}/zh/` : `${SITE}/`,
      downloadUrl: release.download,
      installUrl: release.android,
      screenshot: `${SITE}/shots/wall-dark-1200.webp`,
      image: `${SITE}/og.png`,
      description,
      inLanguage: isZh ? "zh-CN" : "en",
      isAccessibleForFree: true,
      license: "https://www.gnu.org/licenses/agpl-3.0.html",
      codeRepository: release.repo,
      sameAs: [release.repo],
      offers: {
        "@type": "Offer",
        price: "0",
        priceCurrency: "USD",
      },
    }),
  };
}
