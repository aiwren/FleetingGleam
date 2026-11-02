import { ReactNode, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import Accordion from "./Accordion";
import { Photo, isRawPath } from "../types";
import { fmtBytes } from "../format";

/** 核心 `photo_info` 给的文件信息：数值字段都已经是人读的短字符串（"1/250 s"、"f/2.8"、"+0.3 EV"），这里不再二次格式化。 */
export type Info = {
  format: string | null; bytes: number; color_space: string | null; bit_depth: string | null;
  camera: string | null; lens: string | null; focal: string | null; aperture: string | null; shutter: string | null; iso: string | null; software: string | null; taken: string | null;
  exposure_bias: string | null; white_balance: string | null; flash: string | null; metering: string | null; exposure_program: string | null; focal_35: string | null;
  /** 这张照片由哪几个文件组成（自己 + 同名伴随 RAW）。 */
  files: { path: string; bytes: number }[];
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

/** 顶部角标：按扩展名与伴随文件判断这张照片由什么组成。 */
function kindOf(photo: Photo, files: { path: string }[]): "jpgOnly" | "rawJpg" | "rawOnly" {
  if (isRawPath(photo.path)) return "rawOnly";
  // 核心列出的文件（含同名 RAW）比前端手里的 companions 更可靠：照片记录是旧的也能对上
  return files.some((f) => f.path !== photo.path && isRawPath(f.path)) || photo.companions?.some(isRawPath) ? "rawJpg" : "jpgOnly";
}

/**
 * EXIF 卡：角标（JPG ONLY / RAW + JPG / RAW ONLY）、四格大字（快门 / 光圈 / ISO / 焦距），
 * 下面机身、镜头、时间等（有值才显示），再是"包含文件"及各自大小，最后是格式 / 像素 / 色彩。
 * `dark` 是查看器里的浮层版本，字色固定为浅色。
 */
export default function FileFacts({ photo, dark = false }: { photo: Photo; dark?: boolean }) {
  const { t, i18n } = useTranslation();
  const info = useInfo(photo.path);
  const mp = (photo.width * photo.height) / 1e6;
  const color = info?.color_space && info.color_space !== "Uncalibrated" ? info.color_space : null;
  const depth = info?.bit_depth ? `${info.bit_depth.split(",")[0].trim()} bit` : null;
  const cells: [string, string | null | undefined, string | null | undefined][] = [
    [t("file.shutter"), info?.shutter, null],
    [t("file.aperture"), info?.aperture, null],
    ["ISO", info?.iso, null],
    // 焦距下面一行小字是等效 35mm（跟实际焦距一样时不重复）
    [t("file.focal"), info?.focal, info?.focal_35 && info.focal_35 !== info.focal ? `≈ ${info.focal_35}` : null],
  ];
  const details: [string, ReactNode][] = [
    [t("file.camera"), info?.camera],
    [t("file.lens"), info?.lens],
    [t("file.taken"), info?.taken],
    [t("file.bias"), info?.exposure_bias],
    [t("file.wb"), info?.white_balance],
    [t("file.flash"), info?.flash],
    [t("file.metering"), info?.metering],
    [t("file.program"), info?.exposure_program],
  ];
  // 核心没给（旧版本或还没回来）时，至少把自己和伴随 RAW 列出来
  const files = info?.files?.length ? info.files : [photo.path, ...(photo.companions ?? [])].map((path) => ({ path, bytes: 0 }));
  const facts: [string, ReactNode][] = [
    [t("file.format"), [info?.format, info ? fmtBytes(i18n.language, info.bytes) : null].filter(Boolean).join(" · ")],
    [t("file.pixels"), `${photo.width} × ${photo.height} · ${mp.toFixed(1)} MP`],
    [t("file.color"), [color, depth].filter(Boolean).join(" · ")],
  ];
  return (
    <div className={`exif ${dark ? "dark" : ""}`}>
      <span className="exif-kind mono">{t(`file.${kindOf(photo, files)}`)}</span>
      <div className="exif-grid">
        {cells.map(([k, v, sub]) => <div key={k}><b className="mono">{v || "—"}</b>{sub && <i className="mono">{sub}</i>}<small>{k}</small></div>)}
      </div>
      <dl className="kv">
        {details.filter(([, v]) => v).map(([k, v]) => <div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}
      </dl>
      <dl className="kv exif-files">
        {files.map((f, i) => <div key={f.path}><dt>{i === 0 ? t("file.files") : ""}</dt><dd><span className="mono">{f.path.split("/").pop()}</span>{f.bytes > 0 && <small>{fmtBytes(i18n.language, f.bytes)}</small>}</dd></div>)}
      </dl>
      <dl className="kv">
        {facts.filter(([, v]) => v).map(([k, v]) => <div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}
      </dl>
    </div>
  );
}

/** The file facts as a collapsible row; the summary shows the essentials (format and size) without opening it. */
export function FileSection({ photo }: { photo: Photo }) {
  const { t, i18n } = useTranslation();
  const info = useInfo(photo.path);
  const hint = info ? [info.format, fmtBytes(i18n.language, info.bytes)].filter(Boolean).join(" · ") : undefined;
  return <Accordion icon="file" title={t("file.title")} hint={hint}><FileFacts photo={photo} /></Accordion>;
}
