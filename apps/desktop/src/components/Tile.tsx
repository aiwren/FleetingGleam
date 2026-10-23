import { KeyboardEvent, memo, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { DUR, SPRING } from "../motion";
import { Photo, State, pct, thumbSrc } from "../types";
import PhotoTags from "./PhotoTags";
import FocusImg, { markSeen } from "./FocusImg";

export type StageInfo = { stage: string; pct: number; thumb?: string; width?: number; height?: number };

type Props = {
  path: string;
  /** Final displayed aspect ratio, known before analysis, so the tile never changes size. */
  ratio: number;
  photo?: Photo;
  stage?: StageInfo;
  state: State | "";
  /** True right after an analysis: the picks are revealed one after another. */
  reveal: boolean;
  dim: boolean;
  /** Position and size in the wall, computed by the windowed layout. */
  box: { left: number; top: number; width: number; height: number };
  /** True when the current mark was set by the user (clicking it again clears it). */
  manual: boolean;
  /** Last photo of a group of similar shots: leaves a little extra room after it. */
  groupEnd: boolean;
  showScore: boolean;
  index: number;
  interactive: boolean;
  /** 分析中：第一次判出来也播动效。 */
  stamp?: boolean;
  /** Stable functions that take the photo's path, so a tile only re-renders when something about *it* changed. */
  onSelect: (path: string) => void;
  onOpen: (path: string) => void;
  onDecide: (path: string, s: State | null) => void;
};

/** The app's own mark: the four-point gleam from the logo. Filled = keep, outline = maybe. */
const STAR = "M0,-10 Q1.7,-1.7 10,0 Q1.7,1.7 0,10 Q-1.7,1.7 -10,0 Q-1.7,-1.7 0,-10Z";
export function Star({ filled, size = 20 }: { filled: boolean; size?: number }) {
  return (
    <svg className={`star ${filled ? "filled" : "outline"}`} viewBox="-12 -12 24 24" width={size} height={size} aria-hidden>
      <path d={STAR} pathLength={1} fill={filled ? "url(#fg-gold)" : "none"} stroke={filled ? "none" : "currentColor"} strokeWidth={1.6} strokeLinejoin="round" />
    </svg>
  );
}
/** Tiles that already had their entrance are not animated again when scrolling brings them back. */
const entered = new Set<string>();
const Cross = () => <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" aria-hidden><path d="M3 3l6 6M9 3L3 9" /></svg>;

/**
 * One photo. Before analysis finishes it is a "developing" tile (the real thumbnail comes into focus stage by
 * stage); afterwards it is the finished photo. The same element is used throughout, so nothing jumps.
 *
 * Marks are made of light rather than badges: a kept photo is *lit* (gold gleam, warm halo), a maybe is a hollow
 * gleam, a reject simply lets the light fall away. A glass dock under the cursor marks a photo in one click.
 */
function Tile({ path, ratio, photo, stage, state, reveal, dim, box, manual, groupEnd, showScore, index, interactive, stamp = false, onSelect, onOpen, onDecide }: Props) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const prev = useRef<State | "">(state);
  const [changed, setChanged] = useState<{ to: State; n: number } | null>(null);

  // Effects play when the user *changes* a photo's mark, not every time a view is re-drawn.
  // `stamp`：从"还没判"到有判定也算（分析中第一次判出来：星星弹出来、淘汰收一下光圈）；分析完的墙上不算，免得整面墙一起动
  useEffect(() => {
    if ((prev.current || stamp) && state && prev.current !== state) setChanged((c) => ({ to: state as State, n: (c?.n ?? 0) + 1 }));
    prev.current = state;
  }, [state, stamp]);

  // Only the first screenful animates in, once. Tiles mounted by scrolling appear at once: waiting for an entrance feels like waiting for the image.
  const firstTime = useRef(!entered.has(path) && index < 36).current;
  useEffect(() => { entered.add(path); }, [path]);
  const src = photo ? thumbSrc(photo) : stage?.thumb ? convertFileSrc(stage.thumb) : undefined;
  const stagger = Math.min(index, 16) * 0.012;
  const play = (s: State) => (changed?.to === s ? changed.n : 0) > 0 || reveal;
  const delay = reveal && !changed ? `${300 + Math.min(index, 40) * 55}ms` : "0ms";

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => { if (interactive && (e.key === "Enter")) onSelect(path); };
  const stop = (e: { stopPropagation: () => void }) => e.stopPropagation();

  return (
    <motion.div
      ref={ref} id={`t-${path}`} layout="position" role="button" tabIndex={interactive ? 0 : -1}
      className={`tile ${photo ? state : "pending"} ${!photo && stage ? "busy" : ""} ${interactive ? "live" : ""} ${groupEnd ? "gend" : ""} ${showScore ? "showscore" : ""}`}
      style={{ position: "absolute", left: box.left, top: box.top, width: box.width, height: box.height, ["--r" as string]: ratio }}
      initial={firstTime ? { opacity: 0, scale: 0.94 } : false}
      animate={{ opacity: dim ? 0.62 : 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.94 }}
      whileHover={interactive ? { opacity: 1 } : undefined}
      whileTap={interactive ? { scale: 0.985, transition: SPRING.tap } : undefined}
      transition={{ layout: SPRING.glide, opacity: { duration: DUR.slow, delay: stagger }, scale: { ...SPRING.soft, delay: stagger } }}
      onKeyDown={onKey}
      onClick={() => interactive && onSelect(path)}
      onDoubleClick={() => { if (interactive) { onSelect(path); onOpen(path); } }}
      aria-label={photo?.name ?? path.split("/").pop()}>
      <span className="tile-in">
        {photo && src && <FocusImg src={src} quick={interactive && !reveal} />}
        {!photo && src && (
          <img className="devimg" src={src} alt="" draggable={false} onLoad={() => markSeen(src)}
            style={{ filter: `blur(${Math.max(0, (100 - (stage?.pct ?? 0)) * 0.2).toFixed(1)}px) brightness(${(1 + (1 - (stage?.pct ?? 0) / 100) * 0.3).toFixed(2)})` }} />
        )}
        {/* 分析中的格子（M4.3 / R9）：没开始的只有底色；进行中的是模糊渐清的缩略图加一个进度环，没有扫光、呼吸边和文字 */}
        {!photo && stage && (
          <div className="pending-body">
            <svg className="ring" viewBox="0 0 40 40">
              <circle cx="20" cy="20" r="16" className="ring-bg" />
              <circle cx="20" cy="20" r="16" className="ring-fg" strokeDasharray={100.53} strokeDashoffset={100.53 * (1 - stage.pct / 100)} transform="rotate(-90 20 20)" />
            </svg>
          </div>
        )}

        {/* keep: a lit gleam, an anamorphic streak and a bloom */}
        {photo && state === "keep" && (
          <>
            {/* 星的出现：缩放 0.6 → 1、淡入，不转、不弹跳（R7） */}
            <motion.span key={`k${changed?.n ?? 0}${reveal}`} className="gmark keepmark" title={t("mark.keep")}
              initial={play("keep") ? { scale: 0.6, opacity: 0 } : false} animate={{ scale: 1, opacity: 1 }}
              transition={{ ...SPRING.tap, delay: reveal && !changed ? 0.35 + Math.min(index, 40) * 0.055 : 0 }}>
              <Star filled />
            </motion.span>
            {play("keep") && (
              <span key={`f${changed?.n ?? 0}${reveal}`} className="flare" aria-hidden style={{ ["--gd" as string]: delay }}><i className="streak" /><i className="bloom" /></span>
            )}
          </>
        )}
        {/* maybe: the same gleam, unlit */}
        {photo && state === "review" && (
          <motion.span key={`r${changed?.n ?? 0}`} className="gmark reviewmark" title={t("mark.review")}
            initial={changed?.to === "review" ? { scale: 0.6, opacity: 0 } : false} animate={{ scale: 1, opacity: 1 }} transition={SPRING.soft}>
            <Star filled={false} />
          </motion.span>
        )}
        {/* reject: no icon at all. The light falls away: an aperture closes for a moment, then the photo recedes */}
        {photo && state === "reject" && changed?.to === "reject" && <i key={`i${changed.n}`} className="iris" aria-hidden />}

        {/* 格子上的覆盖层最多两个（R10）：格式标签只在 ≥ 180px 宽的格子上 */}
        {photo && box.width >= 180 && <PhotoTags photo={photo} className="ontile" />}
        {photo && <span className="chip mono" data-tip={t("inspector.scoreChip", { score: pct(photo.score) })}>{pct(photo.score)}</span>}

        {interactive && (
          <span className="dock" onClick={stop} onDoubleClick={stop} onPointerMove={stop}>
            <button className={`dk keep ${state === "keep" ? "on" : ""}`} aria-label={t("mark.keepAct")} aria-pressed={state === "keep"}
              data-tip={`${state === "keep" && manual ? t("mark.clear") : t("mark.keepAct")} · P`}
              onClick={() => onDecide(path, state === "keep" && manual ? null : "keep")}>
              <Star filled size={18} />
            </button>
            <button className={`dk reject ${state === "reject" ? "on" : ""}`} aria-label={t("mark.rejectAct")} aria-pressed={state === "reject"}
              data-tip={`${state === "reject" && manual ? t("mark.clear") : t("mark.rejectAct")} · X`}
              onClick={() => onDecide(path, state === "reject" && manual ? null : "reject")}>
              <Cross />
            </button>
          </span>
        )}
        {interactive && (
          <button className="tzoom" aria-label={t("inspector.zoomLabel")} data-tip={`${t("inspector.zoomLabel")} · ${t("inspector.space")}`}
            onClick={(e) => { stop(e); onSelect(path); onOpen(path); }} onDoubleClick={stop} onPointerMove={stop}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9" /></svg>
          </button>
        )}
      </span>

    </motion.div>
  );
}

/** Memoised: with hundreds of tiles mounted, everything that changes elsewhere (a selection, a progress event) must not redraw all of them. */
export default memo(Tile);
