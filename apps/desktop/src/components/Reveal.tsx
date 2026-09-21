import { ElementType, useMemo } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { SPRING } from "../motion";

/** Text that rises out of a mask word by word. Words come from Intl.Segmenter, so Chinese splits sensibly too. */
export default function Reveal({ text, delay = 0, as: Tag = "span", className = "" }: { text: string; delay?: number; as?: ElementType; className?: string }) {
  const { i18n } = useTranslation();
  const parts = useMemo(() => {
    try {
      const Seg = (Intl as unknown as { Segmenter: new (l: string, o: { granularity: string }) => { segment(t: string): Iterable<{ segment: string }> } }).Segmenter;
      return [...new Seg(i18n.language, { granularity: "word" }).segment(text)].map((s) => s.segment);
    } catch {
      return text.split(/(\s+)/);
    }
  }, [text, i18n.language]);
  let n = 0;
  return (
    <Tag className={className} aria-label={text}>
      {parts.map((w, i) =>
        w.trim() === "" ? (
          <span key={i} aria-hidden style={{ whiteSpace: "pre-wrap" }}>{w}</span>
        ) : (
          <span key={i} className="rv" aria-hidden>
            <motion.span className="rv-in" initial={{ y: "108%" }} animate={{ y: 0 }} transition={{ ...SPRING.soft, delay: delay + 0.04 * n++ }}>{w}</motion.span>
          </span>
        ),
      )}
    </Tag>
  );
}
