import { useEffect, useRef, type CSSProperties } from "react";
import { copy, release, type Lang } from "../content";
import Footer from "./Footer";
import { addScene, reduced, reveal, smooth, viewProg } from "./scroll";

// 五列照片，错开速度上下漂；竖幅和方幅按原尺寸给宽高，免得加载时跳动
const COLS = [["01", "06", "10"], ["04", "12", "02", "15"], ["07", "13", "16"], ["03", "09", "14", "05"], ["11", "08", "06"]];
const SIZE: Record<string, [number, number]> = { "04": [500, 500], "06": [420, 630], "13": [420, 630], "15": [420, 630] };
const SPEED = [-0.9, 0.55, -0.4, 0.75, -0.65];

/** 一句要求里用反引号括起来的命令画成等宽字。 */
const withCode = (s: string) => s.split("`").map((part, i) => (i % 2 ? <code key={i}>{part}</code> : part));

/** 收尾：背后几列照片错层漂动，中间一句邀请；发布之后换成下载按钮和安装说明。 */
export default function Closing({ lang }: { lang: Lang }) {
  const c = copy[lang].final;
  const sec = useRef<HTMLElement>(null), body = useRef<HTMLDivElement>(null), title = useRef<HTMLHeadingElement>(null), cols = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const off = reveal(body.current!);
    if (reduced()) return off;
    const colEls = Array.from(cols.current!.children) as HTMLElement[], t = title.current!;
    const stop = addScene(sec.current!, viewProg, (p) => {
      colEls.forEach((col, i) => { col.style.transform = `translateY(${(p - 0.5) * SPEED[i] * 900}px)`; });
      t.style.transform = `scale(${0.72 + 0.28 * smooth(0.15, 0.5, p)})`;
    });
    return () => { off(); stop(); };
  }, []);
  const d = (ms: number) => ({ "--d": `${ms}ms` }) as CSSProperties;
  return (
    <>
      <section ref={sec} className="cl" id="download" aria-labelledby="dl-title">
        <div className="cl-cols" ref={cols} aria-hidden>
          {COLS.map((col, i) => (
            <div key={i} className="cl-col">
              {col.map((n, j) => { const [w, h] = SIZE[n] ?? [600, 400]; return <img key={j} src={`/wall/${n}.webp`} width={w} height={h} alt="" loading="lazy" draggable={false} />; })}
            </div>
          ))}
        </div>
        <div className="wrap cl-body" ref={body}>
          <h2 id="dl-title" ref={title}><span className="ln"><span>{release.available ? c.readyTitle : c.title}</span></span></h2>
          <p className="lead rise" style={d(160)}>{release.available ? c.readyBody : c.body}</p>
          {release.available ? (
            <div className="cl-btns rise" style={d(240)}>
              <a className="btn btn-primary" href={release.download}>{c.download}</a>
              {/* 安卓直接下载 APK（Release 里固定的文件名），不用再到 Release 页里找 */}
              <a className="btn btn-primary" href={release.android}>{c.downloadAndroid}</a>
              <a className="btn btn-secondary" href={release.repo}>{c.source}</a>
            </div>
          ) : (
            <p className="status rise" style={d(240)}><span className="dot" aria-hidden />{c.label}</p>
          )}
          <div className="reqs rise" style={d(300)}>
            {(release.available ? c.readyReqs : c.reqs).map((r) => <p key={r}>{withCode(r)}</p>)}
          </div>
        </div>
      </section>
      <Footer lang={lang} />
    </>
  );
}
