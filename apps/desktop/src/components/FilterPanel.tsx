import { memo, useLayoutEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Photo, State, thumbSrc } from "../types";
import { Star } from "./Tile";
import Ticker from "./Ticker";

/** 状态筛选：全部、三种判定，或"未处理"（既没手动判定也没打过星的）。 */
export type ViewMode = "all" | State | "untouched";
/** `total` is how many photos the scene has; `count` is how many of them match the other filters right now; `handled` 是 `count` 里已处理（手动判定或打过星）的几张。 */
export type SceneItem = { scene: number; cover: Photo; total: number; count: number; handled: number; text: string };
export type PersonItem = { person: number; photo: Photo; box: [number, number, number, number]; total: number; count: number };

const MODES: ViewMode[] = ["all", "keep", "review", "reject", "untouched"];

/** How many columns a grid has right now (they follow the panel's width), and so how many rows fit a given number of items. */
function useColumns(): [(el: HTMLDivElement | null) => void, number] {
  const [el, setEl] = useState<HTMLDivElement | null>(null);
  const [cols, setCols] = useState(2);
  // `el` is set when the grid actually appears (the people grid only exists once people were found), and observed from then on
  useLayoutEffect(() => {
    if (!el) return;
    const measure = () => setCols(Math.max(1, getComputedStyle(el).gridTemplateColumns.split(" ").filter(Boolean).length));
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [el]);
  return [setEl, cols];
}

/** A round crop of one face out of the photo's thumbnail (the thumbnail keeps the photo's proportions, so the box applies as is). */
export function FaceAvatar({ photo, box, size }: { photo: Photo; box: [number, number, number, number]; size: number }) {
  const [x, y, w, h] = box;
  const W = photo.width, H = photo.height;
  const side = Math.max(w * W, h * H) * 1.7;
  const k = size / side;
  return (
    <span className="fp-face" style={{ width: size, height: size }}>
      <img src={thumbSrc(photo)} alt="" draggable={false}
        style={{ width: W * k, height: H * k, left: size / 2 - (x + w / 2) * W * k, top: size / 2 - (y + h / 2) * H * k }} />
    </span>
  );
}

/**
 * The filters for a finished project, in one column beside the wall: what was decided (keep / review / reject),
 * who is in it and which scene it belongs to. They combine, and every number is what the wall would show if you picked
 * that entry now, so a filter never promises photos the others have already ruled out.
 */
function FilterPanel({ counts, mode, onMode, scenes, sceneFilter, onScene, people, personFilter, onPerson, onBrowseScenes, width, starFilter = null, onStar, starCounts, starExact, pairFilter = false, onPair, unpairedCount = 0, showPair = false }: {
  counts: Record<ViewMode, number>; mode: ViewMode; onMode: (m: ViewMode) => void;
  scenes: SceneItem[]; sceneFilter: number | null; onScene: (s: number | null) => void;
  people: PersonItem[]; personFilter: number | null; onPerson: (p: number | null) => void; onBrowseScenes: () => void; width: number;
  /** 手动星级筛选：只看 n 星及以上；`starCounts[n]` 是别的筛选都算上 n 星及以上有几张。没打过星的项目不显示这一栏。 */
  starFilter?: number | null; onStar?: (n: number | null) => void; starCounts?: number[]; starExact?: number[];
  /** "文件"一栏（只在项目里有成对或 RAW 照片时出现）：只看缺配对的；`unpairedCount` 是别的筛选都算上缺配对的有几张。 */
  pairFilter?: boolean; onPair?: (v: boolean) => void; unpairedCount?: number; showPair?: boolean;
}) {
  const { t } = useTranslation();
  const [allPeople, setAllPeople] = useState(false);
  const [sceneRef, sceneCols] = useColumns();
  const [peopleRef, peopleCols] = useColumns();
  // The panel shows two rows of scenes when narrow and three when it is wide enough for four or more columns; the rest is one click away.
  const sceneSlots = sceneCols * (sceneCols >= 4 ? 3 : 2);
  const peopleSlots = peopleCols * 2;

  // Biggest scenes first; the chosen one always stays visible even when it is small.
  // Entries the other filters have ruled out are left out rather than greyed: a row of ghosts says nothing.
  const liveScenes = scenes.filter((s) => s.count > 0 || s.scene === sceneFilter);
  const livePeople = people.filter((p) => p.count > 0 || p.person === personFilter);
  const shownPeople = allPeople ? livePeople : livePeople.slice(0, peopleSlots);
  const inPanel = liveScenes.slice(0, sceneSlots);
  const chosen = liveScenes.find((s) => s.scene === sceneFilter);
  if (chosen && !inPanel.includes(chosen)) inPanel[inPanel.length - 1] = chosen;

  return (
    <aside className="fpanel" style={{ width }} aria-label={t("cull.filters")}>
      <section>
        <h4>{t("cull.status")}</h4>
        <div className="fp-rows" role="tablist">
          {MODES.map((id) => (
            <button key={id} id={`fp-${id}`} role="tab" aria-selected={mode === id} className={`fp-row ${mode === id ? "on" : ""}`} onClick={() => onMode(id)}>
              <span className="fp-ico">
                {id === "keep" && <Star filled size={13} />}
                {id === "review" && <Star filled={false} size={13} />}
                {id === "reject" && <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden><path d="M3 3l6 6M9 3L3 9" /></svg>}
                {/* 未处理：一个空心圆，和三种判定的标记都不像 */}
                {id === "untouched" && <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden><circle cx="6" cy="6" r="4.2" /></svg>}
                {id === "all" && <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden><rect x="1.5" y="1.5" width="4" height="4" rx="1" /><rect x="6.5" y="1.5" width="4" height="4" rx="1" /><rect x="1.5" y="6.5" width="4" height="4" rx="1" /><rect x="6.5" y="6.5" width="4" height="4" rx="1" /></svg>}
              </span>
              <span>{t(`state.${id}`)}</span>
              <em className="mono"><Ticker value={counts[id]} /></em>
            </button>
          ))}
        </div>
      </section>

      {showPair && onPair && (
        <section>
          <h4>{t("cull.files")}{pairFilter && <button className="fp-clear" onClick={() => onPair(false)}>{t("cull.clearFilter")}</button>}</h4>
          <div className="fp-rows">
            <button role="switch" aria-checked={pairFilter} className={`fp-row ${pairFilter ? "on" : ""}`} onClick={() => onPair(!pairFilter)}>
              <span className="fp-ico">
                <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" aria-hidden><rect x="1.5" y="2.5" width="6" height="7.5" rx="1" /><path d="M9 4.5h1.5v6H5" strokeDasharray="1.6 1.4" /></svg>
              </span>
              <span>{t("cull.unpaired")}</span>
              <em className="mono"><Ticker value={unpairedCount} /></em>
            </button>
          </div>
        </section>
      )}

      {onStar && starCounts && (starCounts[1] > 0 || starFilter !== null) && (
        <section>
          <h4>{t("stars.label")}{starFilter !== null && <button className="fp-clear" onClick={() => onStar(null)}>{t("cull.clearFilter")}</button>}</h4>
          <div className="fp-rows" role="tablist">
            {[5, 4, 3, 2, 1].filter((n) => (starExact ?? starCounts)[n] > 0 || starFilter === n).map((n) => (
              <button key={n} role="tab" aria-selected={starFilter === n} className={`fp-row ${starFilter === n ? "on" : ""}`} onClick={() => onStar(starFilter === n ? null : n)}>
                <span className="fp-stars">{"★".repeat(n)}</span>
                <span>{t("stars.atLeast", { count: n })}</span>
                <em className="mono"><Ticker value={starCounts[n]} /></em>
              </button>
            ))}
          </div>
        </section>
      )}

      {people.length > 0 && (
        <section>
          <h4>{t("cull.people")}{personFilter !== null && <button className="fp-clear" onClick={() => onPerson(null)}>{t("cull.clearFilter")}</button>}</h4>
          {livePeople.length === 0 ? <p className="fp-none">{t("cull.noPeopleHere")}</p> : (
            <div className="fp-grid people" ref={peopleRef}>
              {shownPeople.map((p) => (
                <button key={p.person} className={`fp-person ${personFilter === p.person ? "on" : ""}`}
                  title={t("cull.personCount", { count: p.count })}
                  onClick={() => onPerson(personFilter === p.person ? null : p.person)}>
                  <FaceAvatar photo={p.photo} box={p.box} size={44} />
                  <span className="fp-count mono">{p.count}</span>
                </button>
              ))}
            </div>
          )}
          {livePeople.length > peopleSlots && (
            <button className="fp-more" onClick={() => setAllPeople((v) => !v)}>{allPeople ? t("cull.fewer") : t("cull.morePeople", { count: livePeople.length })}</button>
          )}
        </section>
      )}

      {liveScenes.length > 1 && (
        <section>
          <h4>{t("cull.scenes")}{sceneFilter !== null && <button className="fp-clear" onClick={() => onScene(null)}>{t("cull.clearFilter")}</button>}</h4>
          <div className="fp-grid" ref={sceneRef}>
            {inPanel.map((s) => (
              <button key={s.scene} className={`fp-scene ${sceneFilter === s.scene ? "on" : ""}`}
                onClick={() => onScene(sceneFilter === s.scene ? null : s.scene)}>
                <img src={thumbSrc(s.cover)} alt="" draggable={false} />
                {s.text && <span className="fp-words" title={s.text}>{s.text}</span>}
                {/* 角标：已处理 / 总数（一张都没处理时只有总数）；全部处理完用琥珀色 */}
                <span className={`fp-count mono ${s.count > 0 && s.handled >= s.count ? "done" : ""}`}>{s.handled > 0 ? `${s.handled}/${s.count}` : s.count}</span>
              </button>
            ))}
          </div>
          {liveScenes.length > inPanel.length && (
            <button className="fp-more" onClick={onBrowseScenes}>{t("cull.allScenes", { count: liveScenes.length })}</button>
          )}
        </section>
      )}

    </aside>
  );
}

/** Memoised: it is redrawn when its own numbers change, not whenever something else on the page does. */
export default memo(FilterPanel);
