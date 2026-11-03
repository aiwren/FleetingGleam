import { Fragment, useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import { AndroidLogo, AppleLogo, GithubLogo, List, X } from "@phosphor-icons/react";
import { copy, release, type Lang } from "../../content";
import { Gleam, GleamDefs } from "../Gleam";
import { IconDefs, Ic } from "../Icons";
import Footer from "../Footer";
import ThemeToggle from "../ThemeToggle";
import { addScene, lerp, localProg, pinProg, reduced, reveal, smooth } from "../scroll";
import { home } from "./copy";
import Story from "./Story";
import Vault from "./Vault";
import { Focus } from "./Backdrops";

const v = (o: Record<string, string | number>) => o as CSSProperties;

/** 一句话切成会逐个出现的小块：英文按词（空格跟着前一个词），中文按字（标点跟着前一个字）。不用浏览器分词，服务端和浏览器切法一样。 */
function pieces(text: string, lang: Lang) {
  if (lang !== "zh") return text.split(/(?<=\s)/);
  const out: string[] = [];
  for (const ch of text) { if (/[，。、；：！？]/.test(ch) && out.length) out[out.length - 1] += ch; else out.push(ch); }
  return out;
}

/** 按钮文字：悬停时原文向上滚出，同一句从下面接上。 */
function Roll({ children }: { children: string }) {
  return <span className="v-roll"><span>{children}</span><span aria-hidden>{children}</span></span>;
}
const Arrow = ({ dir }: { dir: "down" | "out" }) => (
  <svg className="v-arrow" data-dir={dir} viewBox="0 0 16 16" aria-hidden>
    {dir === "down" ? <path d="M8 3v10M3.5 8.5 8 13l4.5-4.5" /> : <path d="M5 11 11 5M5.5 5H11v5.5" />}
  </svg>
);

/* ---------------- 导航：左边 logo 圆盘，中间玻璃胶囊，右边白色胶囊；滚过首屏换磨砂底 ---------------- */
function Nav({ lang }: { lang: Lang }) {
  const c = copy[lang], x = home[lang];
  const [solid, setSolid] = useState(false), [open, setOpen] = useState(false);
  // 记住手动选的语言：英文首页会按浏览器语言跳中文，选过之后就不再跳（见 __root.tsx）
  const remember = () => { try { localStorage.setItem("lang", x.other.id); } catch { /* 记不住也没关系 */ } };
  useEffect(() => {
    const on = () => setSolid(scrollY > innerHeight * 0.8);
    on(); addEventListener("scroll", on, { passive: true });
    return () => removeEventListener("scroll", on);
  }, []);
  useEffect(() => {
    if (!open) return;
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    const close = () => setOpen(false);
    document.addEventListener("keydown", esc); addEventListener("resize", close);
    document.documentElement.style.overflow = "hidden";
    return () => { document.removeEventListener("keydown", esc); removeEventListener("resize", close); document.documentElement.style.overflow = ""; };
  }, [open]);
  return (
    <header className={`v-nav${solid ? " is-solid" : ""}`}>
      <Link to={c.home as never} className="v-brand" aria-label={c.name}>
        <span className="v-disc v-glass"><img src="/logo.png" alt="" width={26} height={26} /></span>
        <b>{c.name}</b>
      </Link>
      <nav className="v-links v-glass" aria-label="Sections">
        {c.nav.map(([l, h]) => <a key={h} href={h}>{l}</a>)}
      </nav>
      <div className="v-right">
        <a className="v-lang" href={x.other.to} hrefLang={lang === "zh" ? "en" : "zh-CN"} onClick={remember}>{x.other.label}</a>
        <span className="v-theme"><ThemeToggle label={c.theme} /></span>
        <a className="v-pill v-pill-solid v-cta-sm" href="#download"><Roll>{c.cta.ready}</Roll></a>
        <button type="button" className="v-burger v-glass" aria-expanded={open} aria-label={c.menu} onClick={() => setOpen(true)}><List size={18} /></button>
      </div>
      {open && (
        <div className="v-menu" role="dialog" aria-modal="true" aria-label={c.menu}>
          <button type="button" className="v-burger v-glass v-menu-x" aria-label={x.close} onClick={() => setOpen(false)}><X size={18} /></button>
          <ul>
            {c.nav.map(([l, h], i) => <li key={h} style={v({ "--i": i })}><a href={h} onClick={() => setOpen(false)}>{l}</a></li>)}
            <li style={v({ "--i": c.nav.length })}><a href={x.other.to} onClick={remember}>{x.other.label}</a></li>
          </ul>
        </div>
      )}
    </header>
  );
}

/* ---------------- 首屏：通栏大图取景对焦，左下大标题逐词对焦 ---------------- */
function Hero({ lang }: { lang: Lang }) {
  const c = copy[lang], h = c.hero, x = home[lang];
  const step = lang === "zh" ? 45 : 90;
  let n = 0;
  // 标题按块逐个对焦；强调词换成衬线斜体、颜色收一档
  const line = (text: string) => {
    const at = text.indexOf(x.accent);
    const parts = at < 0 ? [[text, false]] : [[text.slice(0, at), false], [x.accent, true], [text.slice(at + x.accent.length), false]];
    return (parts as [string, boolean][]).filter(([t]) => t).map(([t, acc], k) => (
      <span key={k} className={acc ? "v-accent" : undefined}>
        {pieces(t, lang).map((w, i) => <span key={i} className="v-word" style={v({ "--d": `${250 + n++ * step}ms` })}>{w}</span>)}
      </span>
    ));
  };
  return (
    <section className="v-hero" aria-labelledby="v-h1">
      <div className="v-frame">
        <div className="v-hero-film"><Focus lang={lang} /></div>
        <div className="v-hero-shade" aria-hidden />
        <div className="v-hero-fade" aria-hidden />
        <div className="v-hero-body">
          <p className="v-badge v-glass v-in" style={v({ "--d": "150ms" })}><span className="v-chip">{x.chip}</span>{x.badge}</p>
          <h1 id="v-h1" className={`v-h1${lang === "zh" ? " is-zh" : ""}`}>
            <span className="v-h1-line">{line(h.title[0])}</span>
            <span className="v-h1-line">{line(h.title[1])}</span>
          </h1>
          <p className="v-sub v-in" style={v({ "--d": "950ms" })}>{h.body}</p>
          <div className="v-ctas v-in" style={v({ "--d": "1100ms" })}>
            <a className="v-pill v-pill-solid v-pill-lg" href="#download"><Roll>{x.cta}</Roll><span className="v-chip-arrow"><Arrow dir="down" /></span></a>
            <a className="v-pill v-glass v-pill-lg" href="#story"><Roll>{x.cta2}</Roll></a>
          </div>
        </div>
      </div>
    </section>
  );
}

/* ---------------- 隐私：一句大标题，一段正文（关键句琥珀色），右边轨道中间的手机演示本机处理 ---------------- */
function Manifesto({ lang }: { lang: Lang }) {
  const p = copy[lang].privacy, hi = home[lang].privacyHi;
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => reveal(body.current!), []);
  const at = p.text.indexOf(hi);
  return (
    <section className="v-sec v-manifesto" id="privacy" aria-labelledby="v-pv">
      <div className="v-wrap v-manifesto-grid" ref={body}>
        <div>
          <p className="v-eyebrow rv"><span className="v-dot" />{p.eyebrow}</p>
          <h2 id="v-pv" className={`v-pv-title${lang === "zh" ? " is-zh" : ""}`}><span className="v-ln"><span>{p.title}</span></span></h2>
          <p className="v-pv-body rv" style={v({ "--d": "200ms" })}>
            {at < 0 ? p.text : <>{p.text.slice(0, at)}<em>{hi}</em>{p.text.slice(at + hi.length)}</>}
          </p>
          <ul className="v-points">
            {p.points.map(([icon, label], i) => <li key={label} className="rv" style={v({ "--d": `${300 + i * 100}ms` })}><Ic id={icon} />{label}</li>)}
          </ul>
        </div>
        <Vault lang={lang} />
      </div>
    </section>
  );
}

/* ---------------- 功能便当格 ---------------- */
/** 搜索框里轮流打出几句示例描述，下面对应的那张缩略图亮起细边；不在视野里就停。 */
function Typer({ lang }: { lang: Lang }) {
  // 示例描述和缩略图换成和整页同一趟旅行的风景（首页仍用 content.ts 里那一套）
  const f = copy[lang].features, phrases = home[lang].typer, keys = [987, 827, 235, 563] as const;
  const [k, setK] = useState(0), [txt, setTxt] = useState<string>(phrases[0]);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (reduced()) return;
    let alive = true, vis = false, timer = 0;
    const sleep = (ms: number) => new Promise<void>((r) => { timer = window.setTimeout(r, ms); });
    const io = new IntersectionObserver(([e]) => { vis = e.isIntersecting; }, { threshold: 0.2 });
    io.observe(box.current!);
    (async () => {
      let i = 0;
      while (alive) {
        await sleep(1600);
        if (!vis) continue;
        const cur = phrases[i];
        for (let j = cur.length; j >= 0 && alive; j--) { setTxt(cur.slice(0, j)); await sleep(18); }
        i = (i + 1) % phrases.length; setK(i);
        const next = phrases[i];
        for (let j = 1; j <= next.length && alive; j++) { setTxt(next.slice(0, j)); await sleep(lang === "zh" ? 90 : 35); }
      }
    })();
    return () => { alive = false; clearTimeout(timer); io.disconnect(); };
  }, [lang, phrases]);
  return (
    <div ref={box}>
      <div className="v-search" role="img" aria-label={`${f.c.label}: ${phrases.join(", ")}`}>
        <Ic id="search" /><span className="v-typed">{txt}<i className="v-caret" /></span>
      </div>
      <div className="v-thumbs">
        {keys.map((key, i) => <img key={key} src={`/home/t-${key}.webp`} alt="" className={i === k ? "is-on" : ""} loading="lazy" />)}
      </div>
    </div>
  );
}

function Features({ lang }: { lang: Lang }) {
  const c = copy[lang], f = c.features, z = c.zoom, pro = c.pro, x = home[lang].features;
  const body = useRef<HTMLDivElement>(null), proBox = useRef<HTMLDivElement>(null);
  useEffect(() => { const a = reveal(body.current!), b = reveal(proBox.current!); return () => { a(); b(); }; }, []);
  const shot = (name: string, alt: string) => (
    <div className="v-shot">
      <img className="for-dark" src={`/shots/${name}-dark-1200.webp`} srcSet={`/shots/${name}-dark-1200.webp 1200w, /shots/${name}-dark-2400.webp 2400w`} sizes="(min-width: 1024px) 50vw, 100vw" width={1200} height={750} alt={alt} loading="lazy" />
      <img className="for-light" src={`/shots/${name}-light-1200.webp`} srcSet={`/shots/${name}-light-1200.webp 1200w, /shots/${name}-light-2400.webp 2400w`} sizes="(min-width: 1024px) 50vw, 100vw" width={1200} height={750} alt={alt} loading="lazy" />
    </div>
  );
  return (
    <section className="v-sec" id="features" aria-labelledby="v-ft">
      <div className="v-wrap" ref={body}>
        <p className="v-eyebrow rv"><span className="v-dot" />{x.eyebrow}</p>
        <h2 id="v-ft" className="v-h2 rv" style={v({ "--d": "80ms" })}>{x.title}</h2>
        <div className="v-bento">
          <article className="v-card v-span7 v-tall rv" style={v({ "--d": "120ms" })}>
            <div><h3>{f.a.t}</h3><p>{f.a.d}</p></div>
            {shot("wall", c.hero.alt)}
          </article>
          <article className="v-card v-span5 rv v-kinds-card" style={v({ "--d": "200ms" })}>
            <div><h3>{f.b.t}</h3><p>{f.b.d}</p></div>
            <ul className="v-kinds" aria-label={f.b.sr}>
              {f.b.kinds.map((k, i) => <li key={k} style={v({ "--i": i })}>{k}</li>)}
            </ul>
          </article>
          <article className="v-card v-span5 rv" style={v({ "--d": "120ms" })}>
            <div><h3>{f.c.t}</h3><p>{f.c.d}</p></div>
            <Typer lang={lang} />
          </article>
          <article className="v-card v-span7 rv" style={v({ "--d": "200ms" })}>
            <div><h3>{z.title}</h3><p>{z.lead}</p></div>
            {shot("compare", z.aria)}
          </article>
        </div>
      </div>
      <div className="v-wrap v-pro" id="pro" ref={proBox}>
        <p className="v-eyebrow rv"><span className="v-dot" />{pro.eyebrow}</p>
        <h2 className="v-h2 rv" style={v({ "--d": "80ms" })}>{pro.title}</h2>
        <p className="v-lead rv" style={v({ "--d": "140ms" })}>{pro.lead}</p>
        <ul className="v-pro-grid">
          {pro.items.map(([icon, t, d], i) => (
            <li key={t} className="v-card rv v-rv-scale" style={v({ "--d": `${200 + i * 90}ms` })}><Ic id={icon} /><h3>{t}</h3><p>{d}</p></li>
          ))}
        </ul>
      </div>
    </section>
  );
}

/* ---------------- 评分只是参考：巨字两行从遮罩里升起 ---------------- */
/**
 * 右边的小剧场：同一座雪山的三张连拍（示例：同一张图裁出来的），一张地平线歪了、一张清楚、一张偏暗。
 * 评分先推荐清楚那张；滚下去，琥珀"保留"落到分数低、偏暗的那张上，推荐那张的标签变成"仅供参考"。
 * 三张照片的颜色始终不动。鼠标在上面时整组跟着轻轻偏转。
 */
const PICK = [
  { src: "/home/trio-tilt.webp", score: 81, x: 0, y: 14, r: -7 },
  { src: "/home/trio-sharp.webp", score: 92, x: 24, y: 0, r: 2 },
  { src: "/home/trio-dark.webp", score: 74, x: 46, y: 30, r: 5 },
];
function Pick({ lang }: { lang: Lang }) {
  const t = home[lang].pick;
  const stage = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = stage.current!;
    const cards = Array.from(el.querySelectorAll<HTMLElement>(".v-pick-card"));
    const render = (p: number) => {
      const fan = smooth(0, 0.4, p), rec = smooth(0.38, 0.55, p), keep = smooth(0.62, 0.86, p);
      cards.forEach((c, i) => {
        const d = PICK[i];
        // 先叠在中间，再扇开到各自的位置
        const x = lerp(24, d.x, fan), y = lerp(12, d.y, fan), r = lerp(0, d.r, fan);
        const lift = i === 2 ? keep : 0, sink = i === 1 ? keep : 0;
        c.style.left = `${x}%`; c.style.top = `${y}%`;
        c.style.transform = `rotate(${r.toFixed(2)}deg) translateY(${(-14 * lift).toFixed(1)}px) scale(${(1 + 0.05 * lift - 0.05 * sink).toFixed(4)})`;
        c.style.zIndex = String(i === 2 ? (keep > 0.3 ? 4 : 3) : i === 1 ? 2 : 1);
        c.style.opacity = lerp(i === 1 ? 1 : 0, 1, fan).toFixed(3);
      });
      el.style.setProperty("--rec", rec.toFixed(3)); el.style.setProperty("--keep", keep.toFixed(3));
    };
    const off: (() => void)[] = [];
    if (reduced()) render(1);
    else {
      off.push(addScene(el, (e) => localProg(e, 0.92, 0.6), render));
      // 鼠标偏转
      const move = (e: PointerEvent) => {
        const b = el.getBoundingClientRect();
        el.style.setProperty("--tx", (((e.clientX - b.left) / b.width - 0.5) * 2).toFixed(3));
        el.style.setProperty("--ty", (((e.clientY - b.top) / b.height - 0.5) * 2).toFixed(3));
      };
      const leave = () => { el.style.setProperty("--tx", "0"); el.style.setProperty("--ty", "0"); };
      el.addEventListener("pointermove", move); el.addEventListener("pointerleave", leave);
      off.push(() => { el.removeEventListener("pointermove", move); el.removeEventListener("pointerleave", leave); });
    }
    return () => off.forEach((f) => f());
  }, [lang]);
  return (
    <div className="v-pick" ref={stage} aria-hidden>
      <div className="v-pick-glow" />
      <div className="v-pick-tilt">
        {PICK.map((d, i) => (
          <figure key={d.src} className={`v-pick-card${i === 1 ? " is-rec" : ""}${i === 2 ? " is-kept" : ""}`}>
            <img src={d.src} alt="" loading="lazy" draggable={false} />
            <span className="v-pick-star"><Gleam size={18} /></span>
            <figcaption className="v-pick-tag v-glass">
              <b className="v-mono">{d.score}</b>
              {i === 0 && <span>{t.tilt}</span>}
              {i === 1 && <span className="v-pick-swap"><span>{t.hint}</span><span>{t.ref}</span></span>}
              {i === 2 && <span className="v-pick-swap"><span>{t.low}</span><span>{t.kept}</span></span>}
            </figcaption>
          </figure>
        ))}
      </div>
    </div>
  );
}

function Honest({ lang }: { lang: Lang }) {
  const c = copy[lang].honest, acc = home[lang].honestAccent;
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => reveal(body.current!), []);
  return (
    <section className="v-sec v-honest" id="honest" aria-labelledby="v-hn">
      <div className="v-wrap v-honest-grid" ref={body}>
        <div>
          <h2 id="v-hn" className={`v-mega${lang === "zh" ? " is-zh" : ""}`}>
            <span className="v-ln"><span>{c.title[0]}</span></span>
            <span className="v-ln" style={v({ "--i": 1 })}><span className="v-accent">{acc}</span></span>
          </h2>
          <p className="v-lead rv" style={v({ "--d": "360ms" })}>{c.body}</p>
        </div>
        <Pick lang={lang} />
      </div>
    </section>
  );
}

/* ---------------- 装好之后的三步（在下载卡片下面） ---------------- */
/** 正在决定下不下载的人，顺便知道装好后怎么用：一行三步，第二步带上 Mac 的快捷键。 */
function Steps({ lang }: { lang: Lang }) {
  const st = home[lang].dl.steps, keys = copy[lang].zoom.keys.slice(0, 3);
  const withKeys = (s: string): ReactNode => {
    const [a, b] = s.split("{keys}");
    return b === undefined ? s : <>{a}{keys.map(([k], i) => <Fragment key={k}>{i ? " / " : ""}<kbd>{k}</kbd></Fragment>)}{b}</>;
  };
  return (
    <div className="v-steps rv" style={v({ "--d": "440ms" })}>
      <p className="v-steps-label v-mono">{st.label}</p>
      <ol aria-label={st.label}>
        {st.items.map(([t, d], i) => (
          <li key={t}><span className="v-mono">{String(i + 1).padStart(2, "0")}</span><b>{t}</b><span className="v-steps-d">{withKeys(d)}</span></li>
        ))}
      </ol>
    </div>
  );
}

/* ---------------- 收尾：首屏那张图再出现一次（全清楚），三个下载入口 ---------------- */
const XATTR = "xattr -cr /Applications/FleetingGleam.app";

/** 一行命令加复制按钮。 */
function Cmd({ lang }: { lang: Lang }) {
  const d = home[lang].dl;
  const [ok, setOk] = useState(false);
  const copyIt = () => navigator.clipboard?.writeText(XATTR).then(() => { setOk(true); setTimeout(() => setOk(false), 1600); }, () => {});
  return (
    <div className="v-cmd">
      {/* 只在斜杠后面换行，免得把 .app 拆开 */}
      <code>{XATTR.split("/").map((part, i) => <Fragment key={i}>{i ? <>/<wbr /></> : null}{part}</Fragment>)}</code>
      <button type="button" onClick={copyIt} aria-live="polite">{ok ? d.copied : d.copy}</button>
    </div>
  );
}

function Closing({ lang }: { lang: Lang }) {
  const c = copy[lang].final, d = home[lang].dl;
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => reveal(body.current!), []);
  // 每个平台一张卡：系统要求、下载按钮、装之前要知道的那一句，都在同一处
  return (
    <section className="v-sec v-close" id="download" aria-labelledby="v-dl">
      <div className="v-frame v-frame-close" ref={body}>
        <div className="v-close-film"><Focus lang={lang} quiet /></div>
        <div className="v-close-shade" aria-hidden />
        <div className="v-close-body">
          <h2 id="v-dl" className="v-h2 v-close-title"><span className="v-ln"><span>{c.readyTitle}</span></span></h2>
          <p className="v-sub rv" style={v({ "--d": "200ms" })}>{c.readyBody}</p>
          <div className="v-dl-grid">
            <article className="v-dl-card v-glass rv" style={v({ "--d": "300ms" })}>
              <h3><AppleLogo size={20} weight="fill" />{d.mac.name}</h3>
              <p className="v-dl-req">{d.mac.req}</p>
              <a className="v-pill v-pill-solid v-pill-lg" href={release.download}><Roll>{c.download}</Roll><span className="v-chip-arrow"><Arrow dir="down" /></span></a>
              <p className="v-dl-note">{d.mac.note}</p>
              <Cmd lang={lang} />
            </article>
            <article className="v-dl-card v-glass rv" style={v({ "--d": "380ms" })}>
              <h3><AndroidLogo size={20} weight="fill" />{d.android.name}</h3>
              <p className="v-dl-req">{d.android.req}</p>
              <a className="v-pill v-pill-solid v-pill-lg" href={release.android}><Roll>{c.downloadAndroid}</Roll><span className="v-chip-arrow"><Arrow dir="down" /></span></a>
              <p className="v-dl-note">{d.android.note}</p>
            </article>
          </div>
          <Steps lang={lang} />
          <ul className="v-dl-facts rv" style={v({ "--d": "500ms" })}>
            {d.facts.map((f) => <li key={f}><svg viewBox="0 0 16 16" aria-hidden><path d="M3.5 8.5 6.5 11.5 12.5 5" /></svg>{f}</li>)}
          </ul>
          <a className="v-textlink rv" style={v({ "--d": "560ms" })} href={release.repo}><GithubLogo size={16} /><Roll>{c.source}</Roll><Arrow dir="out" /></a>
        </div>
      </div>
    </section>
  );
}

/** 第二屏以后几段的底：一层钉住的天幕，随这几段的进度从夜走到晨光（样式见 home.css 的 .v-sky）。 */
function After({ children }: { children: ReactNode }) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (reduced()) return;
    const el = box.current!;
    // 从这一块顶边到窗口顶，到底边到窗口底
    return addScene(el, pinProg, (p) => el.style.setProperty("--t", p.toFixed(4)));
  }, []);
  return (
    <div className="v-after" ref={box}>
      <div className="v-sky" aria-hidden><i className="v-sky-night" /><i className="v-sky-dusk" /><i className="v-sky-dawn" /><i className="v-sky-stars" /></div>
      {children}
    </div>
  );
}

/** 首页：取景对焦的首屏，照片墙那一幕，隐私、功能、评分，最后是下载（带装好后的三步）。 */
export default function Home({ lang }: { lang: Lang }) {
  return (
    <div className="home">
      <GleamDefs />
      <IconDefs />
      <Nav lang={lang} />
      <main>
        <Hero lang={lang} />
        <Story lang={lang} />
        <After>
          <Manifesto lang={lang} />
          <Features lang={lang} />
          <Honest lang={lang} />
            <Closing lang={lang} />
        </After>
      </main>
      <Footer lang={lang} />
    </div>
  );
}
