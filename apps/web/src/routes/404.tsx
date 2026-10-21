import { createFileRoute } from "@tanstack/react-router";
import NotFound from "../components/NotFound";

/** Prerendered to 404.html, which the static host serves for any unknown address. */
export const Route = createFileRoute("/404")({
  head: () => ({ meta: [{ title: "Page not found | FleetingGleam" }, { name: "robots", content: "noindex" }] }),
  component: NotFound,
});
