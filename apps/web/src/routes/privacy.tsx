import { createFileRoute } from "@tanstack/react-router";
import Legal from "../components/Legal";
import { legal } from "../legal";

export const Route = createFileRoute("/privacy")({
  head: () => ({ meta: [{ title: `${legal.en.privacy.title} | FleetingGleam` }] }),
  component: () => <Legal lang="en" kind="privacy" />,
});
