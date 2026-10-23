import { memo, useLayoutEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Photo, State, thumbSrc } from "../types";
import { Star } from "./Tile";
import Ticker from "./Ticker";

export type ViewMode = "all" | State;
/** `total` is how many photos the scene has; `count` is how many of them match the other filters right now. */
export type SceneItem = { scene: number; cover: Photo; total: number; count: number; text: string };
export type PersonItem = { person: number; photo: Photo; box: [number, number, number, number]; total: number; count: number };

const MODES: ViewMode[] = ["all", "keep", "review", "reject"];

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
function FilterPanel({ counts, mode, onMode, scenes, sceneFilter, onScene, people, personFilter, onPerson, onBrowseScenes, width }: {
  counts: Record<ViewMode, number>; mode: ViewMode; onMode: (m: ViewMode) => void;
  scenes: SceneItem[]; sceneFilter: number | null; onScene: (s: number | null) => void;
  people: PersonItem[]; personFilter: number | null; onPerson: (p: number | null) => void; onBrowseScenes: () => void; width: number;
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
                {id === "all" && <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden><rect x="1.5" y="1.5" width="4" height="4" rx="1" /><rect x="6.5" y="1.5" width="4" height="4" rx="1" /><rect x="1.5" y="6.5" width="4" height="4" rx="1" /><rect x="6.5" y="6.5" width="4" height="4" rx="1" /></svg>}
              </span>
              <span>{t(`state.${id}`)}</span>
              <em className="mono"><Ticker value={counts[id]} /></em>
            </button>
          ))}
        </div>
      </section>

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
                <span className="fp-count mono">{s.count}</span>
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
