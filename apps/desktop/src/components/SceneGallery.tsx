import { useTranslation } from "react-i18next";
import { Photo, thumbSrc } from "../types";

export type SceneGroup = { scene: number; photos: Photo[]; text: string };

/**
 * Every scene at a size you can actually recognise: the best shot large, a few more of the scene beneath it.
 * It replaces the wall while open (biggest scenes first); picking one filters the wall to it and goes back to the photos.
 */
export default function SceneGallery({ groups, chosen, describing, canDescribe, onDescribe, onPick, onBack }: {
  groups: SceneGroup[]; chosen: number | null; onPick: (scene: number) => void; onBack: () => void;
  /** Progress of writing the descriptions, or null when none is being written. */
  describing: { done: number; total: number } | null; canDescribe: boolean; onDescribe: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="scene-gallery">
      <div className="sg-head">
        <h3>{t("cull.scenes")} <span className="mono ash">{groups.length}</span></h3>
        <div className="sg-actions">
          {describing ? <span className="ash mono">{t("cull.describing", { done: describing.done, total: describing.total })}</span>
            : canDescribe && <button className="btn sm" onClick={onDescribe}>{t("cull.describe")}</button>}
          <button className="btn sm" onClick={onBack}>{t("cull.backToPhotos")}</button>
        </div>
      </div>
      <div className="sg-grid">
        {groups.map((g) => (
          <button key={g.scene} className={`sg-card ${chosen === g.scene ? "on" : ""}`} onClick={() => onPick(g.scene)}>
            <span className="sg-cover">
              <img src={thumbSrc(g.photos[0])} alt="" loading="lazy" draggable={false} />
              {g.text && <span className="sg-words">{g.text}</span>}
              <span className="sg-count mono">{t("cull.sceneCount", { count: g.photos.length })}</span>
            </span>
            {g.photos.length > 1 && (
              <span className="sg-strip">
                {g.photos.slice(1, 5).map((p, i) => <img key={`${i}-${p.path}`} src={thumbSrc(p)} alt="" loading="lazy" draggable={false} />)}
              </span>
            )}
          </button>
        ))}
      </div>
    </div>
  );
}
