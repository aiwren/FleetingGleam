import { open } from "@tauri-apps/plugin-dialog";
import { createProject } from "./projects";
import { getEnabledShares, openPicker } from "./nas";

export const baseName = (p: string) => p.split("/").filter(Boolean).pop() ?? p;

/** 本机的文件夹：系统的文件夹对话框。 */
export async function pickLocalFolder(): Promise<string | null> {
  const dir = await open({ directory: true });
  return typeof dir === "string" ? dir : null;
}

/** 一个可以当项目文件夹的地方：本机文件夹，或（添加过 NAS 时）共享里的文件夹，由 FolderPicker 来问。 */
export async function pickFolder(): Promise<string | null> {
  return getEnabledShares().length > 0 ? openPicker() : pickLocalFolder();
}

/** Choose a folder and start a new project from it. */
export async function newProjectFromDialog(): Promise<boolean> {
  const dir = await pickFolder();
  if (!dir) return false;
  createProject(baseName(dir), [dir]);
  return true;
}
