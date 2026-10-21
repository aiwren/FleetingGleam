import { createFileRoute } from "@tanstack/react-router";
import Legal from "../../components/Legal";
import { legal } from "../../legal";
import { pageHead } from "../../seo";

export const Route = createFileRoute("/zh/privacy")({
  head: () => pageHead("zh", "/zh/privacy", `${legal.zh.privacy.title} | FleetingGleam`, legal.zh.privacy.intro),
  component: () => <Legal lang="zh" kind="privacy" />,
});
