import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/** Large (2560 px) cached previews for the full-size views. One request per photo, shared by every caller. */
const jobs = new Map<string, Promise<string | null>>();
const done = new Map<string, string>();
/** `full` = the lightbox: the original file itself where the web view can show it. Everywhere else a 2560 px preview is enough and far cheaper. */
const keyOf = (path: string, full: boolean) => (full ? "f:" : "p:") + path;

export function largeSrc(path: string, full = false): Promise<string | null> {
  const k = keyOf(path, full);
  let job = jobs.get(k);
  if (!job) {
    job = invoke<string>("large_preview", { path, full }).then((p) => { const src = convertFileSrc(p); done.set(k, src); return src; }).catch(() => null);
    jobs.set(k, job);
  }
  return job;
}

/** 一张照片转过之后：它的大图换了文件，忘掉手里的，下次再要。 */
export function forgetLarge(path: string) {
  for (const full of [true, false]) { const k = keyOf(path, full); jobs.delete(k); done.delete(k); }
}

/** Warms the previews of the photos likely to be opened next. */
export function prefetchLarge(paths: string[]) {
  paths.forEach((p) => { if (!jobs.has(keyOf(p, false))) void largeSrc(p); });
}

/** The large preview once ready (null until then, so callers keep showing the thumbnail). */
export function useLarge(path: string, enabled = true, full = false): string | null {
  const k = keyOf(path, full);
  const [src, setSrc] = useState<string | null>(() => done.get(k) ?? null);
  // A new photo starts from its own cached preview (or nothing); flipping `enabled` alone never takes a loaded preview away.
  useEffect(() => { setSrc(done.get(k) ?? null); }, [k]);
  useEffect(() => {
    if (done.has(k) || !enabled || !path) return;
    let live = true;
    largeSrc(path, full).then((s) => { if (live) setSrc(s); });
    return () => { live = false; };
  }, [k, enabled]); // eslint-disable-line react-hooks/exhaustive-deps
  return src;
}
