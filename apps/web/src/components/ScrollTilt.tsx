import { useRef, type ReactNode } from "react";
import { motion, useReducedMotion, useScroll, useTransform } from "motion/react";

/**
 * A window that arrives leaning back and settles flat as the reader scrolls it into place. The motion belongs to the scroll
 * position, not the clock: stop scrolling and it stops, and nothing moves on its own.
 */
export default function ScrollTilt({ children, from = 14, className = "" }: { children: ReactNode; from?: number; className?: string }) {
  const reduce = useReducedMotion();
  const ref = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start end", "start 0.28"] });
  const rotateX = useTransform(scrollYProgress, [0, 1], [from, 0]);
  const scale = useTransform(scrollYProgress, [0, 1], [0.94, 1]);
  const y = useTransform(scrollYProgress, [0, 1], [28, 0]);
  return (
    <div ref={ref} className={className} style={{ perspective: 1800 }}>
      <motion.div style={reduce ? undefined : { rotateX, scale, y, transformOrigin: "50% 0%" }}>{children}</motion.div>
    </div>
  );
}
