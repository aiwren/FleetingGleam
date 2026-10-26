import { invoke } from "@tauri-apps/api/core";
import { PackId, ensurePacks, packInstalled, refreshPacks } from "./models";
import { EyeMethod, getSettings, Quality, setSettings } from "./settings";

/**
 * How photos are analysed, decided here instead of asked of the person.
 *
 * - One model for the whole app. The learned taste, grouping and search all compare the embeddings of photos, which only agree when the same
 *   model made them, so the choice is made once (the first analysis) and kept: the best the machine can run at a pace that is bearable.
 *   Someone who analysed before keeps what their results were made with (see settings.ts).
 * - The better eye-closing model whenever it is on the machine (it is a few megabytes, and comes with the first download).
 * - The hardware: whatever the system offers.
 */
export type Machine = { memory_gb: number; cores: number };

let machine: Promise<Machine> | null = null;
export const getMachine = () => (machine ??= invoke<Machine>("machine_info").catch(() => ({ memory_gb: 8, cores: 4 })));

/**
 * The larger model costs more memory (about 600 MB of weights, plus the pictures being decoded alongside), and a weaker GPU takes proportionally
 * longer, so it is for machines that have the room: 16 GB and 8 cores or more.
 */
export const capableOfHigh = (m: Machine) => m.memory_gb >= 16 && m.cores >= 8;

/** The packs an analysis with this model needs. */
export const tierPacks = (t: Quality): PackId[] => (t === "ultra" ? ["standard", "ultra"] : t === "high" ? ["high"] : ["standard"]);

/** What the app would settle on: what is already on the machine decides first (an install from before the app chose for itself), then what the machine can run. */
export async function predictTier(): Promise<Quality> {
  const s = getSettings();
  if (s.tier) return s.tier;
  await refreshPacks();
  if (packInstalled("high") && !packInstalled("standard")) return "high";
  if (packInstalled("standard") && !packInstalled("high")) return "fast";
  return capableOfHigh(await getMachine()) ? "high" : "fast";
}

/**
 * 第一次分析前把模型定下来：先问要不要下（大的那档可以"先用标准模型"），**下好了**才记住档位——
 * 下载被取消时什么都不记，下次照样按这台电脑来挑（以前在下载前就写，取消一次就被锁死）。返回定下的档位，取消时 null。
 */
export async function settleTier(): Promise<Quality | null> {
  const settled = getSettings().tier;
  let tier = await predictTier();
  // 还没定过、又是大模型：询问框里给"先用标准模型"
  let answer = await ensurePacks(analysisPacks(tier), !settled && tier !== "fast");
  if (answer === "standard") {
    tier = "fast";
    answer = await ensurePacks(analysisPacks(tier));
  }
  if (answer === "cancelled" || answer === "standard") return null;
  if (!getSettings().tier) setSettings({ tier });
  return tier;
}

/** The tier to analyse with right now, without waiting: what was settled on, or the quick one while nothing is. */
export const currentTier = (): Quality => getSettings().tier ?? "fast";

/** The eye model: the learned one when it is here, the system's geometric measure otherwise. */
export const eyeMethodNow = (): EyeMethod => (packInstalled("eyes") ? "mediapipe" : "vision");

/** Everything one analysis needs: its model, and the small extras (eyes, people) that make it better. Asked for under one question. */
export const analysisPacks = (t: Quality): PackId[] => [...tierPacks(t), "eyes", "faces"];

/**
 * Roughly how long analysing `photos` new photos takes (seconds): per photo, what the analysis measured on a 10-core Mac with its GPU (the encoder runs
 * there: about 0.05 s a photo for the larger model and 0.03 s for the quick one, decoding included; the experimental tier still runs on the CPU), scaled to the cores.
 */
export function estimateSeconds(t: Quality, photos: number, cores: number): number {
  const perPhoto = t === "high" ? 0.05 : t === "ultra" ? 1.2 : 0.03;
  return Math.round(photos * perPhoto * Math.max(1, 10 / Math.max(1, cores)));
}

/** The tier a pack that was just fetched on purpose (from the models list) stands for: getting a better model is asking to use it. */
export function adopt(id: PackId) {
  if (id === "high" || id === "ultra") setSettings({ tier: id, tierRev: getSettings().tierRev + 1 });
}

/** A model that was in use has been removed: the app settles on one again the next time it analyses. */
export function release(id: PackId) {
  if (getSettings().tier === id) setSettings({ tier: null, tierRev: getSettings().tierRev + 1 });
}
