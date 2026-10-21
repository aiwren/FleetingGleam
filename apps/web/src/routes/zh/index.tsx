import { createFileRoute } from "@tanstack/react-router";
import Landing from "../../components/Landing";
import { copy } from "../../content";
import { pageHead } from "../../seo";

export const Route = createFileRoute("/zh/")({
  head: () => pageHead("zh", "/zh/", copy.zh.title, copy.zh.description),
  component: () => <Landing lang="zh" />,
});
