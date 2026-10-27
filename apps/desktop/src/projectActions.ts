import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { createProject, getStore, setActive, updateProject } from "./projects";
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

/**
 * 拖进窗口的文件夹建项目：只拖了一个、而它正好就是某个项目的文件夹，就打开那个项目；否则用这些文件夹建一个新的（名字取第一个）。
 */
export function projectFromFolders(dirs: string[]): boolean {
  if (!dirs.length) return false;
  const existing = dirs.length === 1 ? getStore().projects.find((p) => !p.trashed && p.folders.length === 1 && p.folders[0] === dirs[0]) : undefined;
  if (existing) setActive(existing.id);
  else createProject(baseName(dirs[0]), dirs);
  return true;
}

/** 示例项目建好后第一次到准备页就自动开始分析（它本来就是来看效果的）；Culling 取一次就清掉。 */
let autoStart: string | null = null;
export function takeAutoStart(id: string): boolean {
  if (autoStart !== id) return false;
  autoStart = null;
  return true;
}

/**
 * 起始屏的"试试示例照片"（和 Android 一样）：应用自带的十张照片复制到数据目录，建一个项目并打开（已经有一个就直接打开它）。
 * 示例项目开着"相似的只留最好的一张"，两组连拍一眼能看出推荐。
 */
export async function openSamples(name: string): Promise<boolean> {
  let dir: string;
  try { dir = await invoke<string>("samples_folder"); } catch { return false; }
  const existing = getStore().projects.find((p) => !p.trashed && p.folders.length === 1 && p.folders[0] === dir);
  if (existing) { setActive(existing.id); return true; }
  const p = createProject(name, [dir]);
  updateProject(p.id, { onePerGroup: true });
  autoStart = p.id;
  return true;
}
