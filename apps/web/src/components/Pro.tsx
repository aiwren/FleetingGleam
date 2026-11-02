import { useEffect, useRef, type CSSProperties } from "react";
import { copy, type Lang } from "../content";
import { Ic } from "./Icons";
import { reveal } from "./scroll";

/** 给专业摄影师：三张卡（RAW + JPG 配对、写回 Lightroom、手动星级），滚到视野里依次升起。 */
export default function Pro({ lang }: { lang: Lang }) {
  const c = copy[lang].pro;
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => reveal(body.current!), []);
  return (
    <section className="pro" id="pro" aria-labelledby="pro-title">
      <div className="wrap" ref={body}>
        <p className="eyebrow">{c.eyebrow}</p>
        <h2 className="h2 rise" id="pro-title" style={{ marginTop: "var(--s-3)" }}>{c.title}</h2>
        <p className="lead rise" style={{ "--d": "100ms" } as CSSProperties}>{c.lead}</p>
        <ul className="pro-cards">
          {c.items.map(([icon, title, text], i) => (
            <li key={title} className="rise" style={{ "--d": `${220 + i * 90}ms` } as CSSProperties}>
              <Ic id={icon} />
              <h3>{title}</h3>
              <p>{text}</p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
