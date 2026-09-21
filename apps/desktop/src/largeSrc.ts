import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/** Large (2560 px) cached previews for the full-size views. One request per photo, shared by every caller. */
const jobs = new Map<string, Promise<string | null>>();
const done = new Map<string, string>();

export function largeSrc(path: string): Promise<string | null> {
  let job = jobs.get(path);
  if (!job) {
    job = invoke<string>("large_preview", { path }).then((p) => { const src = convertFileSrc(p); done.set(path, src); return src; }).catch(() => null);
    jobs.set(path, job);
  }
  return job;
}

/** Warms the previews of the photos likely to be opened next. */
export function prefetchLarge(paths: string[]) {
  paths.forEach((p) => { if (!jobs.has(p)) void largeSrc(p); });
}

/** The large preview once ready (null until then, so callers keep showing the thumbnail). */
export function useLarge(path: string, enabled = true): string | null {
  const [src, setSrc] = useState<string | null>(() => done.get(path) ?? null);
  // A new photo starts from its own cached preview (or nothing); flipping `enabled` alone never takes a loaded preview away.
  useEffect(() => { setSrc(done.get(path) ?? null); }, [path]);
  useEffect(() => {
    if (done.has(path) || !enabled || !path) return;
    let live = true;
    largeSrc(path).then((s) => { if (live) setSrc(s); });
    return () => { live = false; };
  }, [path, enabled]);
  return src;
}
