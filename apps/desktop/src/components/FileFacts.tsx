import { ReactNode, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import Accordion from "./Accordion";
import { Photo } from "../types";

type Info = {
  format: string | null; bytes: number; color_space: string | null; bit_depth: string | null;
  camera: string | null; lens: string | null; focal: string | null; aperture: string | null; shutter: string | null; iso: string | null; software: string | null; taken: string | null;
};

const cache = new Map<string, Info>();

function useInfo(path: string) {
  const [info, setInfo] = useState<Info | null>(() => cache.get(path) ?? null);
  useEffect(() => {
    const hit = cache.get(path);
    if (hit) { setInfo(hit); return; }
    setInfo(null);
    let live = true;
    invoke<Info>("photo_info", { path }).then((r) => { cache.set(path, r); if (live) setInfo(r); }).catch(() => { /* metadata is optional */ });
    return () => { live = false; };
  }, [path]);
  return info;
}

import { fmtBytes } from "../format";

/** EXIF stores the date as "2026:06:13 14:31:17"; show it the way people write it, without the seconds. */
const niceTime = (v: string) => v.replace(/^(\d{4}):(\d{2}):(\d{2})/, "$1-$2-$3").replace(/:\d{2}$/, "");

/** EXIF can hold an exposure time as a long fraction ("1/813.2157314956827 s"): shown as people write it ("1/813 s"). */
function niceShutter(v: string): string {
  const frac = v.match(/^1\/(\d+(?:\.\d+)?)\s*s?$/);
  if (frac) return `1/${Math.round(Number(frac[1]))} s`;
  const sec = v.match(/^(\d+(?:\.\d+)?)\s*s?$/);
  if (sec) return `${Number(Number(sec[1]).toFixed(2))} s`;
  return v;
}

/** File facts as an aligned list: what the file is, what shot it, and the shooting parameters as small tags. */
export default function FileFacts({ photo }: { photo: Photo }) {
  const { t, i18n } = useTranslation();
  const info = useInfo(photo.path);
  const mp = (photo.width * photo.height) / 1e6;
  const color = info?.color_space && info.color_space !== "Uncalibrated" ? info.color_space : null;
  const depth = info?.bit_depth ? `${info.bit_depth.split(",")[0].trim()} bit` : null;
  const tags = [info?.focal, info?.aperture, info?.shutter ? niceShutter(info.shutter) : null, info?.iso ? `ISO ${info.iso}` : null].filter(Boolean) as string[];
  const rows: [string, ReactNode][] = [
    [t("file.format"), [info?.format, info ? fmtBytes(i18n.language, info.bytes) : null].filter(Boolean).join(" · ")],
    [t("file.pixels"), `${photo.width} × ${photo.height} · ${mp.toFixed(1)} MP`],
    [t("file.color"), [color, depth].filter(Boolean).join(" · ")],
    [t("file.camera"), info?.camera ? <>{info.camera}{info.lens && <small>{info.lens}</small>}</> : info?.lens],
    [t("file.exposure"), tags.length ? <span className="tags">{tags.map((x) => <i key={x} className="mono">{x}</i>)}</span> : null],
    [t("file.taken"), info?.taken ? niceTime(info.taken) : null],
    // RAW + JPG 配对：这张实际由哪几个文件组成
    [t("file.files"), photo.companions?.length ? <span className="files">{[photo.path, ...photo.companions].map((f) => <i key={f} className="mono">{f.split("/").pop()}</i>)}</span> : null],
  ];
  return (
    <dl className="kv">
      {rows.filter(([, v]) => v).map(([k, v]) => <div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}
    </dl>
  );
}

/** The file facts as a collapsible row; the summary shows the essentials (format and size) without opening it. */
export function FileSection({ photo }: { photo: Photo }) {
  const { t, i18n } = useTranslation();
  const info = useInfo(photo.path);
  const hint = info ? [info.format, fmtBytes(i18n.language, info.bytes)].filter(Boolean).join(" · ") : undefined;
  return <Accordion icon="file" title={t("file.title")} hint={hint}><FileFacts photo={photo} /></Accordion>;
}
