/**
 * 更新说明（发布时由 .github/scripts/release-notes.sh 生成，格式见 release-notes/README.md）的解析。
 * 和 Android 的 core/ReleaseNotes.kt 同一套规则：
 * - 中文在前、英文在后，中间一行 `<!-- en -->`；按界面语言只取一半，没有英文就都用中文。
 * - 只认几种块：`### 小标题`、单独一行的图片 `![说明](https://…)`、`- 列表项`、普通段落；行内只认 **加粗**。
 *   其余 Markdown 原样当文字，不渲染 HTML（说明来自 GitHub，不该在应用里执行任何东西）。
 */
export type NoteBlock =
  | { kind: "h"; text: string }
  | { kind: "img"; src: string; alt: string }
  | { kind: "list"; items: string[] }
  | { kind: "p"; text: string };

const EN_MARK = "<!-- en -->";

/** 取界面语言那一半。 */
export function notesFor(md: string, lang: string): string {
  const i = md.indexOf(EN_MARK);
  if (i < 0) return md.trim();
  const zh = md.slice(0, i).trim(), en = md.slice(i + EN_MARK.length).trim();
  return (lang.startsWith("zh") ? zh : en || zh);
}

/** 图片只认 https（再加上本机预览用的 http://localhost、127.0.0.1）；别的地址当没有。应用里的 CSP 另外只放行 raw.githubusercontent.com。 */
const okSrc = (s: string) => /^https:\/\//.test(s) || /^http:\/\/(127\.0\.0\.1|localhost)[:/]/.test(s);

export function parseNotes(md: string): NoteBlock[] {
  const out: NoteBlock[] = [];
  let para: string[] = [];
  let list: string[] | null = null;
  const flush = () => {
    if (para.length) { out.push({ kind: "p", text: para.join(" ") }); para = []; }
    if (list) { out.push({ kind: "list", items: list }); list = null; }
  };
  for (const raw of md.replace(/\r/g, "").split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("<!--") || /^-{3,}$/.test(line)) { flush(); continue; }
    const h = /^#{1,6}\s+(.*)$/.exec(line);
    if (h) { flush(); out.push({ kind: "h", text: h[1] }); continue; }
    const img = /^!\[([^\]]*)\]\(([^)\s]+)\)$/.exec(line);
    if (img) { flush(); if (okSrc(img[2])) out.push({ kind: "img", src: img[2], alt: img[1] }); continue; }
    const li = /^[-*]\s+(.*)$/.exec(line);
    if (li) { if (para.length) { out.push({ kind: "p", text: para.join(" ") }); para = []; } (list ??= []).push(li[1]); continue; }
    if (list) { out.push({ kind: "list", items: list }); list = null; }
    para.push(line);
  }
  flush();
  return out;
}

/** 行内：把 **加粗** 切成段，奇数段是粗体。 */
export const boldParts = (s: string): string[] => s.split("**");

/** 摘要：小标题列表（没有小标题就取前三段的头一句）。 */
export function noteHeadlines(blocks: NoteBlock[]): string[] {
  const hs = blocks.filter((b) => b.kind === "h").map((b) => (b as { text: string }).text);
  if (hs.length) return hs;
  return blocks.flatMap((b) => (b.kind === "list" ? b.items : b.kind === "p" ? [b.text] : [])).slice(0, 3).map((s) => s.replace(/\*\*/g, ""));
}
