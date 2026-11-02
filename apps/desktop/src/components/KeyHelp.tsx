import { Fragment } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { DUR, EASE_OUT } from "../motion";
import { keyFor, useKeymap } from "../keymap";

/** 墙 / 单张 / 对比三种布局，按键条的内容按它变。 */
export type KeyLayout = "wall" | "single" | "compare";

/**
 * 常驻的按键条（A7.6）：贴在照片区底部一条很克制的提示，细字、ash 色、mono 的 kbd；键名跟着设置里的快捷键预设走。
 * 内容按布局变：墙上是判定、星级、大图、撤销；对比布局是悬停判定、换对手、换组、同步缩放。设置里"常驻按键提示"可以关掉。
 */
export function KeyBar({ layout }: { layout: KeyLayout }) {
  const { t } = useTranslation();
  const km = useKeymap();
  const sp = t("inspector.space");
  const items: [string, string][] = layout === "compare"
    ? [["", t("cull.keybar.hover")], [keyFor("keep", km), t("cull.keybar.keep")], [keyFor("reject", km), t("cull.keybar.reject")], ["↑↓", t("cull.keybar.rival")], ["← →", t("cull.keybar.group")], ["⌘Z", t("cull.keybar.undo")], ["?", t("cull.keybar.help")]]
    : [[keyFor("keep", km), t("cull.keybar.keep")], [keyFor("review", km), t("cull.keybar.review")], [keyFor("reject", km), t("cull.keybar.reject")], ["1–5", t("cull.keybar.stars")],
      ...(layout === "single" ? [["← →", t("cull.keybar.move")] as [string, string]] : []),
      [sp, t("cull.keybar.zoom")], ["⌘Z", t("cull.keybar.undo")], ["?", t("cull.keybar.help")]];
  return (
    <div className="keybar ash" aria-hidden>
      {items.map(([k, label], i) => (
        <Fragment key={i}>
          {i > 0 && <i className="keybar-dot">·</i>}
          <span>{k && <kbd className="mono">{k}</kbd>}{label}</span>
        </Fragment>
      ))}
    </div>
  );
}

/**
 * 按 ? 弹出的快捷键总表：判定键、星级、方向键、空格、⌘Z / ⌘⇧Z、⌘] / ⌘[ 旋转，和对比布局的 ↑↓ / ← →。
 * 再按 ? 或 Esc 或点任意处关闭（按键在 Culling 里处理）。
 */
export function KeyHelp({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const km = useKeymap();
  const sp = t("inspector.space");
  const h = (k: string) => t(`cull.help.${k}`);
  const sections: { title: string; rows: [string, string][] }[] = [
    { title: h("decide"), rows: [[keyFor("keep", km), h("keep")], [keyFor("review", km), h("review")], [keyFor("reject", km), h("reject")], ["", h("clearMark")]] },
    { title: h("stars"), rows: [["1–5", h("rate")], ["0", h("clearStars")]] },
    { title: h("move"), rows: [["← →", h("prevNext")], ["↑ ↓", h("rows")], ["PgUp PgDn", h("pages")], ["Home End", h("ends")], ["⇧", h("extend")]] },
    { title: h("view"), rows: [[sp, h("zoom")], ["I", h("info")], [keyFor("motion", km), h("motion")], ["?", h("helpKey")]] },
    { title: h("edit"), rows: [["⌘Z", h("undo")], ["⌘⇧Z", h("redo")], ["⌘] ⌘[", h("rotate")], ["⌘F", h("search")]] },
    { title: h("compare"), rows: [["↑ ↓", h("cmpRival")], ["← →", h("cmpGroup")], [`${keyFor("keep", km)} ${keyFor("reject", km)}`, h("cmpDecide")], ["0", h("cmpReset")]] },
  ];
  return (
    <motion.div className="keyhelp-back" role="dialog" aria-label={h("title")} onClick={onClose}
      initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: DUR.base, ease: EASE_OUT }}>
      <motion.div className="keyhelp" onClick={(e) => e.stopPropagation()} initial={{ y: 8, scale: 0.98 }} animate={{ y: 0, scale: 1 }} exit={{ y: 6, scale: 0.98 }} transition={{ duration: DUR.base, ease: EASE_OUT }}>
        <div className="keyhelp-head"><b>{h("title")}</b><span className="ash">{h("close")}</span></div>
        <div className="keyhelp-grid">
          {sections.map((sec) => (
            <section key={sec.title}>
              <h4>{sec.title}</h4>
              {sec.rows.map(([k, label], i) => (
                <div key={i} className="keyhelp-row"><span className="keyhelp-keys">{k.split(" ").filter(Boolean).map((x, j) => <kbd key={j} className="mono">{x}</kbd>)}</span><span>{label}</span></div>
              ))}
            </section>
          ))}
        </div>
      </motion.div>
    </motion.div>
  );
}
