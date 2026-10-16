import ModelGate from "./components/ModelGate";
import { ReactNode, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { MotionConfig, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import Culling from "./modules/Culling";
import Styles from "./modules/Styles";
import Settings from "./modules/Settings";
import { newProjectFromDialog } from "./projectActions";
import { purgeExpired, setActive } from "./projects";
import { SPRING } from "./motion";
import logo from "./assets/logo.png";
import "./App.css";

type ModuleDef = { id: string; title: string; subtitle: string; icon: ReactNode; ready: boolean };

const svg = (d: string) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>
);
const ICON = {
  cull: svg("M2 4.5A1.5 1.5 0 013.5 3h9A1.5 1.5 0 0114 4.5v7a1.5 1.5 0 01-1.5 1.5h-9A1.5 1.5 0 012 11.5zM2.5 11l3.2-3.2a1 1 0 011.4 0L10 10.7M9.4 10l1.2-1.2a1 1 0 011.4 0l1.5 1.5"),
  settings: svg("M8 5.4a2.6 2.6 0 100 5.2 2.6 2.6 0 000-5.2zM8 1.8l.8 1.6 1.7.4 1.5-.9 1.2 1.2-.9 1.5.4 1.7 1.6.8v1.7l-1.6.8-.4 1.7.9 1.5-1.2 1.2-1.5-.9-1.7.4-.8 1.6H6.3l-.8-1.6-1.7-.4-1.5.9-1.2-1.2.9-1.5-.4-1.7L.4 9.3V7.6L2 6.8l.4-1.7-.9-1.5 1.2-1.2 1.5.9 1.7-.4.8-1.6z"),
  meter: svg("M8 2.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM8 5v3l2 1.2"),
};

export default function App() {
  const { t, i18n } = useTranslation();
  const [moduleId, setModuleId] = useState("culling");
  /** Where the styles page was opened from, so its back button returns there and not always to settings. */
  const [stylesFrom, setStylesFrom] = useState<"culling" | "settings">("settings");
  const openStyles = (from: "culling" | "settings") => { setStylesFrom(from); setModuleId("styles"); };
  /** true = full sidebar, false = narrow icon rail. */
  const [sidebar, setSidebar] = useState(true);
  // The inspector has one rule: it is there when a photo is selected and gone when none is. The only choice left to the user is to
  // hide it while a photo is selected; that stays until they bring it back.
  const [inspectorHidden, setInspectorHidden] = useState(false);
  const toggleInspector = () => setInspectorHidden((v) => !v);
  const newProject = async () => { if (await newProjectFromDialog()) setModuleId("culling"); };

  useEffect(() => {
    purgeExpired();
    window.addEventListener("focus", purgeExpired);
    return () => window.removeEventListener("focus", purgeExpired);
  }, []);

  useEffect(() => {
    const un = listen<string>("menu", (e) => {
      if (e.payload === "toggle-sidebar") setSidebar((v) => !v);
      if (e.payload === "toggle-inspector") toggleInspector();
      if (e.payload === "open-settings") setModuleId("settings");
      if (e.payload === "open-folder") newProject();
    });
    return () => { un.then((f) => f()); };
  }, []);

  /** Each feature is one module: a sidebar entry plus its own workspace (and inspector). */
  const modules: ModuleDef[] = [
    { id: "culling", title: t("nav.cull"), subtitle: t("nav.cullSub"), ready: true, icon: ICON.cull },
    { id: "exif", title: t("nav.meter"), subtitle: t("nav.meterSub"), ready: false, icon: ICON.meter },
  ];
  const current = modules.find((m) => m.id === moduleId);

  return (
    <MotionConfig reducedMotion="user">
      {/* shared gradient for the gleam mark used on photo tiles */}
      <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden>
        <defs>
          <linearGradient id="fg-gold" x1=".2" y1="0" x2=".8" y2="1"><stop offset="0" stopColor="#FFF3C4" /><stop offset=".55" stopColor="#FFD87A" /><stop offset="1" stopColor="#FFB84D" /></linearGradient>
        </defs>
      </svg>
      <ModelGate />
      <div className={`shell ${sidebar ? "" : "collapsed"}`}>
        <button className="sidebar-toggle" onClick={() => setSidebar((v) => !v)}
          aria-label={sidebar ? t("nav.collapse") : t("nav.expand")} title={`${sidebar ? t("nav.collapse") : t("nav.expand")} (⌘⌥S)`}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><rect x="1.7" y="2.7" width="12.6" height="10.6" rx="2" /><path d="M6 3v10" /></svg>
        </button>
        <div className="sidebar-wrap">
          <nav className="sidebar">
            <div className="sidebar-drag" data-tauri-drag-region />
            <div className="brand">
              <img className="brand-logo" src={logo} alt="" draggable={false} />
              <div className="brand-text">
                <b>{t("brand.name")}</b>
                {i18n.language !== "en" && <span className="mono">{t("brand.latin")}</span>}
              </div>
            </div>
            <h2 className="grp">{t("nav.group")}</h2>
            <ul className="modules">
              {modules.map((m) => (
                <li key={m.id}>
                  <button className={m.id === moduleId ? "active" : ""} onClick={() => { if (m.id === moduleId && m.id === "culling") setActive(null); setModuleId(m.id); }} title={m.title}>
                    {m.id === moduleId && <motion.i layoutId="navpill" className="navpill" transition={SPRING.glide} />}
                    <span className="nav-inner">{m.icon}<span className="nav-text">{m.title}<small>{m.subtitle}</small></span>{!m.ready && <em>{t("nav.soon")}</em>}</span>
                  </button>
                </li>
              ))}
            </ul>
            <p className="privacy">{t("nav.privacy")}</p>
            <ul className="nav-bottom">
              <li>
                <button className={moduleId === "settings" ? "active" : ""} onClick={() => setModuleId("settings")} title={t("settings.title")}>
                  {moduleId === "settings" && <motion.i layoutId="navpill" className="navpill" transition={SPRING.glide} />}
                  <span className="nav-inner">{ICON.settings}<span className="nav-text">{t("settings.title")}</span><kbd>⌘,</kbd></span>
                </button>
              </li>
            </ul>
          </nav>
        </div>
        <main className="content">
          <Culling active={moduleId === "culling"} inspectorHidden={inspectorHidden} onToggleInspector={toggleInspector} onOpenStyles={() => openStyles("culling")} />
          <Styles active={moduleId === "styles"} from={stylesFrom} onBack={() => setModuleId(stylesFrom)} />
          <Settings active={moduleId === "settings"} onOpenStyles={() => openStyles("settings")} />
          {current && !current.ready && (
            <div className="module soon">
              <div className="empty">
                <h1>{current.title}</h1>
                <p>{t("nav.soonBody", { name: t("nav.cull") })}</p>
              </div>
            </div>
          )}
        </main>
      </div>
    </MotionConfig>
  );
}
