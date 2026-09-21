import type { ReactNode } from "react";
import { Outlet, createRootRoute, HeadContent, Scripts, useRouterState } from "@tanstack/react-router";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "../styles.css";

export const Route = createRootRoute({
  head: () => ({
    meta: [{ charSet: "utf-8" }, { name: "viewport", content: "width=device-width, initial-scale=1" }, { name: "color-scheme", content: "dark light" }],
    links: [{ rel: "icon", type: "image/png", href: "/favicon.png" }],
  }),
  component: () => {
    const zh = useRouterState({ select: (s) => s.location.pathname.startsWith("/zh") });
    return (
      <html lang={zh ? "zh-CN" : "en"}>
        <head><HeadContent /></head>
        <body><Outlet /><Scripts /></body>
      </html>
    );
  },
});
