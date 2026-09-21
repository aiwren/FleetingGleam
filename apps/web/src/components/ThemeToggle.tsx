import { Moon, Sun } from "@phosphor-icons/react";

/** Flips between the two themes. The choice is remembered; until one is made the page follows the system. */
export default function ThemeToggle({ label }: { label: string }) {
  const flip = () => {
    const root = document.documentElement;
    const dark = root.dataset.theme ? root.dataset.theme === "dark" : !matchMedia("(prefers-color-scheme: light)").matches;
    const next = dark ? "light" : "dark";
    root.dataset.theme = next;
    try { localStorage.setItem("theme", next); } catch { /* private mode */ }
  };
  return (
    <button type="button" onClick={flip} aria-label={label} title={label}
      className="ml-1 grid size-9 place-items-center rounded-full border border-line text-ash transition-colors hover:bg-surface hover:text-ink active:translate-y-px">
      <Sun size={17} className="theme-sun" /><Moon size={17} className="theme-moon" />
    </button>
  );
}
