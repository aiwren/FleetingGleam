import { Fragment, useEffect, useRef } from "react";
import { copy, type Lang } from "../content";
import { Badge, Ic } from "./Icons";
import { addScene, clamp, lerp, pinProg, reduced, smooth } from "./scroll";

const HOW = ["01", "02", "03", "05", "07", "08", "09", "10"];
const DEC = ["P", "X", "P", "R", "X", "P", "X", "P"]; // 示例：保留 4、淘汰 3、待复查 1
const KEYS = ["P", "R", "X"];

/** 一步的说明里 {keys} 的位置画成三个按键。 */
function withKeys(text: string) {
  const [a, b] = text.split("{keys}");
  if (b === undefined) return text;
  return <>{a}{KEYS.map((k, i) => <Fragment key={k}>{i > 0 && " "}<kbd>{k}</kbd></Fragment>)}{b}</>;
}

/**
 * 怎么用：一扇示例应用窗口跟着滚动走完三步。照片逐张显影、进度条走满；选框逐张移动、按下对应的键；
 * 最后保留的飞进"保留"，淘汰的飞进废纸篓，待复查的留在原地。
 */
export default function How({ lang }: { lang: Lang }) {
  const c = copy[lang].flow;
  const sec = useRef<HTMLElement>(null), steps = useRef<HTMLOListElement>(null), grid = useRef<HTMLDivElement>(null), ring = useRef<HTMLSpanElement>(null);
  const status = useRef<HTMLSpanElement>(null), prog = useRef<HTMLSpanElement>(null), foot = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const reduce = reduced();
    const winGrid = grid.current!, rg = ring.current!, winStatus = status.current!, winProg = prog.current!, f = foot.current!;
    const destKeep = f.querySelector<HTMLElement>("[data-dest=keep]")!, destTrash = f.querySelector<HTMLElement>("[data-dest=trash]")!;
    const nKeep = destKeep.querySelector<HTMLElement>(".mono")!, nTrash = destTrash.querySelector<HTMLElement>(".mono")!;
    const keyEls = Object.fromEntries(Array.from(f.querySelectorAll<HTMLElement>(".keys kbd")).map((k) => [k.dataset.k!, k]));
    const howSteps = Array.from(steps.current!.querySelectorAll<HTMLElement>(".how-step"));
    const tiles = Array.from(winGrid.querySelectorAll<HTMLElement>(".ph"));
    type Pt = { x: number; y: number };
    let hg: { tiles: (Pt & { w: number; h: number; cx: number; cy: number })[]; keep: Pt; trash: Pt } | null = null;
    function layout() {
      tiles.forEach((t) => { t.style.transform = ""; });
      const gr = winGrid.getBoundingClientRect(), kr = destKeep.getBoundingClientRect(), tr = destTrash.getBoundingClientRect();
      hg = {
        tiles: tiles.map((t) => { const r = t.getBoundingClientRect(); return { x: r.left - gr.left, y: r.top - gr.top, w: r.width, h: r.height, cx: r.left + r.width / 2, cy: r.top + r.height / 2 }; }),
        keep: { x: kr.left + kr.width / 2, y: kr.top + kr.height / 2 },
        trash: { x: tr.left + tr.width / 2, y: tr.top + tr.height / 2 },
      };
    }
    function render(p: number) {
      if (!hg) layout();
      const g = hg!;
      const step = p < 0.34 ? 0 : p < 0.67 ? 1 : 2;
      howSteps.forEach((li, i) => li.classList.toggle("is-on", i === step));
      // 第 1 步：照片逐张"显影"出现，进度条走满
      const a = clamp((p - 0.02) / 0.28, 0, 1);
      winProg.style.transform = `scaleX(${a})`;
      // 第 2 步：选框逐张移动，按下对应按键，结果落在照片上
      const b = clamp((p - 0.36) / 0.27, 0, 1) * HOW.length;
      const cur = Math.min(HOW.length - 1, Math.floor(b));
      const showRing = b > 0 && step === 1;
      rg.style.opacity = showRing ? "1" : "0";
      if (showRing) { const t = g.tiles[cur]; rg.style.width = `${t.w + 8}px`; rg.style.height = `${t.h + 8}px`; rg.style.transform = `translate(${t.x - 4}px, ${t.y - 4}px)`; }
      const fr = b - Math.floor(b), down = step === 1 && fr > 0.4 && fr < 0.75 ? DEC[cur] : null;
      Object.entries(keyEls).forEach(([k, el]) => el.classList.toggle("is-down", k === down));
      // 第 3 步：保留的飞进「保留」，淘汰的飞进废纸篓
      const ci = smooth(0.68, 0.94, p);
      let kept = 0, trashed = 0, appeared = 0;
      tiles.forEach((t, i) => {
        const ai = smooth(i * 0.09, i * 0.09 + 0.3, a);
        if (ai > 0.5) appeared++;
        const decided = b >= i + 0.6, d = DEC[i];
        t.classList.toggle("is-kept", decided && d === "P");
        t.classList.toggle("is-rejected", decided && d === "X");
        t.classList.toggle("is-review", decided && d === "R");
        let sc = lerp(1.1, 1, ai) * (decided && d === "X" ? 0.9 : 1), op = ai * (decided && d === "X" ? 0.55 : 1), dx = 0, dy = 0;
        if (decided && d !== "R" && ci > 0) {
          const to = d === "P" ? g.keep : g.trash, s0 = g.tiles[i];
          const fl = smooth(i * 0.035, 0.7 + i * 0.035, ci);
          dx = (to.x - s0.cx) * fl; dy = (to.y - s0.cy) * fl;
          sc = lerp(sc, 0.14, fl); op *= 1 - smooth(0.8, 1, fl);
          if (fl > 0.95) { if (d === "P") kept++; else trashed++; }
        }
        t.style.transform = `translate(${dx}px, ${dy}px) scale(${sc})`;
        t.style.opacity = String(op);
        t.style.filter = ai < 1 ? `blur(${(1 - ai) * 10}px) brightness(${1 + (1 - ai) * 0.35})` : "";
      });
      nKeep.textContent = String(kept); nTrash.textContent = String(trashed);
      destKeep.classList.toggle("is-hit", kept > 0); destTrash.classList.toggle("is-hit", trashed > 0);
      winStatus.textContent = step === 0 ? (a > 0 ? `${c.analysing} ${appeared} / ${HOW.length}` : c.ready)
        : step === 1 ? `${c.photo} ${cur + 1} ${c.of} ${HOW.length}${c.unit}`
        : `${c.keptN} ${kept}${c.unit} · ${c.trashN} ${trashed}${c.unit}`;
    }
    const onResize = () => { hg = null; if (reduce) render(0.66); };
    addEventListener("resize", onResize);
    let stop = () => {};
    if (reduce) render(0.66); else stop = addScene(sec.current!, pinProg, render);
    return () => { stop(); removeEventListener("resize", onResize); };
  }, [c]);

  return (
    <section ref={sec} className="how" id="flow" aria-labelledby="flow-title">
      <div className="pin how-pin">
        <div className="wrap how-inner">
          <div>
            <p className="eyebrow">{c.eyebrow}</p>
            <h2 className="h2" id="flow-title" style={{ marginTop: "var(--s-3)" }}>{c.title}</h2>
            <ol className="how-steps" ref={steps}>
              {c.steps.map(([t, d], i) => (
                <li key={t} className={`how-step${i === 0 ? " is-on" : ""}`}><span className="mono">{String(i + 1).padStart(2, "0")}</span><div><h3>{t}</h3><p>{withKeys(d)}</p></div></li>
              ))}
            </ol>
          </div>
          <figure className="win" aria-label={c.aria}>
            <div className="win-bar">
              <span className="win-dots" aria-hidden><i /><i /><i /></span>
              <span className="win-title">{c.folder}</span>
              <span className="win-status" ref={status}>{c.ready}</span>
            </div>
            <div className="win-prog" aria-hidden><span ref={prog} /></div>
            <div className="win-grid" ref={grid}>
              {HOW.map((n, i) => (
                <div key={n} className="ph">
                  <img src={`/wall/${n}.webp`} width={600} height={400} alt={`${c.photoAlt} ${i + 1}`} draggable={false} />
                  <Badge label={c.kept} />
                  <span className="tag">{DEC[i] === "X" ? c.reject : c.review}</span>
                </div>
              ))}
              <span className="win-ring" ref={ring} aria-hidden />
            </div>
            <div className="win-foot" ref={foot}>
              <span className="dest" data-dest="trash"><Ic id="trash" />{c.trash}<span className="mono">0</span></span>
              <span className="keys" aria-hidden>{KEYS.map((k) => <kbd key={k} data-k={k}>{k}</kbd>)}</span>
              <span className="dest" data-dest="keep"><Ic id="folder" />{c.kept}<span className="mono">0</span></span>
            </div>
          </figure>
        </div>
      </div>
    </section>
  );
}
