import { createFileRoute } from "@tanstack/react-router";
import Legal from "../../components/Legal";
import { legal } from "../../legal";

export const Route = createFileRoute("/zh/terms")({
  head: () => ({ meta: [{ title: `${legal.zh.terms.title} | FleetingGleam` }] }),
  component: () => <Legal lang="zh" kind="terms" />,
});
