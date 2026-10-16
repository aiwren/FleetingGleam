import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Hit = { path: string; name: string; thumb: string; width: number; height: number; score: number };
type Status = { total: number; ready: number; available: boolean };

export const searchPhotos = (dirs: string[], query: string) => invoke<Hit[]>("search_photos", { dirs, query });

/**
 * Keeps the search index of some folders complete. Photos need a small vector each before they can be found; it is made in the
 * background from the thumbnails (fast, local) as soon as the folders are known, so by the time someone searches it is ready.
 */
export function useSearchIndex(dirs: string[], rev = 0) {
  const key = `${dirs.join("\n")}|${rev}`;
  const [status, setStatus] = useState<Status | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const running = useRef("");

  useEffect(() => {
    if (!dirs.length) { setStatus(null); return; }
    let live = true;
    const refresh = () => invoke<Status>("search_status", { dirs }).then((s) => { if (live) setStatus(s); return s; });
    refresh().then((s) => {
      if (!live || !s.available || s.ready >= s.total || running.current === key) return;
      running.current = key;
      setProgress(0);
      invoke("search_index", { dirs })
        .catch(() => { /* the index is optional: search then covers what is ready */ })
        .finally(() => { running.current = ""; if (live) { setProgress(null); refresh(); } });
    }).catch(() => { /* not running inside the app */ });
    let un: (() => void) | undefined;
    listen<{ done: number; total: number }>("search-index-progress", (e) => { if (live) setProgress(e.payload.total ? e.payload.done / e.payload.total : 1); }).then((f) => { if (live) un = f; else f(); });
    return () => { live = false; un?.(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  return { available: status?.available ?? false, ready: status?.ready ?? 0, total: status?.total ?? 0, progress };
}

/** A photo to select once a project has opened (set by the global search). */
let pending: string | null = null;
export const setPendingSelect = (path: string | null) => { pending = path; };
export const takePendingSelect = () => { const p = pending; pending = null; return p; };
