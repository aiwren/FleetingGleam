import { useEffect, useRef, type CSSProperties } from "react";
import { copy, type Lang } from "../content";
import { addScene, reduced, reveal, smooth, viewProg } from "./scroll";

/** 评分是参考：两行字从两侧滑进来在中间对上，再说一句为什么。 */
export default function Honest({ lang }: { lang: Lang }) {
  const c = copy[lang].honest;
  const sec = useRef<HTMLElement>(null), body = useRef<HTMLDivElement>(null), l1 = useRef<HTMLSpanElement>(null), l2 = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const off = reveal(body.current!);
    if (reduced()) return off;
    const a1 = l1.current!, a2 = l2.current!;
    const stop = addScene(sec.current!, viewProg, (p) => {
      const a = smooth(0.12, 0.42, p), b = smooth(0.16, 0.46, p);
      a1.style.transform = `translateX(${(1 - a) * -45}vw) scale(${1.25 - 0.25 * a})`;
      a2.style.transform = `translateX(${(1 - b) * 45}vw) scale(${1.25 - 0.25 * b})`;
      a1.style.opacity = String(0.15 + 0.85 * a); a2.style.opacity = String(0.15 + 0.85 * b);
    });
    return () => { off(); stop(); };
  }, []);
  return (
    <section ref={sec} className="honest" aria-labelledby="honest-title">
      <div className="wrap" ref={body}>
        <h2 id="honest-title">
          <span className="hx" ref={l1}>{c.title[0]}</span>
          <span className="hx quiet" ref={l2}>{c.title[1]}</span>
        </h2>
        <p className="lead rise" style={{ "--d": "320ms" } as CSSProperties}>{c.body}</p>
      </div>
    </section>
  );
}
