export type Photo = {
  path: string;
  name: string;
  thumb: string;
  width: number;
  height: number;
  sharpness: number;
  brightness: number;
  clipped_high: number;
  clipped_low: number;
  sharp_score: number;
  expo_score: number;
  tech_score: number;
  clip_score: number | null;
  sys_score: number | null;
  ai_score: number | null;
  pref_score: number | null;
  face_count: number;
  face_min: number | null;
  /** Openness of the most closed eyes among measurable faces (null = not measured). */
  eyes_open: number | null;
  /** Scene kind: portrait, people, landscape, architecture, food, animal, object, document (or empty). */
  kind: string;
  utility: boolean;
  flags: string[];
  taken_at: number | null;
  score: number;
  group: number;
  scene: number;
  group_best: boolean;
};

export type State = "keep" | "review" | "reject";
/** A reason is a message key plus parameters, so the UI can render it in the current language. */
export type Reason = { k: string; p?: Record<string, string | number> };
export type Decision = { state: State; manual: boolean; reasons: Reason[] };

export const pct = (v: number) => Math.round(v * 100);

export type ProbeFile = { path: string; width: number; height: number; cached: boolean };
export type Probe = { files: ProbeFile[]; bytes: number; uncached: number };
