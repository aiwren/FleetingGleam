import { open } from "@tauri-apps/plugin-dialog";
import { createProject } from "./projects";

export const baseName = (p: string) => p.split("/").filter(Boolean).pop() ?? p;

export async function pickFolder(): Promise<string | null> {
  const dir = await open({ directory: true });
  return typeof dir === "string" ? dir : null;
}

/** Choose a folder and start a new project from it. */
export async function newProjectFromDialog(): Promise<boolean> {
  const dir = await pickFolder();
  if (!dir) return false;
  createProject(baseName(dir), [dir]);
  return true;
}
