import { createFileRoute } from "@tanstack/react-router";
import Home from "../components/home/Home";
import css from "../components/home/home.css?url";
import { copy } from "../content";
import { pageHead } from "../seo";

// 首页的样式表只在首页加载；首屏大图先预加载
export const Route = createFileRoute("/")({
  head: () => {
    const h = pageHead("en", "/", copy.en.title, copy.en.description);
    return { ...h, links: [...h.links, { rel: "stylesheet", href: css }, { rel: "preload", as: "image", href: "/home/peak-2400.webp", imageSrcSet: "/home/peak-1200.webp 1200w, /home/peak-2400.webp 2400w", imageSizes: "100vw" }] };
  },
  component: () => <Home lang="en" />,
});
