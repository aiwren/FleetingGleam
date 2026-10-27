import { useEffect, useRef, type CSSProperties } from "react";
import { altOf, copy, type Lang } from "../content";
import { Badge, Ic } from "./Icons";
import { EASE, EASE_IN, addScene, clamp, forceRender, glint, lerp, localProg, pinProg, reduced, smooth, viewProg, wait } from "./scroll";

const pad2 = (n: number) => String(n).padStart(2, "0");
const src = (k: string) => `/photos/${k}.webp`;

// 进度 → 面板位置：每块面板到位后停留一段供阅读
const KEYS = [[0, 0], [0.14, 0], [0.34, 1], [0.46, 1], [0.66, 2], [0.78, 2], [0.96, 3], [1, 3]] as const;
const P_FOR = [0.07, 0.4, 0.72, 0.98];
function posAt(p: number) {
  for (let i = 1; i < KEYS.length; i++) {
    const [p0, v0] = KEYS[i - 1], [p1, v1] = KEYS[i];
    if (p <= p1) return v0 === v1 ? v0 : lerp(v0, v1, smooth(p0, p1, p));
  }
  return 3;
}

// 面板 1：三组连拍，每组四张；BEST 是每组里最好的那一张
const GROUPS = [["wd0", "wd1", "wd2", "wd3"], ["st0", "st1", "st2", "st3"], ["field", "village", "boat", "forest"]];
const BEST = [1, 1, 0];
// 带一点过冲的落位，像被甩到桌面上
const backOut = (x: number) => { const c1 = 1.5, c3 = c1 + 1; return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2); };

type Card = { el: HTMLElement; j: number; k: number; best: boolean; kept: boolean; sx: number; sy: number; sr: number; tx: number; ty: number; tr: number; ts: number };

/**
 * 四个功能，宽屏上钉住横着走：每块面板到位后停一会儿，滚得越快长廊越斜；窄屏或减少动态效果时竖着排，各自经过窗口时演示。
 * 分组和场景两块演示由滚动推着走；搜索和口味两块可以直接上手。
 */
export default function Features({ lang }: { lang: Lang }) {
  const c = copy[lang].features;
  const sec = useRef<HTMLElement>(null), pin = useRef<HTMLDivElement>(null), track = useRef<HTMLDivElement>(null), progress = useRef<HTMLOListElement>(null);
  const grp = useRef<HTMLDivElement>(null), grpLabels = useRef<HTMLParagraphElement>(null);
  const reel = useRef<HTMLOListElement>(null), reelBox = useRef<HTMLDivElement>(null), ruleK = useRef<HTMLParagraphElement>(null), ruleR = useRef<HTMLParagraphElement>(null), rule = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const reduce = reduced();
    const wideMQ = matchMedia("(min-width: 1024px) and (prefers-reduced-motion: no-preference)");
    const hz = sec.current!, hzPin = pin.current!, tr = track.current!, g = grp.current!;
    const hzBars = Array.from(progress.current!.querySelectorAll<HTMLElement>(".fill")), hzItems = Array.from(progress.current!.querySelectorAll("li"));
    const hzPanels = Array.from(tr.querySelectorAll<HTMLElement>(".hz-panel")).map((el) => ({ text: el.querySelector<HTMLElement>(".hz-text")!, demo: el.querySelector<HTMLElement>(".hz-inner > :last-child")! }));

    // ---- 分组 ----
    const cards: Card[] = Array.from(g.querySelectorAll<HTMLElement>(".card")).map((el, i) => {
      const j = Math.floor(i / 4), k = i % 4;
      return { el, j, k, best: k === BEST[j], kept: false, sx: 0, sy: 0, sr: 0, tx: 0, ty: 0, tr: 0, ts: 1 };
    });
    let gW = 0, gH = 0;
    function layoutGroup() {
      gW = g.clientWidth; gH = g.clientHeight;
      const cw = gW * 0.28, ch = cw * 2 / 3;
      cards.forEach((cd, i) => {
        // 打散：确定性的位置与角度，范围超出画框，飞入时更有冲击
        cd.sx = ((i * 0.379 + 0.05) % 1) * gW * 1.6 - gW * 0.3 - cw / 2;
        cd.sy = ((i * 0.618 + 0.1) % 1) * gH * 1.7 - gH * 0.35 - ch / 2;
        cd.sr = (((i * 53) % 9) - 4) * 7;
        // 归叠：每组 3 张在后，最好的一张在最前
        const base = gW * (0.015 + cd.j * 0.335);
        if (cd.best) { cd.tx = base + 18; cd.ty = gH / 2 - ch / 2 + 14; cd.tr = 0; cd.ts = 1.04; cd.el.style.zIndex = "5"; }
        else {
          const q = [0, 1, 2, 3].filter((x) => x !== BEST[cd.j]).indexOf(cd.k);
          cd.tx = base + q * 9; cd.ty = gH / 2 - ch / 2 - (2 - q) * 9 + 14; cd.tr = (q - 1) * 3; cd.ts = 0.96; cd.el.style.zIndex = String(q + 1);
        }
      });
    }
    function renderGroup(d: number) {
      if (!gW) layoutGroup();
      cards.forEach((cd) => {
        const lp = clamp((d - cd.k * 0.04 - cd.j * 0.07) / 0.68, 0, 1), e = backOut(lp);
        const x = lerp(cd.sx, cd.tx, e), y = lerp(cd.sy, cd.ty, e), r = lerp(cd.sr, cd.tr, e), s = lerp(1.3, cd.ts, e);
        cd.el.style.transform = `translate(${x}px, ${y}px) rotate(${r}deg) scale(${s})`;
        cd.el.style.opacity = String(clamp(lp * 3, 0, 1) * (cd.best ? 1 : lerp(1, 0.75, lp)));
        const kept = cd.best && d > 0.92;
        if (kept !== cd.kept) { cd.kept = kept; cd.el.classList.toggle("is-kept", kept); if (kept) glint(cd.el); }
      });
      grpLabels.current!.style.opacity = String(smooth(0.7, 0.95, d));
    }

    // ---- 场景滚轮 ----
    const reelItems = Array.from(reel.current!.querySelectorAll("li"));
    let reelOn = -1;
    function renderReel(f: number) {
      const idx = clamp(f, 0, c.b.kinds.length - 1);
      reel.current!.style.setProperty("--idx", idx.toFixed(3));
      const on = Math.round(idx);
      if (on === reelOn) return;
      reelItems.forEach((li, i) => li.classList.toggle("is-on", i === on));
      reelOn = on;
      ruleK.current!.textContent = `${pad2(on + 1)} · ${c.b.kinds[on]}`;
      ruleR.current!.textContent = c.b.rules[on] ?? c.b.other;
      if (!reduce) rule.current!.animate([{ opacity: 0.2, transform: "translateY(6px)" }, { opacity: 1, transform: "none" }], { duration: 240, easing: EASE });
    }

    // ---- 长廊 ----
    let hzPos = 0, pw = 0;
    function renderHz(p: number, v = 0) {
      if (wideMQ.matches) {
        if (hzPin.clientWidth !== pw) { pw = hzPin.clientWidth; tr.style.setProperty("--pw", `${pw}px`); }
        hzPos = posAt(p);
        // 滚得越快，长廊越斜；停下即回正
        const skew = clamp(v * -220, -8, 8);
        tr.style.transform = `translateX(${-hzPos * pw}px) skewX(${skew}deg)`;
        // 景深：文字和演示以不同速度进出
        hzPanels.forEach((pn, i) => {
          const d = clamp(i - hzPos, -1, 1), a = Math.abs(d);
          pn.text.style.transform = `translateX(${d * 220}px)`;
          pn.text.style.opacity = String(1 - a * 0.7);
          pn.demo.style.transform = `translateX(${d * -80}px) scale(${1 - a * 0.2})`;
          pn.demo.style.opacity = String(1 - a * 0.5);
        });
        hzBars.forEach((b, i) => { b.style.transform = `scaleX(${clamp(hzPos - i + 1, 0, 1)})`; });
        hzItems.forEach((li, i) => li.classList.toggle("is-on", Math.round(hzPos) === i));
        renderGroup(clamp(p / 0.13, 0, 1));
        renderReel(clamp((p - 0.26) / 0.22, 0, 1) * (c.b.kinds.length - 1));
      } else {
        tr.style.transform = "";
        hzPanels.forEach((pn) => { pn.text.style.transform = pn.demo.style.transform = ""; pn.text.style.opacity = pn.demo.style.opacity = ""; });
        renderGroup(reduce ? 1 : localProg(g));
        renderReel(reduce ? 0 : localProg(reelBox.current!, 0.95, 0.75) * (c.b.kinds.length - 1));
      }
    }

    // 键盘焦点进入屏外面板时，滚动到该面板
    const onFocus = (e: FocusEvent) => {
      if (!wideMQ.matches) return;
      const panel = (e.target as HTMLElement).closest<HTMLElement>(".hz-panel"); if (!panel) return;
      const i = +panel.dataset.panel!;
      if (Math.round(hzPos) === i) return;
      scrollTo({ top: hz.offsetTop + P_FOR[i] * (hz.offsetHeight - innerHeight), behavior: "auto" });
    };
    tr.addEventListener("focusin", onFocus);
    const onMQ = () => { gW = 0; pw = 0; forceRender(); };
    wideMQ.addEventListener("change", onMQ);
    const onResize = () => { gW = 0; pw = 0; if (reduce) renderHz(1); };
    addEventListener("resize", onResize);
    let stop = () => {};
    if (reduce) renderHz(1);
    else stop = addScene(hz, (el) => (wideMQ.matches ? pinProg(el) : viewProg(el)), renderHz);
    return () => { stop(); tr.removeEventListener("focusin", onFocus); wideMQ.removeEventListener("change", onMQ); removeEventListener("resize", onResize); };
  }, [c]);

  const idx = (i: number) => <p className="hz-idx">{pad2(i + 1)} / 04</p>;
  return (
    <section ref={sec} className="hz" id="features" aria-label={c.label}>
      <div className="hz-pin" ref={pin}>
        <div className="hz-track" ref={track}>

          <article className="hz-panel" data-panel="0" aria-labelledby="f1-title">
            <div className="wrap hz-inner">
              <div className="hz-text">
                {idx(0)}
                <h2 className="h2" id="f1-title">{c.a.t}</h2>
                <p className="lead">{c.a.d}</p>
              </div>
              <div>
                <div className="grp" ref={grp} role="group" aria-label={c.a.aria}>
                  {GROUPS.flat().map((k) => (
                    <div key={k} className="ph card"><img src={src(k)} width={720} height={480} alt={altOf(lang, k)} draggable={false} /><Badge label={c.a.best} /></div>
                  ))}
                </div>
                <p className="grp-labels" ref={grpLabels}>
                  {[1, 2, 3].map((n) => <span key={n}>{c.a.group.replace("{n}", String(n))}<i className="grp-n"> · {c.a.frames}</i></span>)}
                </p>
              </div>
            </div>
          </article>

          <article className="hz-panel" data-panel="1" aria-labelledby="f2-title">
            <div className="wrap hz-inner">
              <div className="hz-text">
                {idx(1)}
                <h2 className="h2" id="f2-title">{c.b.t}</h2>
                <p className="lead">{c.b.d}</p>
              </div>
              <div className="reel-box">
                <div className="reel" ref={reelBox} aria-hidden>
                  <ol className="reel-list" ref={reel}>
                    {c.b.kinds.map((k, i) => <li key={k} className={i === 0 ? "is-on" : undefined}><span className="mono">{pad2(i + 1)}</span>{k}</li>)}
                  </ol>
                </div>
                <div className="reel-rule" ref={rule} aria-hidden>
                  <p className="k" ref={ruleK}>01 · {c.b.kinds[0]}</p>
                  <p className="r" ref={ruleR}>{c.b.rules[0]}</p>
                </div>
                <p className="sr-only">{c.b.sr}</p>
              </div>
            </div>
          </article>

          <article className="hz-panel" data-panel="2" aria-labelledby="f3-title">
            <div className="wrap hz-inner">
              <div className="hz-text">
                {idx(2)}
                <h2 className="h2" id="f3-title">{c.c.t}</h2>
                <p className="lead">{c.c.d}</p>
              </div>
              <Search lang={lang} />
            </div>
          </article>

          <article className="hz-panel" data-panel="3" aria-labelledby="f4-title">
            <div className="wrap hz-inner">
              <div className="hz-text">
                {idx(3)}
                <h2 className="h2" id="f4-title">{c.d.t}</h2>
                <p className="lead">{c.d.d}</p>
              </div>
              <Taste lang={lang} />
            </div>
          </article>

        </div>
        <ol className="hz-progress wrap" ref={progress} aria-hidden>
          {c.progress.map((l) => <li key={l}><span>{l}</span><span className="track"><span className="fill" /></span></li>)}
        </ol>
      </div>
    </section>
  );
}

// ---- 面板 3：搜索 ----
const LIB = ["field", "boat", "village", "forest", "st0", "st1", "st2", "st3", "wd0", "wd1", "wd2", "wd3"];
/** 示例照片的关键词，中英文都认，哪种语言的页面都能用另一种语言搜。 */
const WORDS: Record<string, string[]> = {
  field: ["田野", "日落", "夕阳", "黄昏", "草地", "sunset", "field", "dusk", "meadow"],
  boat: ["船", "水面", "湖", "海", "boat", "water", "sea", "lake"],
  village: ["房屋", "房子", "白色", "山坡", "村", "house", "village", "hill", "white"],
  forest: ["森林", "溪", "树", "绿", "forest", "stream", "tree", "green", "wood"],
  st: ["街", "城市", "路人", "street", "city"],
  wd: ["婚礼", "新人", "新娘", "婚纱", "wedding", "bride"],
};
function matchKeys(text: string) {
  const t = text.trim().toLowerCase();
  if (!t) return null;
  const hit = new Set<string>();
  LIB.forEach((k) => { const words = WORDS[k] || WORDS[k.slice(0, 2)]; if (words.some((w) => t.includes(w))) hit.add(k); });
  return hit;
}

function Search({ lang }: { lang: Lang }) {
  const c = copy[lang].features, s = c.c;
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const root = box.current!;
    const q = root.querySelector<HTMLInputElement>("input")!, qClear = root.querySelector<HTMLButtonElement>(".icon-btn")!;
    const results = Array.from(root.querySelectorAll<HTMLElement>(".results .ph")), empty = root.querySelector<HTMLElement>(".empty")!, countEl = root.querySelector<HTMLElement>("[data-count]")!;
    const sugBtns = Array.from(root.querySelectorAll<HTMLButtonElement>(".suggest button"));
    let run = 0, debounce = 0;
    async function apply(text: string) {
      const me = ++run, reduce = reduced();
      const keys = matchKeys(text);
      qClear.hidden = !text;
      sugBtns.forEach((b) => b.setAttribute("aria-pressed", String(b.textContent === text.trim())));
      const want = (t: HTMLElement) => !keys || keys.has(t.dataset.key!);
      const leaving = results.filter((t) => !t.hidden && !want(t));
      if (!reduce && leaving.length) {
        await Promise.all(leaving.map((t) => t.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: "scale(.94)" }], { duration: 200, easing: EASE_IN, fill: "forwards" }).finished.catch(() => {})));
        if (me !== run) return;
      }
      const first = new Map(results.map((t) => [t, t.hidden ? null : t.getBoundingClientRect()]));
      results.forEach((t) => { t.getAnimations().forEach((a) => a.cancel()); t.hidden = !want(t); });
      const shown = results.filter((t) => !t.hidden);
      empty.hidden = shown.length > 0;
      countEl.textContent = !keys ? s.all : shown.length ? `${s.found} ${shown.length}${s.foundUnit}` : s.none;
      if (reduce) return;
      shown.forEach((t, i) => {
        const a = first.get(t), b = t.getBoundingClientRect();
        if (!a) t.animate([{ opacity: 0, transform: "scale(.94)" }, { opacity: 1, transform: "none" }], { duration: 400, delay: i * 40, easing: EASE, fill: "backwards" });
        else if (a.left !== b.left || a.top !== b.top) t.animate([{ transform: `translate(${a.left - b.left}px, ${a.top - b.top}px)` }, { transform: "none" }], { duration: 520, easing: EASE });
      });
    }
    const onInput = () => { clearTimeout(debounce); debounce = window.setTimeout(() => apply(q.value), 180); };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Enter") { e.preventDefault(); clearTimeout(debounce); apply(q.value); } };
    const onClear = () => { q.value = ""; apply(""); q.focus({ preventScroll: true }); };
    const onSug = (e: Event) => { q.value = (e.currentTarget as HTMLElement).textContent!; apply(q.value); };
    q.addEventListener("input", onInput); q.addEventListener("keydown", onKey); qClear.addEventListener("click", onClear);
    sugBtns.forEach((b) => b.addEventListener("click", onSug));
    return () => {
      clearTimeout(debounce); run++;
      q.removeEventListener("input", onInput); q.removeEventListener("keydown", onKey); qClear.removeEventListener("click", onClear);
      sugBtns.forEach((b) => b.removeEventListener("click", onSug));
    };
  }, [s]);
  return (
    <div ref={box}>
      <label className="field-label" htmlFor="q">{s.label}</label>
      <div className="search-box">
        <Ic id="search" />
        <input id="q" type="search" autoComplete="off" placeholder={s.placeholder} aria-describedby="result-count" />
        <button className="icon-btn" type="button" aria-label={s.clear} hidden><Ic id="x" /></button>
      </div>
      <div className="suggest" role="group" aria-label={s.suggest}>
        {s.phrases.map((p) => <button key={p} type="button" aria-pressed="false">{p}</button>)}
      </div>
      <div className="result-meta"><span id="result-count" data-count aria-live="polite">{s.all}</span><span>{s.sample}</span></div>
      <div className="results">
        {LIB.map((k) => (
          <button key={k} type="button" className="ph ph-btn" data-key={k} data-cap={altOf(lang, k)} aria-label={`${c.zoom}${altOf(lang, k)}`}>
            <img src={src(k)} width={720} height={480} alt={altOf(lang, k)} loading="lazy" draggable={false} />
          </button>
        ))}
      </div>
      <div className="empty" hidden>
        <p>{s.empty[0]}</p>
        <p>{s.empty[1]}</p>
      </div>
    </div>
  );
}

// ---- 面板 4：口味 ----
function Taste({ lang }: { lang: Lang }) {
  const c = copy[lang].features, t = c.d;
  const box = useRef<HTMLDivElement>(null);
  const first = t.styles[0];
  useEffect(() => {
    const root = box.current!;
    const tg = root.querySelector<HTMLElement>(".taste-grid")!, note = root.querySelector<HTMLElement>(".taste-note")!;
    const thumb = root.querySelector<HTMLElement>(".thumb")!, segBtns = Array.from(root.querySelectorAll<HTMLButtonElement>(".seg button"));
    const tiles = Array.from(tg.querySelectorAll<HTMLElement>(".ph"));
    let styleIdx = 0, run = 0;
    const timers: number[] = [];
    function placeThumb() {
      const b = segBtns[styleIdx];
      thumb.style.width = `${b.offsetWidth}px`;
      thumb.style.transform = `translateX(${b.offsetLeft - 4}px)`;
    }
    function fill(i: number) {
      const [name, photos, keep] = t.styles[i];
      tiles.forEach((tile, j) => {
        const k = photos[j], img = tile.querySelector("img")!;
        img.src = src(k); img.alt = altOf(lang, k);
        tile.dataset.cap = `${name} · ${altOf(lang, k)}`;
        tile.setAttribute("aria-label", `${c.zoom}${altOf(lang, k)}${(keep as readonly number[]).includes(j) ? ` · ${t.kept}` : ""}`);
      });
      const n = note.querySelectorAll("span");
      n[0].textContent = `${t.note[0]}${name}${t.note[1]}`; n[1].textContent = String(keep.length);
    }
    async function setStyle(i: number) {
      if (i === styleIdx) return;
      const me = ++run; styleIdx = i;
      segBtns.forEach((b, j) => b.setAttribute("aria-pressed", String(j === i)));
      placeThumb();
      const keep = t.styles[i][2] as readonly number[];
      if (reduced()) { fill(i); tiles.forEach((tile, j) => tile.classList.toggle("is-kept", keep.includes(j))); return; }
      tiles.forEach((tile) => tile.classList.remove("is-kept"));
      tg.classList.add("is-out");
      await wait(220); if (me !== run) return;
      tg.classList.add("is-dev"); tg.classList.remove("is-out");
      fill(i);
      await Promise.all(tiles.map((tile) => tile.querySelector("img")!.decode().catch(() => {})));
      if (me !== run) return;
      void tg.offsetWidth;
      tg.classList.remove("is-dev");
      await wait(520); if (me !== run) return;
      let k = 0;
      tiles.forEach((tile, j) => {
        if (!keep.includes(j)) return;
        tile.style.setProperty("--bd", `${k * 40}ms`); tile.classList.add("is-kept");
        timers.push(window.setTimeout(() => { if (me === run) glint(tile); }, k * 40 + 60)); k++;
      });
    }
    const onClick = (e: Event) => setStyle(segBtns.indexOf(e.currentTarget as HTMLButtonElement));
    segBtns.forEach((b) => b.addEventListener("click", onClick));
    placeThumb();
    document.fonts?.ready.then(placeThumb);
    addEventListener("resize", placeThumb);
    return () => { run++; timers.forEach(clearTimeout); segBtns.forEach((b) => b.removeEventListener("click", onClick)); removeEventListener("resize", placeThumb); };
  }, [t, c.zoom, lang]);
  return (
    <div className="taste-wrap" ref={box}>
      <div className="seg" role="group" aria-label={t.pick}>
        <span className="thumb" aria-hidden />
        {t.styles.map(([name], i) => <button key={name} type="button" aria-pressed={i === 0}>{name}</button>)}
      </div>
      <div className="taste-grid">
        {first[1].map((k, j) => {
          const kept = (first[2] as readonly number[]).includes(j);
          return (
            <button key={j} type="button" className={`ph ph-btn${kept ? " is-kept" : ""}`} style={{ "--d": `${j * 40}ms` } as CSSProperties}
              data-cap={`${first[0]} · ${altOf(lang, k)}`} aria-label={`${c.zoom}${altOf(lang, k)}${kept ? ` · ${t.kept}` : ""}`}>
              <img src={src(k)} width={720} height={480} alt={altOf(lang, k)} draggable={false} />
              <Badge label={t.kept} />
            </button>
          );
        })}
      </div>
      <p className="taste-note" aria-live="polite"><span>{`${t.note[0]}${first[0]}${t.note[1]}`}</span><span className="count">{first[2].length}</span><span>{t.note[2]}</span></p>
    </div>
  );
}
