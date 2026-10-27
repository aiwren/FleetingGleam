import { useEffect, useRef, type CSSProperties } from "react";
import { copy, type Lang } from "../content";
import { Ic } from "./Icons";
import { addScene, lerp, reduced, reveal, viewProg } from "./scroll";

/** 隐私：一句话写得很大，一张照片透过字形露出来，滚动时照片在字里慢慢移动。下面是那句承诺和三条要点。 */
export default function Privacy({ lang }: { lang: Lang }) {
  const c = copy[lang].privacy;
  const sec = useRef<HTMLElement>(null), title = useRef<HTMLHeadingElement>(null), body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const off = reveal(body.current!);
    if (reduced()) return off;
    const t = title.current!;
    const stop = addScene(sec.current!, viewProg, (p) => { t.style.setProperty("--py", `${lerp(15, 85, p).toFixed(2)}%`); });
    return () => { off(); stop(); };
  }, []);
  return (
    <section ref={sec} className="pv" id="privacy" aria-labelledby="pv-title">
      <div className="wrap">
        <p className="eyebrow">{c.eyebrow}</p>
        <h2 ref={title} className="pv-title" id="pv-title">{c.title}</h2>
        <div ref={body}>
          <p className="lead rise" style={{ "--d": "120ms" } as CSSProperties}>{c.text}</p>
          <ul className="pv-points">
            {c.points.map(([icon, label], i) => (
              <li key={label} className="rise" style={{ "--d": `${240 + i * 80}ms` } as CSSProperties}><Ic id={icon} />{label}</li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}
