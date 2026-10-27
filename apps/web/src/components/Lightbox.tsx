import { useEffect, useRef } from "react";
import { copy, type Lang } from "../content";
import { Ic } from "./Icons";
import { EASE, EASE_IN, reduced } from "./scroll";

/**
 * 放大查看（始终深色）：点任何一张可放大的照片（.ph-btn），它从原位置飞到中间，背后铺一层它自己的模糊光；
 * 关掉时再飞回原处。Esc、点空白处、关闭按钮都能关。
 */
export default function Lightbox({ lang }: { lang: Lang }) {
  const c = copy[lang].lightbox;
  const dlg = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const lb = dlg.current!;
    const img = lb.querySelector<HTMLImageElement>(".lbx-img")!, amb = lb.querySelector<HTMLImageElement>(".lbx-amb")!, cap = lb.querySelector<HTMLElement>(".lbx-cap")!, close = lb.querySelector<HTMLButtonElement>(".lbx-close")!;
    let from: HTMLElement | null = null, closing = false;
    function open(btn: HTMLElement) {
      const s = btn.querySelector("img")!;
      from = btn; closing = false;
      img.src = s.currentSrc || s.src; img.alt = s.alt; amb.src = img.src;
      cap.textContent = btn.dataset.cap || c.fallback;
      lb.showModal();
      close.focus({ preventScroll: true });
      if (reduced()) { amb.style.opacity = ".45"; cap.style.opacity = "1"; return; }
      const a = s.getBoundingClientRect(), b = img.getBoundingClientRect();
      img.animate([{ transform: `translate(${a.left - b.left}px, ${a.top - b.top}px) scale(${a.width / b.width})` }, { transform: "none" }], { duration: 560, easing: EASE });
      amb.animate([{ opacity: 0 }, { opacity: 0.45 }], { duration: 600, easing: EASE, fill: "forwards" });
      cap.animate([{ opacity: 0, transform: "translateY(6px)" }, { opacity: 1, transform: "none" }], { duration: 320, delay: 300, easing: EASE, fill: "forwards" });
    }
    async function shut() {
      if (closing || !lb.open) return;
      closing = true;
      const back = from && from.isConnected && !from.hidden ? from : null;
      if (!reduced() && back) {
        const a = back.getBoundingClientRect(), b = img.getBoundingClientRect();
        cap.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 120, easing: EASE_IN, fill: "forwards" });
        amb.animate([{ opacity: 0.45 }, { opacity: 0 }], { duration: 360, easing: EASE_IN, fill: "forwards" });
        await img.animate([{ transform: "none" }, { transform: `translate(${a.left - b.left}px, ${a.top - b.top}px) scale(${a.width / b.width})` }], { duration: 380, easing: EASE }).finished.catch(() => {});
      } else if (!reduced()) {
        await lb.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 200, easing: EASE_IN }).finished.catch(() => {});
      }
      lb.close();
      amb.getAnimations().forEach((x) => x.cancel()); cap.getAnimations().forEach((x) => x.cancel());
      amb.style.opacity = ""; cap.style.opacity = "";
      back?.focus({ preventScroll: true });
      closing = false;
    }
    const onCancel = (e: Event) => { e.preventDefault(); shut(); };
    const onBackdrop = (e: MouseEvent) => { const t = e.target as HTMLElement; if (t === lb || t.classList.contains("lbx-fig")) shut(); };
    const onDoc = (e: MouseEvent) => { const b = (e.target as HTMLElement).closest<HTMLElement>(".ph-btn"); if (b) open(b); };
    close.addEventListener("click", shut);
    lb.addEventListener("cancel", onCancel);
    lb.addEventListener("click", onBackdrop);
    document.addEventListener("click", onDoc);
    return () => { close.removeEventListener("click", shut); lb.removeEventListener("cancel", onCancel); lb.removeEventListener("click", onBackdrop); document.removeEventListener("click", onDoc); };
  }, [c]);
  return (
    <dialog className="lbx" ref={dlg} aria-label={c.label}>
      <img className="lbx-amb" alt="" width={600} height={400} />
      <button className="lbx-close" type="button" aria-label={c.close}><Ic id="x" /></button>
      <figure className="lbx-fig">
        <img className="lbx-img" alt="" width={600} height={400} />
        <figcaption className="lbx-cap" />
      </figure>
    </dialog>
  );
}
