import type { ReactNode } from "react";
import { Outlet, createRootRoute, HeadContent, Scripts, useRouterState } from "@tanstack/react-router";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "../styles.css";
import { SITE } from "../content";

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: "utf-8" }, { name: "viewport", content: "width=device-width, initial-scale=1" }, { name: "color-scheme", content: "dark light" },
      // the share card every page carries (title, description and url come from the page itself)
      { property: "og:type", content: "website" }, { property: "og:site_name", content: "FleetingGleam" },
      { property: "og:image", content: `${SITE}/og.png` }, { property: "og:image:width", content: "1200" }, { property: "og:image:height", content: "630" },
      { name: "twitter:card", content: "summary_large_image" }, { name: "twitter:image", content: `${SITE}/og.png` },
    ],
    links: [{ rel: "icon", type: "image/png", href: "/favicon.png" }],
  }),
  component: () => {
    const zh = useRouterState({ select: (s) => s.location.pathname.startsWith("/zh") });
    return (
      <html lang={zh ? "zh-CN" : "en"} suppressHydrationWarning>
        <head>
          <HeadContent />
          {/* On the English home page only: a visitor whose browser prefers Chinese lands on /zh/, unless they chose a language before. Runs before the first paint, so nothing flashes. */}
          <script dangerouslySetInnerHTML={{ __html: `try{if(location.pathname==="/"){var l=localStorage.getItem("lang");if(l?l==="zh":/^zh/i.test((navigator.languages&&navigator.languages[0])||navigator.language||""))location.replace("/zh/"+location.search+location.hash)}}catch(e){}` }} />
          {/* Settles the theme before the first paint so a stored choice never flashes the other one. */}
          <script dangerouslySetInnerHTML={{ __html: `try{var t=localStorage.getItem("theme");if(t!=="light"&&t!=="dark")t=matchMedia("(prefers-color-scheme: light)").matches?"light":"dark";document.documentElement.dataset.theme=t}catch(e){}` }} />
          {/* Without scripts the entrance animations never run, so nothing may stay hidden. */}
          <noscript><style>{"*{opacity:1!important;transform:none!important}"}</style></noscript>
        </head>
        <body><Outlet /><Scripts /></body>
      </html>
    );
  },
});
