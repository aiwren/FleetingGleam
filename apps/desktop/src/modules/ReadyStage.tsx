import { PointerEvent, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { motion, MotionValue, useMotionValue, useSpring, useTransform } from "motion/react";
import { useTranslation } from "react-i18next";
import Reveal from "../components/Reveal";
import Ticker from "../components/Ticker";
import { SPRING } from "../motion";
import { baseName } from "../projectActions";
import { ProbeFile } from "../types";

/** Where each print lands on the table (fractions of its width/height), and how big it is (% of table height). */
const SLOTS: { x: number; y: number; h: number }[] = [
  { x: 0.2, y: 0.26, h: 38 }, { x: 0.52, y: 0.18, h: 32 }, { x: 0.82, y: 0.3, h: 36 },
  { x: 0.28, y: 0.7, h: 34 }, { x: 0.58, y: 0.6, h: 40 }, { x: 0.88, y: 0.74, h: 30 },
  { x: 0.08, y: 0.5, h: 26 }, { x: 0.44, y: 0.92, h: 26 },
];
const tilt = (i: number) => (((i * 37) % 25) - 12) * 0.7; // deterministic "tossed" angles, about -8..8 degrees

function Print({ f, src, i, mx, my }: { f: ProbeFile; src: string; i: number; mx: MotionValue<number>; my: MotionValue<number>; }) {
  const slot = SLOTS[i % SLOTS.length];
  const depth = 0.4 + (i % 4) * 0.22; // near prints move more than far ones
  const x = useTransform(mx, [-1, 1], [-18 * depth, 18 * depth]);
  const y = useTransform(my, [-1, 1], [-13 * depth, 13 * depth]);
  const rot = tilt(i);
  const ratio = f.width / f.height;
  return (
    <div className="rs-card" style={{ left: `${slot.x * 100}%`, top: `${slot.y * 100}%`, ["--h" as string]: slot.h, ["--r" as string]: ratio }}>
      <motion.div className="rs-depth" style={{ x, y }}>
        <motion.div className="rs-in"
          initial={{ opacity: 0, y: 150, x: 70, rotate: rot * 2 + 22, scale: 0.78 }}
          animate={{ opacity: 1, y: 0, x: 0, rotate: rot, scale: 1 }}
          exit={{ opacity: 0, y: -70, x: -20, rotate: rot - 14, scale: 0.9, transition: { delay: i * 0.035, duration: 0.32, ease: [0.4, 0, 1, 1] } }}
          transition={{ ...SPRING.soft, delay: 0.12 + i * 0.075 }}
          whileHover={{ scale: 1.09, rotate: 0, transition: SPRING.tap }}>
          <img src={src} alt="" draggable={false} />
          <span className="rs-cap mono">{baseName(f.path)}</span>
        </motion.div>
      </motion.div>
    </div>
  );
}

/** A button that leans toward the cursor. */
function Magnetic({ children, onClick, primary }: { children: React.ReactNode; onClick: () => void; primary?: boolean }) {
  const px = useSpring(0, { stiffness: 260, damping: 18 }), py = useSpring(0, { stiffness: 260, damping: 18 });
  const move = (e: PointerEvent<HTMLButtonElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    px.set(Math.max(-8, Math.min(8, (e.clientX - (r.left + r.width / 2)) * 0.22)));
    py.set(Math.max(-6, Math.min(6, (e.clientY - (r.top + r.height / 2)) * 0.28)));
  };
  return (
    <motion.button className={`btn ${primary ? "primary" : ""}`} style={{ x: px, y: py }} onPointerMove={move} onPointerLeave={() => { px.set(0); py.set(0); }}
      whileTap={{ scale: 0.96 }} onClick={onClick}>{children}</motion.button>
  );
}

/**
 * The screen before analysis: the photos are tossed onto a light table. Parallax follows the cursor, a warm light follows it too,
 * and pressing Start sends the prints away one by one into the wall that is about to develop.
 */
export default function ReadyStage({ name, count, sizeText, files, thumbs, onStart, onAddFolder }: {
  name: string; count: number; sizeText: string; files: ProbeFile[]; thumbs: Record<string, string>; onStart: () => void; onAddFolder: () => void;
}) {
  const { t } = useTranslation();
  const table = useRef<HTMLDivElement>(null);
  const rawX = useMotionValue(0), rawY = useMotionValue(0);
  const mx = useSpring(rawX, { stiffness: 90, damping: 20 }), my = useSpring(rawY, { stiffness: 90, damping: 20 });
  const [n, setN] = useState(0);
  useEffect(() => { const id = setTimeout(() => setN(count), 450); return () => clearTimeout(id); }, [count]);

  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    const el = table.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const x = (e.clientX - r.left) / r.width, y = (e.clientY - r.top) / r.height;
    rawX.set((x - 0.5) * 2); rawY.set((y - 0.5) * 2);
    el.style.setProperty("--gx", `${(x * 100).toFixed(1)}%`);
    el.style.setProperty("--gy", `${(y * 100).toFixed(1)}%`);
  };

  return (
    <motion.div className="ready-stage" exit={{ opacity: 0, transition: { duration: 0.28, delay: 0.1 } }}>
      <div className="ready-left">
        <Reveal as="h1" text={name} delay={0.05} />
        <motion.div className="ready-count" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: 0.25 }} exit={{ opacity: 0 }}>
          <Ticker value={n} />
          <span>{t("cull.readyUnit")}</span>
        </motion.div>
        <motion.p className="mono ash" initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.5 }} exit={{ opacity: 0 }}>{t("cull.readySize", { size: sizeText })}</motion.p>
        <motion.p className="ash ready-body" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: 0.55 }} exit={{ opacity: 0 }}>{t("cull.readyBody")}</motion.p>
        <motion.div className="ready-actions" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ ...SPRING.soft, delay: 0.7 }} exit={{ opacity: 0 }}>
          <Magnetic primary onClick={onStart}>{t("cull.start")}</Magnetic>
          <Magnetic onClick={onAddFolder}>{t("cull.addFolder")}</Magnetic>
        </motion.div>
      </div>

      <div className="ptable" ref={table} onPointerMove={onMove} onPointerLeave={() => { rawX.set(0); rawY.set(0); }}>
        <i className="pglow" />
        <i className="crop tl" /><i className="crop tr" /><i className="crop bl" /><i className="crop br" />
        {files.slice(0, SLOTS.length).map((f, i) => <Print key={f.path} f={f} i={i} src={thumbs[f.path] ? convertFileSrc(thumbs[f.path]) : convertFileSrc(f.path)} mx={mx} my={my} />)}
      </div>
    </motion.div>
  );
}
