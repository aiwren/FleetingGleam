import { convertFileSrc } from "@tauri-apps/api/core";

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
  /** Faces big enough to recognise: `[x, y, w, h]`, top-left origin, normalised to the photo. */
  face_boxes: [number, number, number, number][];
  /** Which recurring person each of `face_boxes` is (0 = the person in the most photos, -1 = nobody in particular). */
  people: number[];
  /** HDR (a gain map, or an HDR format): shown in HDR in the full-size viewer (thumbnails are always standard range). */
  hdr: boolean;
  /** A motion photo: a short video follows the picture inside the file. */
  motion: boolean;
  /** 显示时顺时针转的角度（0 / 90 / 180 / 270）；缩略图、大图和 width / height 都已经是转过之后的。 */
  rotation?: number;
};

const RAW_EXT = new Set(["dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f", "3fr", "erf", "kdc", "mef", "mos", "mrw", "raw", "rwl", "iiq", "fff", "dcr", "k25", "rwz"]);
const FORMAT_TAG = new Set(["heic", "heif", "avif", "jxl", "tif", "tiff", "bmp", "gif"]);
const extOf = (p: Photo) => (p.path.split(".").pop() ?? "").toLowerCase();

/** The small picture of a photo (the web view shows it as standard range; the full-size viewer shows the real HDR). */
export const thumbSrc = (p: Photo) => convertFileSrc(p.thumb);

/** What to tag a photo with, in order: its format when that is notable, Live or else HDR, and long shapes (panoramas and tall strips). */
export type PhotoTag = { id: "raw" | "format" | "hdr" | "motion" | "long"; text?: string };
export function photoTags(p: Photo): PhotoTag[] {
  const out: PhotoTag[] = [];
  const e = extOf(p);
  if (RAW_EXT.has(e)) out.push({ id: "raw", text: e === "dng" ? "RAW · DNG" : `RAW · ${e.toUpperCase()}` });
  else if (FORMAT_TAG.has(e)) out.push({ id: "format", text: e === "tif" ? "TIFF" : e.toUpperCase() });
  // Live and HDR are never both shown: a Live photo says "Live" (the more useful thing to know), otherwise HDR.
  if (p.motion) out.push({ id: "motion" });
  else if (p.hdr) out.push({ id: "hdr" });
  const r = p.width / Math.max(1, p.height);
  if (r >= 2.2 || r <= 1 / 2.2) out.push({ id: "long" });
  return out;
}

export type State = "keep" | "review" | "reject";
/** A reason is a message key plus parameters, so the UI can render it in the current language. */
export type Reason = { k: string; p?: Record<string, string | number> };
export type Decision = { state: State; manual: boolean; reasons: Reason[] };

export const pct = (v: number) => Math.round(v * 100);
/** A 0..1 score as a friendlier 1..5 star rating: the exact number still lives in the details panel for anyone who wants it. */
export const stars = (v: number) => Math.max(1, Math.min(5, Math.ceil(v * 5)));

export type ProbeFile = { path: string; width: number; height: number; cached: boolean };
/** `offline`：项目在 NAS 上、现在连不上，照片是上次同步到本机镜像的那些（核心跳过了同步，不报错）。 */
export type Probe = { files: ProbeFile[]; bytes: number; uncached: number; offline?: boolean };
