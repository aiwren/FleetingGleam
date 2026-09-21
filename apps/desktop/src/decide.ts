import { Decision, Photo, Reason, State, pct } from "./types";

/** Tunable, and not yet calibrated on labelled data. */
const KEEP_FLOOR = 0.3;
const REVIEW_FLOOR = 0.2;
const REVIEW_SHARE = 0.15;
/** Score subtracted per already-kept photo of the same scene, so picks cover the whole trip. */
const SCENE_PENALTY = 0.06;
/** A non-best photo this close to its group's best is worth a second look. */
const NEAR_BEST = 0.04;

const HARD = new Set(["blur", "utility"]);

function flagReasons(p: Photo): Reason[] {
  const r: Reason[] = [];
  if (p.flags.includes("blur")) r.push({ k: "reason.blur" });
  if (p.flags.includes("utility")) r.push({ k: "reason.utility" });
  if (p.flags.includes("over")) r.push({ k: "reason.over", p: { pct: Math.round(p.clipped_high * 100) } });
  if (p.flags.includes("under")) r.push({ k: "reason.under" });
  if (p.flags.includes("eyes")) r.push({ k: "reason.eyes" });
  return r;
}

/** Three-way decision: keep / review / reject, each with human-readable reasons. */
export function decide(photos: Photo[], keepRatio: number, overrides: Map<string, State>): Map<string, Decision> {
  const n = photos.length;
  const byGroup = new Map<number, Photo[]>();
  photos.forEach((p) => byGroup.set(p.group, [...(byGroup.get(p.group) ?? []), p]));
  const bestOf = (g: number) => byGroup.get(g)!.find((p) => p.group_best)!;

  const hard = (p: Photo) => p.flags.some((f) => HARD.has(f));
  const soft = (p: Photo) => p.flags.some((f) => !HARD.has(f));
  const eligible = photos.filter((p) => p.group_best && !hard(p));

  const keepN = Math.max(1, Math.round(n * keepRatio));
  const reviewN = Math.max(1, Math.round(n * REVIEW_SHARE));
  const sceneCount = new Map<number, number>();
  const chosen = new Map<string, State>();

  const takeGreedy = (pool: Photo[], limit: number, floor: number, state: State, countScene: boolean) => {
    const rest = [...pool];
    let taken = 0;
    while (taken < limit && rest.length) {
      let bi = -1, bv = -Infinity;
      rest.forEach((p, i) => {
        const v = p.score - SCENE_PENALTY * (sceneCount.get(p.scene) ?? 0);
        if (v > bv) { bv = v; bi = i; }
      });
      const [p] = rest.splice(bi, 1);
      if (p.score < floor) break;
      chosen.set(p.path, state);
      if (countScene) sceneCount.set(p.scene, (sceneCount.get(p.scene) ?? 0) + 1);
      taken++;
    }
  };
  takeGreedy(eligible.filter((p) => !soft(p)), keepN, KEEP_FLOOR, "keep", true);
  takeGreedy(eligible.filter((p) => !chosen.has(p.path)), reviewN, REVIEW_FLOOR, "review", false);

  const out = new Map<string, Decision>();
  for (const p of photos) {
    const best = bestOf(p.group);
    const size = byGroup.get(p.group)!.length;
    const flagR = flagReasons(p);
    let state: State;
    let reasons: Reason[] = [];

    if (hard(p)) {
      state = "reject";
      reasons = flagR;
    } else if (chosen.has(p.path)) {
      state = chosen.get(p.path)!;
      if (state === "keep") {
        reasons.push(size > 1 ? { k: "reason.keepGroup", p: { n: size, score: pct(p.score) } } : { k: "reason.keepSolo", p: { score: pct(p.score) } });
      } else {
        reasons.push({ k: "reason.reviewEdge", p: { score: pct(p.score) } });
      }
      reasons.push(...flagR);
    } else if (!p.group_best) {
      if (p.score >= best.score - NEAR_BEST && !hard(p)) {
        state = "review";
        reasons.push({ k: "reason.nearBest", p: { diff: Math.max(0, pct(best.score - p.score)) } });
      } else {
        state = "reject";
        reasons.push({ k: "reason.betterExists", p: { best: pct(best.score), score: pct(p.score) } });
      }
      reasons.push(...flagR);
    } else {
      state = "reject";
      reasons.push(soft(p) ? { k: "reason.exposureFloor" } : { k: "reason.belowLine", p: { score: pct(p.score) } });
      reasons.push(...flagR);
    }

    const o = overrides.get(p.path);
    if (o) out.set(p.path, { state: o, manual: true, reasons: [{ k: "reason.manual", p: { stateKey: `state.${o}` } }, ...reasons] });
    else out.set(p.path, { state, manual: false, reasons });
  }
  return out;
}
