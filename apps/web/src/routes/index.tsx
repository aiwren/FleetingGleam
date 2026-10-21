import { createFileRoute } from "@tanstack/react-router";
import Landing from "../components/Landing";
import { copy } from "../content";
import { pageHead } from "../seo";

export const Route = createFileRoute("/")({
  head: () => pageHead("en", "/", copy.en.title, copy.en.description),
  component: () => <Landing lang="en" />,
});
