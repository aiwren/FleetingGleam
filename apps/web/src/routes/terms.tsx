import { createFileRoute } from "@tanstack/react-router";
import Legal from "../components/Legal";
import { legal } from "../legal";
import { pageHead } from "../seo";

export const Route = createFileRoute("/terms")({
  head: () => pageHead("en", "/terms", `${legal.en.terms.title} | FleetingGleam`, legal.en.terms.intro),
  component: () => <Legal lang="en" kind="terms" />,
});
