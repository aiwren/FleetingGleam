import { createFileRoute } from "@tanstack/react-router";
import Landing from "../components/Landing";
import { copy } from "../content";

export const Route = createFileRoute("/zh")({
  head: () => ({ meta: [{ title: copy.zh.title }, { name: "description", content: copy.zh.description }] }),
  component: () => <Landing lang="zh" />,
});
