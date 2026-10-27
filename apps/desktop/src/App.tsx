import ModelGate from "./components/ModelGate";
import DownloadNav from "./components/DownloadNav";
import FolderPicker from "./components/FolderPicker";
import ErrorBoundary from "./components/ErrorBoundary";
import { ReactNode, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import Culling from "./modules/Culling";
import Styles from "./modules/Styles";
import Settings from "./modules/Settings";
import { newProjectFromDialog, projectFromFolders } from "./projectActions";
import { purgeExpired, setActive, useProjectsStatus } from "./projects";
import { refreshShares } from "./nas";
import { watchForBackup } from "./backup";
import { DUR, SPRING } from "./motion";
import { editHistory } from "./history";
import logo from "./assets/logo.png";
import "./App.css";

type ModuleDef = { id: string; title: string; subtitle: string; icon: ReactNode; ready: boolean };

const svg = (d: string) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>
);
const ICON = {
  cull: svg("M2 4.5A1.5 1.5 0 013.5 3h9A1.5 1.5 0 0114 4.5v7a1.5 1.5 0 01-1.5 1.5h-9A1.5 1.5 0 012 11.5zM2.5 11l3.2-3.2a1 1 0 011.4 0L10 10.7M9.4 10l1.2-1.2a1 1 0 011.4 0l1.5 1.5"),
  settings: svg("M8 5.4a2.6 2.6 0 100 5.2 2.6 2.6 0 000-5.2zM8 1.8l.8 1.6 1.7.4 1.5-.9 1.2 1.2-.9 1.5.4 1.7 1.6.8v1.7l-1.6.8-.4 1.7.9 1.5-1.2 1.2-1.5-.9-1.7.4-.8 1.6H6.3l-.8-1.6-1.7-.4-1.5.9-1.2-1.2.9-1.5-.4-1.7L.4 9.3V7.6L2 6.8l.4-1.7-.9-1.5 1.2-1.2 1.5.9 1.7-.4.8-1.6z"),
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
  /** 打开设置并滚到某一节（例如 NAS 密码不对时直接到 NAS 那一节）。设置页一直挂着，切过去之后就能滚。 */
  const openSettingsAt = (section?: string) => {
    setModuleId("settings");
    if (section) setTimeout(() => document.getElementById(section)?.scrollIntoView({ block: "start" }), 0);
  };
  const newProject = async () => { if (await newProjectFromDialog()) setModuleId("culling"); };

  useEffect(() => {
    purgeExpired();
    void refreshShares(); // 添加过 NAS 的话，"新建项目"要先问文件夹在本机还是共享里
    watchForBackup();     // 配置过云备份的话，设置和喜好一变就自动上传
    // 后端启动时在后台连 NAS，连完会说一声；设置页的状态跟着刷新
    const un = listen("nas-changed", () => { void refreshShares(); });
    window.addEventListener("focus", purgeExpired);
    return () => { un.then((f) => f()); window.removeEventListener("focus", purgeExpired); };
  }, []);

  // 把文件夹（或照片）拖进窗口：松手就用它们建项目并打开（已经是某个项目的那个文件夹就直接打开它）
  const [dragging, setDragging] = useState(false);
  useEffect(() => {
    let un: (() => void) | null = null;
    let live = true;
    getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter") setDragging(true);
      else if (p.type === "leave") setDragging(false);
      else if (p.type === "drop") {
        setDragging(false);
        invoke<string[]>("dropped_folders", { paths: p.paths })
          .then((dirs) => { if (projectFromFolders(dirs)) setModuleId("culling"); })
          .catch(() => { /* not in Tauri */ });
      }
    }).then((f) => { if (live) un = f; else f(); }).catch(() => { /* not in Tauri */ });
    return () => { live = false; un?.(); };
  }, []);

  useEffect(() => {
    const un = listen<string>("menu", (e) => {
      if (e.payload === "toggle-sidebar") setSidebar((v) => !v);
      if (e.payload === "toggle-inspector") toggleInspector();
      if (e.payload === "open-settings") setModuleId("settings");
      if (e.payload === "open-folder") newProject();
      // 编辑菜单的撤销 / 重做：文本框里是文字的撤销，别处是照片墙的撤销栈
      if (e.payload === "undo" || e.payload === "redo") editHistory(e.payload);
    });
    return () => { un.then((f) => f()); };
  }, []);

  /** Each feature is one module: a sidebar entry plus its own workspace (and inspector). */
  // 没做好的模块不列（设计审查 M1.2）；导航只留名字，副标题删掉（M1.3）
  const modules: ModuleDef[] = [
    { id: "culling", title: t("nav.cull"), subtitle: "", ready: true, icon: ICON.cull },
  ];
  const current = modules.find((m) => m.id === moduleId);
  // the projects are read from the backend first; until then there is nothing right to show
  const { loaded } = useProjectsStatus();
  if (!loaded) return <div className="shell" />;

  return (
    <MotionConfig reducedMotion="user">
      {/* shared gradient for the gleam mark used on photo tiles */}
      <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden>
        <defs>
          <linearGradient id="fg-gold" x1=".2" y1="0" x2=".8" y2="1"><stop offset="0" stopColor="#FFF3C4" /><stop offset=".55" stopColor="#FFD87A" /><stop offset="1" stopColor="#FFB84D" /></linearGradient>
        </defs>
      </svg>
      <ModelGate />
      <FolderPicker />
      <AnimatePresence>
        {dragging && (
          <motion.div className="dropzone" aria-hidden initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: DUR.fast }}>
            <div className="dropzone-text"><b>{t("drop.title")}</b><span className="ash">{t("drop.body")}</span></div>
          </motion.div>
        )}
      </AnimatePresence>
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
                    <span className="nav-inner">{m.icon}<span className="nav-text">{m.title}</span>{!m.ready && <em>{t("nav.soon")}</em>}</span>
                  </button>
                </li>
              ))}
            </ul>
            <DownloadNav />
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
          <ErrorBoundary>
          <Culling active={moduleId === "culling"} inspectorHidden={inspectorHidden} onToggleInspector={toggleInspector} onOpenStyles={() => openStyles("culling")} onOpenSettings={openSettingsAt} />
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
          </ErrorBoundary>
        </main>
      </div>
    </MotionConfig>
  );
}
