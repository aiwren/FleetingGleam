import { useTranslation } from "react-i18next";
import { Photo, photoTags } from "../types";

/** Small labels for what a photo is beyond a plain JPEG: RAW (and which RAW), HEIC and the like, HDR, a motion photo, a long shape. */
export default function PhotoTags({ photo, className = "", skip = [] }: { photo: Photo; className?: string; skip?: string[] }) {
  const { t } = useTranslation();
  const tags = photoTags(photo).filter((g) => !skip.includes(g.id));
  if (tags.length === 0) return null;
  return (
    <span className={`ptags ${className}`}>
      {tags.map((g) => (
        <span key={g.id} className={`ptag ${g.id}`} title={t(`tag.${g.id}Tip`, { defaultValue: "" })}>
          {g.id === "motion" && <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3" aria-hidden><circle cx="6" cy="6" r="1.6" fill="currentColor" stroke="none" /><circle cx="6" cy="6" r="4.6" strokeDasharray="1.6 1.6" /></svg>}
          {g.text ?? t(`tag.${g.id}`)}
        </span>
      ))}
    </span>
  );
}
