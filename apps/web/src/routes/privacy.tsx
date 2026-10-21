import { createFileRoute } from "@tanstack/react-router";
import Legal from "../components/Legal";
import { legal } from "../legal";
import { pageHead } from "../seo";

export const Route = createFileRoute("/privacy")({
  head: () => pageHead("en", "/privacy", `${legal.en.privacy.title} | FleetingGleam`, legal.en.privacy.intro),
  component: () => <Legal lang="en" kind="privacy" />,
});
