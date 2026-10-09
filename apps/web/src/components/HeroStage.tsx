import { useEffect, useRef, useState, type CSSProperties } from "react";
import { motion, useMotionValue, useReducedMotion, useScroll, useSpring, useTransform, type MotionValue } from "motion/react";
import { copy, type Lang } from "../content";
import Crop from "./Crop";

const EASE = [0.16, 1, 0.3, 1] as const;
const W = 1100, H = 580, PW = 640, PH = 470;

/** The three places the product does its work, lifted out of one window and hung in space: where bursts are grouped, where two
 * frames are compared, where the reasons are read. Each pane is a real region of the real app; `z` is how far it sits forward. */
const PANES = [
  { id: "group", shot: "wall", crop: [190, 50, 752, 640], scale: 0.76, left: 6, top: 84, z: 0, ry: 9, depth: 6, delay: 0.25 },
  { id: "compare", shot: "compare", crop: [448, 44, 490, 650], scale: 0.84, left: 362, top: 40, z: 90, ry: 0, depth: 22, delay: 0.4 },
  { id: "decide", shot: "wall", crop: [950, 0, 250, 610], scale: 0.8, left: 858, top: 76, z: 40, ry: -9, depth: 12, delay: 0.55 },
] as const;
const PHONE_PANES = [
  { id: "group", shot: "wall", crop: [190, 50, 752, 640], scale: 0.5, left: 0, top: 78, z: 0, ry: 8, depth: 4, delay: 0.25 },
  { id: "compare", shot: "compare", crop: [448, 44, 490, 650], scale: 0.62, left: 300, top: 30, z: 60, ry: 0, depth: 12, delay: 0.4 },
] as const;

function Layer({ pane, i, px, py, label, ready, reduce }: { pane: { id: string; shot: "wall" | "compare"; crop: readonly number[]; scale: number; left: number; top: number; z: number; ry: number; depth: number; delay: number }; i: number; px: MotionValue<number>; py: MotionValue<number>; label: string; ready: boolean; reduce: boolean | null }) {
  const x = useTransform(px, (v) => v * pane.depth);
  const y = useTransform(py, (v) => v * pane.depth * 0.7);
  const { scrollY } = useScroll();
  const lift = useTransform(scrollY, [0, 700], [0, -pane.depth * 3.2]);
  const [sx, sy, sw, sh] = pane.crop as [number, number, number, number];
  const style: CSSProperties = { position: "absolute", left: pane.left, top: pane.top, width: sw * pane.scale, height: sh * pane.scale, transformStyle: "preserve-3d" };
  return (
    <motion.div style={{ ...style, x: reduce ? 0 : x, y: reduce ? 0 : y }}>
      <motion.div style={{ y: reduce ? 0 : lift, transformStyle: "preserve-3d" }}>
        <motion.div initial={reduce ? false : { opacity: 0, y: 46, rotateY: pane.ry * 2.2, scale: 0.94, filter: "blur(10px)" }}
          animate={ready ? { opacity: 1, y: 0, rotateY: pane.ry, scale: 1, filter: "blur(0px)" } : undefined}
          transition={{ duration: 1.25, ease: EASE, delay: pane.delay }}
          style={{ transform: `translateZ(${pane.z}px)`, transformStyle: "preserve-3d", boxShadow: "var(--lift)", borderRadius: 18 }}>
          <Crop shot={pane.shot} x={sx} y={sy} w={sw} h={sh} scale={pane.scale} />
          <motion.span className="absolute -top-9 left-3 flex items-center gap-2 rounded-full border border-line bg-canvas/80 px-3 py-1 text-xs font-medium text-ink backdrop-blur-md"
            initial={reduce ? false : { opacity: 0, y: 6 }} animate={ready ? { opacity: 1, y: 0 } : undefined} transition={{ duration: 0.7, ease: EASE, delay: 1.5 + i * 0.18 }}>
            <i className="size-1.5 rounded-full bg-gleam" /><b className="tabular-nums text-ash">{i + 1}</b>{label}
          </motion.span>
        </motion.div>
      </motion.div>
    </motion.div>
  );
}

/**
 * The hero's picture: the real app pulled apart into three panes that arrive out of focus and settle into depth, then lean
 * a few pixels toward the pointer — each at its own depth — and drift apart a little as the page is scrolled. Nothing loops;
 * it only ever answers to the reader.
 */
export default function HeroStage({ lang }: { lang: Lang }) {
  const c = copy[lang].hero;
  const reduce = useReducedMotion();
  const box = useRef<HTMLDivElement>(null);
  const [u, setU] = useState(0);
  const [phone, setPhone] = useState(false);
  useEffect(() => {
    const el = box.current!;
    // The picture is fitted to what is left of the first screen, so the whole of it is in view the moment the page opens.
    const read = () => {
      const isPhone = window.innerWidth < 640;
      const cw = isPhone ? PW : W, ch = isPhone ? PH : H;
      const top = el.getBoundingClientRect().top + window.scrollY;
      const room = isPhone ? Infinity : Math.max(0.5 * ch, window.innerHeight - top - 28);
      setPhone(isPhone);
      setU(Math.min(1, el.getBoundingClientRect().width / cw, room / ch));
    };
    read();
    const ro = new ResizeObserver(read);
    ro.observe(el);
    window.addEventListener("resize", read);
    document.fonts?.ready.then(read);
    return () => { ro.disconnect(); window.removeEventListener("resize", read); };
  }, []);
  const cw = phone ? PW : W, ch = phone ? PH : H;
  const panes = phone ? PHONE_PANES : PANES;

  const mx = useSpring(useMotionValue(0), { stiffness: 90, damping: 18 });
  const my = useSpring(useMotionValue(0), { stiffness: 90, damping: 18 });
  const ry = useTransform(mx, (v) => v * 3.2);
  const rx = useTransform(my, (v) => v * -2.2);
  const onMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    mx.set(((e.clientX - r.left) / r.width - 0.5) * 2);
    my.set(((e.clientY - r.top) / r.height - 0.5) * 2);
  };

  return (
    <div ref={box} className="relative mx-auto w-full max-w-[1100px]" style={{ height: ch * (u || 1) }} onPointerMove={onMove} onPointerLeave={() => { mx.set(0); my.set(0); }}>
      <div aria-hidden className="pointer-events-none absolute left-1/2 top-[44%] -z-10 h-[78%] w-[92%] -translate-x-1/2 -translate-y-1/2 rounded-full opacity-[.5] blur-3xl"
        style={{ background: "radial-gradient(closest-side, color-mix(in srgb, var(--gleam) 22%, transparent), transparent)" }} />
      <div style={{ width: cw, height: ch, transform: `scale(${u || 1})`, transformOrigin: "0 0", marginLeft: `calc((100% - ${cw * (u || 1)}px) / 2)`, opacity: u ? 1 : 0, perspective: 1700 }}>
        <motion.div style={{ width: cw, height: ch, rotateX: reduce ? 0 : rx, rotateY: reduce ? 0 : ry, transformStyle: "preserve-3d" }}>
          {panes.map((p, i) => (
            <Layer key={p.id} pane={p} i={i} px={mx} py={my} label={c.steps[i]} ready={u > 0} reduce={reduce} />
          ))}
        </motion.div>
      </div>
    </div>
  );
}
