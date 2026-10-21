import { createFileRoute } from "@tanstack/react-router";
import Legal from "../../components/Legal";
import { legal } from "../../legal";
import { pageHead } from "../../seo";

export const Route = createFileRoute("/zh/terms")({
  head: () => pageHead("zh", "/zh/terms", `${legal.zh.terms.title} | FleetingGleam`, legal.zh.terms.intro),
  component: () => <Legal lang="zh" kind="terms" />,
});
