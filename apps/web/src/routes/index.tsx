import { createFileRoute } from "@tanstack/react-router";
import Landing from "../components/Landing";
import { copy } from "../content";

export const Route = createFileRoute("/")({
  head: () => ({ meta: [{ title: copy.en.title }, { name: "description", content: copy.en.description }] }),
  component: () => <Landing lang="en" />,
});
