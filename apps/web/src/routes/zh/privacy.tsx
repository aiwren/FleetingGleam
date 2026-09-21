import { createFileRoute } from "@tanstack/react-router";
import Legal from "../../components/Legal";
import { legal } from "../../legal";

export const Route = createFileRoute("/zh/privacy")({
  head: () => ({ meta: [{ title: `${legal.zh.privacy.title} | FleetingGleam` }] }),
  component: () => <Legal lang="zh" kind="privacy" />,
});
