import { createFileRoute } from "@tanstack/react-router";
import Legal from "../components/Legal";
import { legal } from "../legal";

export const Route = createFileRoute("/terms")({
  head: () => ({ meta: [{ title: `${legal.en.terms.title} | FleetingGleam` }] }),
  component: () => <Legal lang="en" kind="terms" />,
});
